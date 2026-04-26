//! Playback controller — shared state between the TUI and audio pipeline.
//!
//! The controller owns the playback queue and manages the lifecycle of
//! decode → ring buffer → CPAL output for each track. The TUI sends
//! commands (play, pause, next, seek) and reads state (position, duration,
//! current track) without blocking the audio pipeline.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::backend::cpal_backend;
use crate::core::favorites::Favorites;
use crate::core::session::Session;
use crate::core::track::{Track, TrackSource};
use crate::core::types::Volume;
use crate::playback::decoder;
use crate::playback::eq::{self, EqPreset, Equalizer};
use crate::playback::fft::SpectrumData;
use crate::playback::replaygain::{self, ReplayGainInfo, ReplayGainMode};
use crate::playback::speed;
use crate::playback::stream_decoder;

/// Playback state visible to the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

/// Repeat mode for the playback queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    /// Cycle to the next repeat mode: Off → All → One → Off.
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

impl std::fmt::Display for RepeatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::All => write!(f, "All"),
            Self::One => write!(f, "One"),
        }
    }
}

/// Commands sent from the TUI to the playback controller.
pub enum PlayerCommand {
    PlayPause,
    Stop,
    NextTrack,
    PrevTrack,
    #[allow(dead_code)] // Seek not yet implemented
    SeekForward(Duration),
    #[allow(dead_code)]
    SeekBackward(Duration),
    VolumeUp,
    VolumeDown,
    ToggleShuffle,
    CycleRepeat,
    ToggleFavorite,
    #[allow(dead_code)]
    SetSleepTimer(u64),
    #[allow(dead_code)]
    CancelSleepTimer,
    CycleSleepTimer,
    SpeedUp,
    SpeedDown,
    CycleEqPreset,
    EqBandUp,
    EqBandDown,
    EqBandLeft,
    EqBandRight,
    #[allow(dead_code)]
    ToggleVisualizer,
    #[allow(dead_code)]
    ToggleFullscreenVisualizer,
    Quit,
}

/// Duration of the fade-out before the sleep timer stops playback.
const FADE_DURATION: Duration = Duration::from_secs(30);

/// Preset durations (in minutes) for the sleep timer cycle.
const SLEEP_PRESETS: [u64; 5] = [15, 30, 45, 60, 90];

/// Sleep timer configuration for automatic stop after a duration.
#[derive(Debug)]
pub struct SleepTimer {
    pub end_time: Instant,
    pub total_duration: Duration,
    pub fade_started: bool,
    pub original_volume_db: f32,
}

/// Result of ticking the sleep timer each frame.
#[derive(Debug)]
#[allow(dead_code)]
pub enum SleepAction {
    None,
    Active(u64),
    Fading(u64),
    Stop,
}

/// Shared playback position updated by the producer thread.
struct SharedPosition {
    samples_played: AtomicU64,
    total_samples: AtomicU64,
}

/// Pre-decoded next track data for gapless playback.
#[derive(Debug)]
pub struct PreDecodedTrack {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
    pub track_index: usize,
}

/// The playback controller manages the queue and audio pipeline.
pub struct Player {
    tracks: Vec<Track>,
    current_index: usize,
    volume: Volume,
    state: PlaybackState,
    eq_preset: Option<&'static EqPreset>,
    eq_preset_index: usize,
    eq_selected_band: usize,
    custom_gains: Option<[f32; 10]>,
    shuffle: bool,
    repeat: RepeatMode,
    shuffle_order: Vec<usize>,

    // Current track playback
    current_samples: Option<Vec<f32>>,
    current_sample_rate: u32,
    current_channels: usize,
    current_duration: Duration,
    playback_position: Duration,
    playback_start: Option<Instant>,
    pause_offset: Duration,

    // Gapless playback: pre-decoded next track
    next_track_samples: Option<PreDecodedTrack>,

    speed: f32,

    replaygain_mode: ReplayGainMode,
    current_rg: Option<ReplayGainInfo>,

    favorites: Favorites,
    sleep_timer: Option<SleepTimer>,

    spectrum: Arc<SpectrumData>,

    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    shared_volume: Arc<AtomicU32>,
    position: Arc<SharedPosition>,
}

