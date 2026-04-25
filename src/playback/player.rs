//! Playback controller — shared state between the TUI and audio pipeline.
//!
//! The controller owns the playback queue and manages the lifecycle of
//! decode → ring buffer → CPAL output for each track. The TUI sends
//! commands (play, pause, next, seek) and reads state (position, duration,
//! current track) without blocking the audio pipeline.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::backend::cpal_backend;
use crate::core::session::Session;
use crate::core::track::{Track, TrackSource};
use crate::core::types::Volume;
use crate::playback::decoder;
use crate::playback::eq::{self, EqPreset, Equalizer};

/// Playback state visible to the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
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
    Quit,
}

/// Shared playback position updated by the producer thread.
struct SharedPosition {
    samples_played: AtomicU64,
    total_samples: AtomicU64,
}

/// The playback controller manages the queue and audio pipeline.
pub struct Player {
    tracks: Vec<Track>,
    current_index: usize,
    volume: Volume,
    state: PlaybackState,
    eq_preset: Option<&'static EqPreset>,

    // Current track playback
    current_samples: Option<Vec<f32>>,
    current_sample_rate: u32,
    current_channels: usize,
    current_duration: Duration,
    playback_position: Duration,
    playback_start: Option<Instant>,
    pause_offset: Duration,

    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    shared_volume: Arc<AtomicU32>,
    position: Arc<SharedPosition>,
}

impl Player {
    pub fn new(tracks: Vec<Track>, volume_db: f32, eq_preset: Option<&str>) -> Result<Self> {
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

        Ok(Self {
            tracks,
            current_index: 0,
            volume: Volume(volume_db),
            state: PlaybackState::Stopped,
            eq_preset: preset,
            current_samples: None,
            current_sample_rate: 44100,
            current_channels: 2,
            current_duration: Duration::ZERO,
            playback_position: Duration::ZERO,
            playback_start: None,
            pause_offset: Duration::ZERO,
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
        let path = match &track.source {
            TrackSource::File(p) => p.clone(),
            TrackSource::Url(_) => {
                tracing::warn!("URL playback not yet supported");
                return Ok(());
            }
        };

        let decoded = decoder::decode_file(&path)
            .with_context(|| format!("Failed to decode {}", path.display()))?;

        let samples = if let Some(p) = self.eq_preset {
            let mut eq = Equalizer::new(decoded.sample_rate, decoded.channels);
            eq.apply_preset(p);
            let mut buf = decoded.samples;
            eq.process(&mut buf, decoded.channels);
            buf
        } else {
            decoded.samples
        };

        let total_samples = samples.len() as u64;
        let sample_rate = decoded.sample_rate;
        let channels = decoded.channels;

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

        let handle = std::thread::spawn(move || {
            cpal_backend::play_audio_with_position(
                &samples,
                sample_rate,
                channels,
                &volume,
                &stop,
                &pause,
                &position.samples_played,
            )
        });

        Some(handle)
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
                PlayerAction::None
            }
            PlayerCommand::NextTrack => {
                self.stop_flag.store(true, Ordering::Relaxed);
                if self.current_index + 1 < self.tracks.len() {
                    self.current_index += 1;
                    PlayerAction::LoadAndPlay
                } else {
                    self.state = PlaybackState::Stopped;
                    PlayerAction::None
                }
            }
            PlayerCommand::PrevTrack => {
                self.stop_flag.store(true, Ordering::Relaxed);
                // If more than 3 seconds in, restart current track
                if self.current_position().as_secs() > 3 || self.current_index == 0 {
                    PlayerAction::LoadAndPlay
                } else {
                    self.current_index -= 1;
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
        self.eq_preset.map(|p| p.name)
    }

    /// Called when playback thread finishes naturally (track ended).
    pub fn on_track_finished(&mut self) -> PlayerAction {
        if self.state == PlaybackState::Playing && self.current_index + 1 < self.tracks.len() {
            self.current_index += 1;
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

    /// Get the track list for display.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Get current track index (0-based).
    pub fn current_index(&self) -> usize {
        self.current_index
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
}

/// Action the TUI should take after handling a command.
pub enum PlayerAction {
    None,
    LoadAndPlay,
    Quit,
}