impl Player {
    pub fn new(
        tracks: Vec<Track>,
        volume_db: f32,
        eq_preset: Option<&str>,
        replaygain_mode: ReplayGainMode,
    ) -> Result<Self> {
        let preset = match eq_preset {
            Some(name) => Some(eq::find_preset(name).with_context(|| {
                format!(
                    "Unknown EQ preset '{}'. Available: {}",
                    name,
                    eq::preset_names().join(", ")
                )
            })?),
            None => None,
        };

        let initial_linear = Volume(volume_db).as_linear();

        let eq_preset_index = preset
            .map(|p| {
                eq::PRESETS
                    .iter()
                    .position(|bp| std::ptr::eq(bp, p))
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let favorites = Favorites::load().unwrap_or_else(|e| {
            tracing::warn!("Failed to load favorites: {e}");
            Favorites::default()
        });

        Ok(Self {
            tracks,
            current_index: 0,
            volume: Volume(volume_db),
            state: PlaybackState::Stopped,
            eq_preset: preset,
            eq_preset_index,
            eq_selected_band: 0,
            custom_gains: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            shuffle_order: Vec::new(),
            current_samples: None,
            current_sample_rate: 44100,
            current_channels: 2,
            current_duration: Duration::ZERO,
            playback_position: Duration::ZERO,
            playback_start: None,
            pause_offset: Duration::ZERO,
            next_track_samples: None,
            speed: speed::DEFAULT_SPEED,
            replaygain_mode,
            current_rg: None,
            favorites,
            sleep_timer: None,
            spectrum: Arc::new(SpectrumData::new(32)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            shared_volume: Arc::new(AtomicU32::new(initial_linear.to_bits())),
            position: Arc::new(SharedPosition {
                samples_played: AtomicU64::new(0),
                total_samples: AtomicU64::new(0),
            }),
        })
    }

    /// Load and start playing the current track.
    pub fn play_current(&mut self) -> Result<()> {
        if self.tracks.is_empty() {
            return Ok(());
        }

        let track = &self.tracks[self.current_index];
        let decoded = match &track.source {
            TrackSource::File(p) => decoder::decode_file(p)
                .with_context(|| format!("Failed to decode {}", p.display()))?,
            TrackSource::Url(url) => stream_decoder::decode_url(url)
                .with_context(|| format!("Failed to stream {url}"))?,
        };

        let mut samples = decoded.samples;

        // Apply ReplayGain before EQ and speed
        if self.replaygain_mode != ReplayGainMode::Off {
            if let TrackSource::File(p) = &self.tracks[self.current_index].source {
                let rg_info = replaygain::read_replaygain(p);
                if let Some(gain_db) = replaygain::gain_to_apply(&rg_info, self.replaygain_mode) {
                    replaygain::apply_replaygain(&mut samples, gain_db);
                    tracing::info!("ReplayGain: {gain_db:+.1}dB");
                }
                self.current_rg = Some(rg_info);
            } else {
                self.current_rg = None;
            }
        } else {
            self.current_rg = None;
        }

        let samples = self.apply_eq_to_samples(samples, decoded.sample_rate, decoded.channels);
        let samples = speed::apply_speed(&samples, decoded.channels, self.speed);

        let total_samples = samples.len() as u64;
        let sample_rate = decoded.sample_rate;
        let channels = decoded.channels;

        // Duration accounts for speed: resampled buffer is shorter/longer
        self.current_duration =
            Duration::from_secs_f64(samples.len() as f64 / (sample_rate as f64 * channels as f64));
        self.current_sample_rate = sample_rate;
        self.current_channels = channels;
        self.current_samples = Some(samples);
        self.playback_position = Duration::ZERO;
        self.pause_offset = Duration::ZERO;

        self.position
            .total_samples
            .store(total_samples, Ordering::Relaxed);
        self.position.samples_played.store(0, Ordering::Relaxed);

        // Sync shared volume so the producer thread picks up the current value
        self.shared_volume
            .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);

        self.state = PlaybackState::Playing;
        self.playback_start = Some(Instant::now());

        Ok(())
    }

    /// Load pre-decoded track data into the player without re-decoding.
    /// Used for gapless transitions where the next track was already decoded.
    pub fn play_predecoded(&mut self, pre: PreDecodedTrack) {
        let total_samples = pre.samples.len() as u64;
        self.current_duration = Duration::from_secs_f64(
            pre.samples.len() as f64 / (pre.sample_rate as f64 * pre.channels as f64),
        );
        self.current_sample_rate = pre.sample_rate;
        self.current_channels = pre.channels;
        self.current_samples = Some(pre.samples);
        self.playback_position = Duration::ZERO;
        self.pause_offset = Duration::ZERO;

        self.position
            .total_samples
            .store(total_samples, Ordering::Relaxed);
        self.position.samples_played.store(0, Ordering::Relaxed);

        self.shared_volume
            .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);

        self.state = PlaybackState::Playing;
        self.playback_start = Some(Instant::now());
    }

    /// Start non-blocking playback of the current loaded samples.
    /// Returns a JoinHandle that completes when playback finishes.
    pub fn start_playback(&mut self) -> Option<std::thread::JoinHandle<Result<()>>> {
        let samples = self.current_samples.take()?;
        let sample_rate = self.current_sample_rate;
        let channels = self.current_channels;
        let volume = self.shared_volume.clone();

        // Reset flags for this playback session.
        self.stop_flag.store(false, Ordering::Relaxed);
        self.pause_flag.store(false, Ordering::Relaxed);
        let stop = self.stop_flag.clone();
        let pause = self.pause_flag.clone();
        let position = self.position.clone();
        let spectrum = self.spectrum.clone();

        let handle = std::thread::spawn(move || {
            cpal_backend::play_audio_with_position(
                &samples,
                sample_rate,
                channels,
                &volume,
                &stop,
                &pause,
                &position.samples_played,
                &spectrum,
            )
        });

        Some(handle)
    }

    /// Start gapless playback: play current samples then seamlessly continue
    /// with the next track's samples without tearing down the CPAL stream.
    /// Returns a (JoinHandle, Arc<AtomicBool>) — the bool signals whether
    /// the next track was actually played (allows TUI to advance state).
    #[allow(dead_code)] // Available for true zero-gap playback in future
    pub fn start_playback_gapless(
        &mut self,
        next_samples: Vec<f32>,
    ) -> Option<(std::thread::JoinHandle<Result<()>>, Arc<AtomicBool>)> {
        let samples = self.current_samples.take()?;
        let sample_rate = self.current_sample_rate;
        let channels = self.current_channels;
        let volume = self.shared_volume.clone();

        self.stop_flag.store(false, Ordering::Relaxed);
        self.pause_flag.store(false, Ordering::Relaxed);
        let stop = self.stop_flag.clone();
        let pause = self.pause_flag.clone();
        let position = self.position.clone();
        let spectrum = self.spectrum.clone();
        let played_next = Arc::new(AtomicBool::new(false));
        let played_next_clone = played_next.clone();

        let handle = std::thread::spawn(move || {
            cpal_backend::play_audio_gapless(
                &samples,
                Some(&next_samples),
                sample_rate,
                channels,
                &volume,
                &stop,
                &pause,
                &position.samples_played,
                &spectrum,
                &played_next_clone,
            )
        });

        Some((handle, played_next))
    }

    /// Handle a command from the TUI.
    pub fn handle_command(&mut self, cmd: PlayerCommand) -> PlayerAction {
        match cmd {
            PlayerCommand::PlayPause => match self.state {
                PlaybackState::Playing => {
                    self.state = PlaybackState::Paused;
                    self.pause_offset = self.current_position();
                    self.pause_flag.store(true, Ordering::Relaxed);
                    PlayerAction::None
                }
                PlaybackState::Paused => {
                    self.state = PlaybackState::Playing;
                    self.pause_flag.store(false, Ordering::Relaxed);
                    self.playback_start = Some(Instant::now());
                    PlayerAction::None
                }
                PlaybackState::Stopped => {
                    self.state = PlaybackState::Playing;
                    PlayerAction::LoadAndPlay
                }
            },
            PlayerCommand::Stop => {
                self.state = PlaybackState::Stopped;
                self.playback_position = Duration::ZERO;
                self.pause_offset = Duration::ZERO;
                self.stop_flag.store(true, Ordering::Relaxed);
                self.clear_predecoded();
                PlayerAction::None
            }
            PlayerCommand::NextTrack => {
                self.stop_flag.store(true, Ordering::Relaxed);
                self.clear_predecoded();
                if let Some(next) = self.next_index() {
                    self.current_index = next;
                    PlayerAction::LoadAndPlay
                } else {
                    self.state = PlaybackState::Stopped;
                    PlayerAction::None
                }
            }
            PlayerCommand::PrevTrack => {
                self.stop_flag.store(true, Ordering::Relaxed);
                self.clear_predecoded();
                // If more than 3 seconds in, restart current track
                if self.current_position().as_secs() > 3 || self.current_index == 0 {
                    PlayerAction::LoadAndPlay
                } else if let Some(prev) = self.prev_index() {
                    self.current_index = prev;
                    PlayerAction::LoadAndPlay
                } else {
                    PlayerAction::LoadAndPlay
                }
            }
            PlayerCommand::SeekForward(_) | PlayerCommand::SeekBackward(_) => {
                // Seeking requires re-decode — simplified for now
                PlayerAction::None
            }
            PlayerCommand::VolumeUp => {
                self.volume = Volume((self.volume.0 + 1.0).min(6.0));
                self.shared_volume
                    .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);
                PlayerAction::None
            }
            PlayerCommand::VolumeDown => {
                self.volume = Volume((self.volume.0 - 1.0).max(-30.0));
                self.shared_volume
                    .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);
                PlayerAction::None
            }
            PlayerCommand::ToggleShuffle => {
                self.shuffle = !self.shuffle;
                if self.shuffle {
                    self.shuffle_order = generate_shuffle_order(self.tracks.len());
                }
                self.clear_predecoded();
                PlayerAction::None
            }
            PlayerCommand::CycleRepeat => {
                self.repeat = self.repeat.cycle();
                self.clear_predecoded();
                PlayerAction::None
            }
            PlayerCommand::ToggleFavorite => {
                if let Some(track) = self.current_track() {
                    let key = track.path_string();
                    let is_fav = self.favorites.toggle(&key);
                    tracing::info!(
                        "{} favorite: {key}",
                        if is_fav { "Added" } else { "Removed" }
                    );
                    if let Err(e) = self.favorites.save() {
                        tracing::warn!("Failed to save favorites: {e}");
                    }
                }
                PlayerAction::None
            }
            PlayerCommand::SetSleepTimer(minutes) => {
                let duration = Duration::from_secs(minutes * 60);
                self.sleep_timer = Some(SleepTimer {
                    end_time: Instant::now() + duration,
                    total_duration: duration,
                    fade_started: false,
                    original_volume_db: self.volume.0,
                });
                PlayerAction::None
            }
            PlayerCommand::CancelSleepTimer => {
                if let Some(ref timer) = self.sleep_timer
                    && timer.fade_started
                {
                    self.shared_volume
                        .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);
                }
                self.sleep_timer = None;
                PlayerAction::None
            }
            PlayerCommand::CycleSleepTimer => {
                let (next_minutes, was_fading) = match &self.sleep_timer {
                    None => (Some(SLEEP_PRESETS[0]), false),
                    Some(timer) => {
                        let current_minutes = timer.total_duration.as_secs() / 60;
                        let next = SLEEP_PRESETS
                            .iter()
                            .position(|&p| p == current_minutes)
                            .and_then(|idx| SLEEP_PRESETS.get(idx + 1).copied());
                        (next, timer.fade_started)
                    }
                };

                if was_fading {
                    self.shared_volume
                        .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);
                }

                match next_minutes {
                    Some(minutes) => {
                        let duration = Duration::from_secs(minutes * 60);
                        self.sleep_timer = Some(SleepTimer {
                            end_time: Instant::now() + duration,
                            total_duration: duration,
                            fade_started: false,
                            original_volume_db: self.volume.0,
                        });
                    }
                    None => {
                        self.sleep_timer = None;
                    }
                }
                PlayerAction::None
            }
            PlayerCommand::SpeedUp => {
                self.speed = speed::clamp_speed(self.speed + speed::SPEED_STEP);
                PlayerAction::None
            }
            PlayerCommand::SpeedDown => {
                self.speed = speed::clamp_speed(self.speed - speed::SPEED_STEP);
                PlayerAction::None
            }
            PlayerCommand::CycleEqPreset => {
                self.eq_preset_index = (self.eq_preset_index + 1) % eq::PRESETS.len();
                self.eq_preset = Some(&eq::PRESETS[self.eq_preset_index]);
                self.custom_gains = None;
                PlayerAction::None
            }
            PlayerCommand::EqBandUp => {
                let mut gains = self.eq_gains();
                gains[self.eq_selected_band] = (gains[self.eq_selected_band] + 1.0).min(12.0);
                self.custom_gains = Some(gains);
                PlayerAction::None
            }
            PlayerCommand::EqBandDown => {
                let mut gains = self.eq_gains();
                gains[self.eq_selected_band] = (gains[self.eq_selected_band] - 1.0).max(-12.0);
                self.custom_gains = Some(gains);
                PlayerAction::None
            }
            PlayerCommand::EqBandLeft => {
                if self.eq_selected_band > 0 {
                    self.eq_selected_band -= 1;
                }
                PlayerAction::None
            }
            PlayerCommand::EqBandRight => {
                if self.eq_selected_band < 9 {
                    self.eq_selected_band += 1;
                }
                PlayerAction::None
            }
            PlayerCommand::ToggleVisualizer | PlayerCommand::ToggleFullscreenVisualizer => {
                // Handled in the TUI layer — no playback state change needed.
                PlayerAction::None
            }
            PlayerCommand::Quit => PlayerAction::Quit,
        }
    }

    /// Get current playback position.
    pub fn current_position(&self) -> Duration {
        if self.state == PlaybackState::Playing
            && let Some(start) = self.playback_start
        {
            return self.pause_offset + start.elapsed();
        }
        self.pause_offset
    }

    /// Get current track duration.
    pub fn duration(&self) -> Duration {
        self.current_duration
    }

    /// Get current state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Get current track (if any).
    pub fn current_track(&self) -> Option<&Track> {
        self.tracks.get(self.current_index)
    }

    /// Get current track index and total count.
    pub fn queue_position(&self) -> (usize, usize) {
        (self.current_index + 1, self.tracks.len())
    }

    /// Get current volume in dB.
    pub fn volume_db(&self) -> f32 {
        self.volume.0
    }

    /// Get EQ preset name (if any).
    pub fn eq_preset_name(&self) -> Option<&str> {
        if self.custom_gains.is_some() {
            return None;
        }
        self.eq_preset.map(|p| p.name)
    }

    /// Get current EQ display name — preset name or "Custom" if gains were adjusted.
    pub fn eq_display_name(&self) -> &str {
        if self.custom_gains.is_some() {
            "Custom"
        } else {
            self.eq_preset.map(|p| p.name).unwrap_or("Off")
        }
    }

    /// Get current EQ gains — custom if set, else preset, else flat.
    pub fn eq_gains(&self) -> [f32; 10] {
        if let Some(gains) = self.custom_gains {
            gains
        } else if let Some(p) = self.eq_preset {
            p.gains
        } else {
            [0.0; 10]
        }
    }

    /// Get the currently selected EQ band index (0-9).
    pub fn eq_selected_band(&self) -> usize {
        self.eq_selected_band
    }

    /// Get a reference to the shared spectrum data for the visualizer.
    pub fn spectrum(&self) -> &Arc<SpectrumData> {
        &self.spectrum
    }

    /// Get the sample rate of the currently loaded track.
    #[allow(dead_code)]
    pub fn current_sample_rate(&self) -> u32 {
        self.current_sample_rate
    }

    /// Add a track to the queue and start playing it immediately.
    pub fn add_and_play(&mut self, track: Track) -> PlayerAction {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.clear_predecoded();
        self.tracks.push(track);
        self.current_index = self.tracks.len() - 1;
        PlayerAction::LoadAndPlay
    }

    /// Called when playback thread finishes naturally (track ended).
    pub fn on_track_finished(&mut self) -> PlayerAction {
        if self.state != PlaybackState::Playing {
            self.state = PlaybackState::Stopped;
            return PlayerAction::None;
        }

        if self.repeat == RepeatMode::One {
            return PlayerAction::LoadAndPlay;
        }

        if let Some(next) = self.next_index() {
            self.current_index = next;
            // Check if we have a pre-decoded track that matches
            if let Some(ref pre) = self.next_track_samples {
                if pre.track_index == self.current_index
                    && pre.sample_rate == self.current_sample_rate
                    && pre.channels == self.current_channels
                {
                    return PlayerAction::GaplessTransition;
                } else if pre.track_index == self.current_index {
                    tracing::info!(
                        "Gapless not possible: format mismatch ({}Hz/{}ch vs {}Hz/{}ch)",
                        self.current_sample_rate,
                        self.current_channels,
                        pre.sample_rate,
                        pre.channels,
                    );
                }
            }
            PlayerAction::LoadAndPlay
        } else if self.repeat == RepeatMode::All && !self.tracks.is_empty() {
            // Wrap to beginning (or first in shuffle order)
            self.current_index = if self.shuffle {
                self.shuffle_order.first().copied().unwrap_or(0)
            } else {
                0
            };
            // Check gapless for wrap-around too
            if let Some(ref pre) = self.next_track_samples
                && pre.track_index == self.current_index
                && pre.sample_rate == self.current_sample_rate
                && pre.channels == self.current_channels
            {
                return PlayerAction::GaplessTransition;
            }
            PlayerAction::LoadAndPlay
        } else {
            self.state = PlaybackState::Stopped;
            PlayerAction::None
        }
    }

    /// Set playback start time (called after thread spawn).
    pub fn mark_playback_started(&mut self) {
        self.playback_start = Some(Instant::now());
    }

    /// Return playback progress as a ratio (0.0–1.0).
    pub fn playback_progress(&self) -> f64 {
        let duration = self.current_duration.as_secs_f64();
        if duration <= 0.0 {
            return 0.0;
        }
        (self.current_position().as_secs_f64() / duration).min(1.0)
    }

    /// Pre-decode the next track in the queue for gapless playback.
    ///
    /// Decodes the next track (respecting shuffle/repeat) and stores it in
    /// `next_track_samples`. Applies EQ if active. No-op if the queue is empty,
    /// there's no next track, or a track is already pre-decoded.
    pub fn pre_decode_next(&mut self) -> Result<()> {
        // Already pre-decoded
        if self.next_track_samples.is_some() {
            return Ok(());
        }

        let next_idx = self.resolve_next_index_for_predecode();
        let next_idx = match next_idx {
            Some(idx) => idx,
            None => return Ok(()),
        };

        let track = match self.tracks.get(next_idx) {
            Some(t) => t,
            None => return Ok(()),
        };

        // Only pre-decode local files — streaming URLs may be live/infinite
        let decoded = match &track.source {
            TrackSource::File(p) => match decoder::decode_file(p) {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("Failed to pre-decode next track: {e}");
                    return Ok(());
                }
            },
            TrackSource::Url(_) => return Ok(()),
        };

        let mut samples = decoded.samples;

        // Apply ReplayGain before EQ and speed
        if self.replaygain_mode != ReplayGainMode::Off
            && let TrackSource::File(p) = &self.tracks[next_idx].source
        {
            let rg_info = replaygain::read_replaygain(p);
            if let Some(gain_db) = replaygain::gain_to_apply(&rg_info, self.replaygain_mode) {
                replaygain::apply_replaygain(&mut samples, gain_db);
                tracing::info!("Pre-decode ReplayGain: {gain_db:+.1}dB");
            }
        }

        let samples = self.apply_eq_to_samples(samples, decoded.sample_rate, decoded.channels);
        let samples = speed::apply_speed(&samples, decoded.channels, self.speed);

        tracing::info!(
            "Pre-decoded next track [{}] ({}Hz, {}ch)",
            next_idx,
            decoded.sample_rate,
            decoded.channels
        );

        self.next_track_samples = Some(PreDecodedTrack {
            samples,
            sample_rate: decoded.sample_rate,
            channels: decoded.channels,
            track_index: next_idx,
        });

        Ok(())
    }

    /// Whether a pre-decoded next track is available and compatible for gapless.
    #[allow(dead_code)] // Used by tests and future true-gapless path
    pub fn has_gapless_next(&self) -> bool {
        match &self.next_track_samples {
            Some(pre) => {
                pre.sample_rate == self.current_sample_rate && pre.channels == self.current_channels
            }
            None => false,
        }
    }

    /// Take the pre-decoded next track samples (consuming them).
    pub fn take_next_track_samples(&mut self) -> Option<PreDecodedTrack> {
        self.next_track_samples.take()
    }

    /// Resolve the next track index for pre-decoding (accounts for repeat/shuffle).
    fn resolve_next_index_for_predecode(&self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.repeat == RepeatMode::One {
            return Some(self.current_index);
        }
        if let Some(next) = self.next_index() {
            return Some(next);
        }
        if self.repeat == RepeatMode::All && !self.tracks.is_empty() {
            return Some(if self.shuffle {
                self.shuffle_order.first().copied().unwrap_or(0)
            } else {
                0
            });
        }
        None
    }

    /// Apply EQ processing to samples based on current preset/custom gains.
    fn apply_eq_to_samples(
        &self,
        samples: Vec<f32>,
        sample_rate: u32,
        channels: usize,
    ) -> Vec<f32> {
        if let Some(gains) = self.custom_gains {
            let mut eq = Equalizer::new(sample_rate, channels);
            eq.set_gains(gains);
            let mut buf = samples;
            eq.process(&mut buf, channels);
            buf
        } else if let Some(p) = self.eq_preset {
            let mut eq = Equalizer::new(sample_rate, channels);
            eq.apply_preset(p);
            let mut buf = samples;
            eq.process(&mut buf, channels);
            buf
        } else {
            samples
        }
    }

    /// Invalidate pre-decoded samples (e.g. when the user skips tracks manually).
    pub fn clear_predecoded(&mut self) {
        self.next_track_samples = None;
    }

    /// Get the track list for display.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Get current track index (0-based).
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Whether shuffle is enabled.
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Current playback speed multiplier.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Current ReplayGain mode.
    pub fn replaygain_mode(&self) -> ReplayGainMode {
        self.replaygain_mode
    }

    /// ReplayGain info for the currently playing track.
    pub fn current_replaygain(&self) -> Option<&ReplayGainInfo> {
        self.current_rg.as_ref()
    }

    /// Current repeat mode.
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Return the next track index respecting shuffle order, or `None` at end.
    fn next_index(&self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.shuffle && !self.shuffle_order.is_empty() {
            let pos = self
                .shuffle_order
                .iter()
                .position(|&i| i == self.current_index)
                .unwrap_or(0);
            if pos + 1 < self.shuffle_order.len() {
                Some(self.shuffle_order[pos + 1])
            } else {
                None
            }
        } else if self.current_index + 1 < self.tracks.len() {
            Some(self.current_index + 1)
        } else {
            None
        }
    }

    /// Return the previous track index respecting shuffle order, or `None` at start.
    fn prev_index(&self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.shuffle && !self.shuffle_order.is_empty() {
            let pos = self
                .shuffle_order
                .iter()
                .position(|&i| i == self.current_index)
                .unwrap_or(0);
            if pos > 0 {
                Some(self.shuffle_order[pos - 1])
            } else {
                None
            }
        } else if self.current_index > 0 {
            Some(self.current_index - 1)
        } else {
            None
        }
    }

    /// Check whether the currently playing track is favorited.
    pub fn is_current_favorited(&self) -> bool {
        self.current_track()
            .map(|t| self.favorites.contains(&t.path_string()))
            .unwrap_or(false)
    }

    /// Get a reference to the favorites store.
    pub fn favorites(&self) -> &Favorites {
        &self.favorites
    }

    /// Build a [`Session`] from current player state and save it to disk.
    pub fn save_session(&self) -> Result<()> {
        let session = Session {
            track_path: self.current_track().map(|t| t.path_string()),
            position_ms: self.current_position().as_millis() as u64,
            queue: self.tracks.iter().map(|t| t.path_string()).collect(),
            queue_index: self.current_index,
            volume_db: self.volume.0,
            eq_preset: self.eq_preset_name().map(|s| s.to_string()),
            shuffle: self.shuffle,
            repeat: self.repeat.to_string(),
            speed: self.speed,
        };
        session.save(&Session::session_path())
    }

    /// Apply a saved session: match track by path, restore queue index,
    /// volume, and EQ preset. Returns `true` if a matching track was found.
    pub fn restore_session(&mut self, session: &Session) -> bool {
        self.volume = Volume(session.volume_db);

        if let Some(ref preset_name) = session.eq_preset {
            if let Some(preset) = eq::find_preset(preset_name) {
                self.eq_preset = Some(preset);
            } else {
                tracing::warn!("Saved EQ preset '{preset_name}' not found, ignoring");
            }
        }

        self.shuffle = session.shuffle;
        self.speed = speed::clamp_speed(session.speed);
        self.repeat = match session.repeat.as_str() {
            "All" => RepeatMode::All,
            "One" => RepeatMode::One,
            _ => RepeatMode::Off,
        };
        if self.shuffle {
            self.shuffle_order = generate_shuffle_order(self.tracks.len());
        }

        if let Some(ref track_path) = session.track_path {
            if let Some(idx) = self
                .tracks
                .iter()
                .position(|t| t.path_string() == *track_path)
            {
                self.current_index = idx;
                return true;
            }
            tracing::warn!(
                "Saved track '{track_path}' not found in queue, starting from beginning"
            );
        }
        false
    }

    /// Tick the sleep timer. Call every frame from the TUI event loop.
    pub fn tick_sleep_timer(&mut self) -> SleepAction {
        let (end_time, original_volume_db) = match &self.sleep_timer {
            Some(t) => (t.end_time, t.original_volume_db),
            None => return SleepAction::None,
        };

        let now = Instant::now();
        if now >= end_time {
            // Timer expired — restore volume and clear
            self.shared_volume
                .store(self.volume.as_linear().to_bits(), Ordering::Relaxed);
            self.sleep_timer = None;
            return SleepAction::Stop;
        }

        let remaining = end_time - now;
        let remaining_secs = remaining.as_secs();

        if remaining < FADE_DURATION {
            if let Some(ref mut timer) = self.sleep_timer {
                timer.fade_started = true;
            }

            let fade_progress = 1.0 - (remaining.as_secs_f32() / FADE_DURATION.as_secs_f32());
            let target_db = original_volume_db + (-30.0 - original_volume_db) * fade_progress;
            let linear = 10.0_f32.powf(target_db / 20.0);
            self.shared_volume
                .store(linear.to_bits(), Ordering::Relaxed);

            SleepAction::Fading(remaining_secs)
        } else {
            SleepAction::Active(remaining_secs)
        }
    }

    /// Get remaining time on the sleep timer, if any.
    pub fn sleep_remaining(&self) -> Option<Duration> {
        self.sleep_timer.as_ref().map(|t| {
            let now = Instant::now();
            if now >= t.end_time {
                Duration::ZERO
            } else {
                t.end_time - now
            }
        })
    }

    /// Whether the sleep timer is currently fading out.
    pub fn is_sleep_fading(&self) -> bool {
        self.sleep_timer.as_ref().is_some_and(|t| t.fade_started)
    }
}

/// Action the TUI should take after handling a command.
pub enum PlayerAction {
    None,
    LoadAndPlay,
    /// Transition to the next track gaplessly using pre-decoded samples.
    GaplessTransition,
    Quit,
}

/// Generate a shuffled index order using a simple LCG PRNG.
fn generate_shuffle_order(len: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    if len <= 1 {
        return order;
    }
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng = seed;
    for i in (1..order.len()).rev() {
        rng = rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = (rng as usize) % (i + 1);
        order.swap(i, j);
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_mode_cycles_correctly() {
        assert_eq!(RepeatMode::Off.cycle(), RepeatMode::All);
        assert_eq!(RepeatMode::All.cycle(), RepeatMode::One);
        assert_eq!(RepeatMode::One.cycle(), RepeatMode::Off);
    }

    #[test]
    fn repeat_mode_display() {
        assert_eq!(RepeatMode::Off.to_string(), "Off");
        assert_eq!(RepeatMode::All.to_string(), "All");
        assert_eq!(RepeatMode::One.to_string(), "One");
    }

    #[test]
    fn shuffle_order_is_valid_permutation() {
        let order = generate_shuffle_order(10);
        assert_eq!(order.len(), 10);
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_order_empty_and_single() {
        let empty = generate_shuffle_order(0);
        assert!(empty.is_empty());

        let single = generate_shuffle_order(1);
        assert_eq!(single, vec![0]);
    }

    fn test_player() -> Player {
        Player::new(vec![], 0.0, None, ReplayGainMode::Off).unwrap()
    }

    #[test]
    fn sleep_timer_remaining_approximately_60s() {
        let mut player = test_player();
        player.handle_command(PlayerCommand::SetSleepTimer(1));
        let remaining = player.sleep_remaining().expect("timer should be set");
        assert!(
            remaining.as_secs() >= 59 && remaining.as_secs() <= 60,
            "expected ~60s, got {}s",
            remaining.as_secs()
        );
    }

    #[test]
    fn tick_returns_active_when_timer_set() {
        let mut player = test_player();
        player.handle_command(PlayerCommand::SetSleepTimer(1));
        match player.tick_sleep_timer() {
            SleepAction::Active(secs) => assert!(secs >= 29, "expected >= 29s, got {secs}s"),
            other => panic!("expected Active, got {other:?}"),
        }
    }

    #[test]
    fn tick_returns_stop_when_timer_expired() {
        let mut player = test_player();
        player.sleep_timer = Some(SleepTimer {
            end_time: Instant::now(),
            total_duration: Duration::from_secs(60),
            fade_started: false,
            original_volume_db: 0.0,
        });
        std::thread::sleep(Duration::from_millis(10));
        assert!(matches!(player.tick_sleep_timer(), SleepAction::Stop));
        assert!(player.sleep_timer.is_none());
    }

    #[test]
    fn cancel_clears_timer() {
        let mut player = test_player();
        player.handle_command(PlayerCommand::SetSleepTimer(15));
        assert!(player.sleep_timer.is_some());
        player.handle_command(PlayerCommand::CancelSleepTimer);
        assert!(player.sleep_timer.is_none());
    }

    #[test]
    fn cycle_presets() {
        let mut player = test_player();

        // Off → 15
        player.handle_command(PlayerCommand::CycleSleepTimer);
        assert_eq!(
            player.sleep_timer.as_ref().unwrap().total_duration,
            Duration::from_secs(15 * 60)
        );

        // 15 → 30
        player.handle_command(PlayerCommand::CycleSleepTimer);
        assert_eq!(
            player.sleep_timer.as_ref().unwrap().total_duration,
            Duration::from_secs(30 * 60)
        );

        // 30 → 45
        player.handle_command(PlayerCommand::CycleSleepTimer);
        assert_eq!(
            player.sleep_timer.as_ref().unwrap().total_duration,
            Duration::from_secs(45 * 60)
        );

        // 45 → 60
        player.handle_command(PlayerCommand::CycleSleepTimer);
        assert_eq!(
            player.sleep_timer.as_ref().unwrap().total_duration,
            Duration::from_secs(60 * 60)
        );

        // 60 → 90
        player.handle_command(PlayerCommand::CycleSleepTimer);
        assert_eq!(
            player.sleep_timer.as_ref().unwrap().total_duration,
            Duration::from_secs(90 * 60)
        );

        // 90 → Off
        player.handle_command(PlayerCommand::CycleSleepTimer);
        assert!(player.sleep_timer.is_none());
    }

    #[test]
    fn tick_returns_fading_in_last_30s() {
        let mut player = test_player();
        player.sleep_timer = Some(SleepTimer {
            end_time: Instant::now() + Duration::from_secs(15),
            total_duration: Duration::from_secs(60),
            fade_started: false,
            original_volume_db: 0.0,
        });
        match player.tick_sleep_timer() {
            SleepAction::Fading(secs) => assert!(secs <= 15, "expected <= 15s, got {secs}s"),
            other => panic!("expected Fading, got {other:?}"),
        }
        assert!(player.is_sleep_fading());
    }

    #[test]
    fn cancel_during_fade_restores_volume() {
        let mut player = test_player();
        let original_bits = player.shared_volume.load(Ordering::Relaxed);

        // Put player in fading state
        player.sleep_timer = Some(SleepTimer {
            end_time: Instant::now() + Duration::from_secs(10),
            total_duration: Duration::from_secs(60),
            fade_started: false,
            original_volume_db: 0.0,
        });
        // Tick to start fade (modifies shared_volume)
        player.tick_sleep_timer();
        let faded_bits = player.shared_volume.load(Ordering::Relaxed);
        assert_ne!(faded_bits, original_bits, "fade should have changed volume");

        // Cancel should restore volume
        player.handle_command(PlayerCommand::CancelSleepTimer);
        let restored_bits = player.shared_volume.load(Ordering::Relaxed);
        assert_eq!(restored_bits, original_bits);
    }

    #[test]
    fn eq_cycle_preset() {
        let mut player = test_player();
        assert_eq!(player.eq_display_name(), "Off");

        player.handle_command(PlayerCommand::CycleEqPreset);
        // Starts at index 0, cycles to index 1 (Rock)
        assert_eq!(player.eq_display_name(), "Rock");

        player.handle_command(PlayerCommand::CycleEqPreset);
        assert_eq!(player.eq_display_name(), "Pop");
    }

    #[test]
    fn eq_band_navigation() {
        let mut player = test_player();
        assert_eq!(player.eq_selected_band(), 0);

        player.handle_command(PlayerCommand::EqBandRight);
        assert_eq!(player.eq_selected_band(), 1);

        player.handle_command(PlayerCommand::EqBandRight);
        assert_eq!(player.eq_selected_band(), 2);

        player.handle_command(PlayerCommand::EqBandLeft);
        assert_eq!(player.eq_selected_band(), 1);

        // Clamp at 0
        player.handle_command(PlayerCommand::EqBandLeft);
        player.handle_command(PlayerCommand::EqBandLeft);
        assert_eq!(player.eq_selected_band(), 0);
    }

    #[test]
    fn eq_band_clamp_at_9() {
        let mut player = test_player();
        for _ in 0..20 {
            player.handle_command(PlayerCommand::EqBandRight);
        }
        assert_eq!(player.eq_selected_band(), 9);
    }

    #[test]
    fn eq_band_gain_adjust() {
        let mut player = test_player();
        assert_eq!(player.eq_gains(), [0.0; 10]);

        player.handle_command(PlayerCommand::EqBandUp);
        let gains = player.eq_gains();
        assert_eq!(gains[0], 1.0);
        assert_eq!(gains[1], 0.0);

        player.handle_command(PlayerCommand::EqBandDown);
        player.handle_command(PlayerCommand::EqBandDown);
        let gains = player.eq_gains();
        assert_eq!(gains[0], -1.0);
    }

    #[test]
    fn eq_gain_clamp_at_12() {
        let mut player = test_player();
        for _ in 0..20 {
            player.handle_command(PlayerCommand::EqBandUp);
        }
        assert_eq!(player.eq_gains()[0], 12.0);

        for _ in 0..30 {
            player.handle_command(PlayerCommand::EqBandDown);
        }
        assert_eq!(player.eq_gains()[0], -12.0);
    }

    #[test]
    fn eq_custom_gains_override_preset() {
        let mut player = test_player();
        player.handle_command(PlayerCommand::CycleEqPreset); // Go to Rock
        assert_eq!(player.eq_display_name(), "Rock");

        // Adjusting a band makes it "Custom"
        player.handle_command(PlayerCommand::EqBandUp);
        assert_eq!(player.eq_display_name(), "Custom");
        assert!(player.eq_preset_name().is_none());

        // Cycling preset resets custom
        player.handle_command(PlayerCommand::CycleEqPreset);
        assert_ne!(player.eq_display_name(), "Custom");
    }

    // --- Gapless playback tests ---

    fn test_player_with_tracks(count: usize) -> Player {
        let tracks: Vec<Track> = (0..count)
            .map(|i| Track::from_file(std::path::PathBuf::from(format!("test_track_{i}.mp3"))))
            .collect();
        Player::new(tracks, 0.0, None, ReplayGainMode::Off).unwrap()
    }

    #[test]
    fn pre_decode_no_next_track_is_noop() {
        // Single track, no next → pre_decode_next should be a no-op
        let mut player = test_player_with_tracks(1);
        let result = player.pre_decode_next();
        assert!(result.is_ok());
        assert!(player.next_track_samples.is_none());
    }

    #[test]
    fn pre_decode_empty_queue_is_noop() {
        let mut player = test_player();
        let result = player.pre_decode_next();
        assert!(result.is_ok());
        assert!(player.next_track_samples.is_none());
    }

    #[test]
    fn pre_decode_already_decoded_is_noop() {
        let mut player = test_player_with_tracks(3);
        // Manually set pre-decoded data
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        });
        // Should not overwrite existing pre-decoded data
        let result = player.pre_decode_next();
        assert!(result.is_ok());
        assert!(player.next_track_samples.is_some());
        assert_eq!(player.next_track_samples.as_ref().unwrap().track_index, 1);
    }

    #[test]
    fn gapless_fallback_different_sample_rate() {
        let mut player = test_player_with_tracks(2);
        player.current_sample_rate = 44100;
        player.current_channels = 2;
        // Set up pre-decoded with different sample rate
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 48000,
            channels: 2,
            track_index: 1,
        });
        assert!(
            !player.has_gapless_next(),
            "Gapless should not be possible with different sample rates"
        );
    }

    #[test]
    fn gapless_fallback_different_channels() {
        let mut player = test_player_with_tracks(2);
        player.current_sample_rate = 44100;
        player.current_channels = 2;
        // Set up pre-decoded with different channel count
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 1,
            track_index: 1,
        });
        assert!(
            !player.has_gapless_next(),
            "Gapless should not be possible with different channel counts"
        );
    }

    #[test]
    fn gapless_compatible_formats() {
        let mut player = test_player_with_tracks(2);
        player.current_sample_rate = 44100;
        player.current_channels = 2;
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        });
        assert!(
            player.has_gapless_next(),
            "Gapless should work with matching formats"
        );
    }

    #[test]
    fn on_track_finished_returns_gapless_transition() {
        let mut player = test_player_with_tracks(3);
        player.state = PlaybackState::Playing;
        player.current_sample_rate = 44100;
        player.current_channels = 2;
        // Pre-decode track 1 (next track)
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.5; 200],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        });
        let action = player.on_track_finished();
        assert!(
            matches!(action, PlayerAction::GaplessTransition),
            "Should return GaplessTransition when pre-decoded data matches"
        );
        assert_eq!(player.current_index, 1);
    }

    #[test]
    fn on_track_finished_falls_back_on_format_mismatch() {
        let mut player = test_player_with_tracks(3);
        player.state = PlaybackState::Playing;
        player.current_sample_rate = 44100;
        player.current_channels = 2;
        // Pre-decode track 1 but with different sample rate
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.5; 200],
            sample_rate: 48000,
            channels: 2,
            track_index: 1,
        });
        let action = player.on_track_finished();
        assert!(
            matches!(action, PlayerAction::LoadAndPlay),
            "Should fall back to LoadAndPlay on format mismatch"
        );
    }

    #[test]
    fn play_predecoded_loads_correctly() {
        let mut player = test_player_with_tracks(2);
        let pre = PreDecodedTrack {
            samples: vec![0.5; 88200],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        };
        player.play_predecoded(pre);
        assert_eq!(player.current_sample_rate, 44100);
        assert_eq!(player.current_channels, 2);
        assert!(player.current_samples.is_some());
        assert_eq!(player.current_samples.as_ref().unwrap().len(), 88200);
        assert_eq!(player.state(), PlaybackState::Playing);
        assert!(player.duration().as_secs_f64() > 0.0);
    }

    #[test]
    fn clear_predecoded_removes_data() {
        let mut player = test_player_with_tracks(2);
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        });
        player.clear_predecoded();
        assert!(player.next_track_samples.is_none());
    }

    #[test]
    fn manual_next_clears_predecoded() {
        let mut player = test_player_with_tracks(3);
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        });
        player.handle_command(PlayerCommand::NextTrack);
        assert!(player.next_track_samples.is_none());
    }

    #[test]
    fn playback_progress_zero_when_stopped() {
        let player = test_player();
        assert_eq!(player.playback_progress(), 0.0);
    }

    #[test]
    fn take_next_track_samples_consumes() {
        let mut player = test_player_with_tracks(2);
        player.next_track_samples = Some(PreDecodedTrack {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
            track_index: 1,
        });
        let taken = player.take_next_track_samples();
        assert!(taken.is_some());
        assert!(player.next_track_samples.is_none());
    }
}
