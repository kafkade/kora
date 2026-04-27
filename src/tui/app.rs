//! TUI application — ratatui event loop, rendering, and input handling.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use super::file_browser::FileBrowser;
use super::podcast_view::{InputMode, PodcastView, PodcastViewMode};
use super::theme::{self, Theme};
use crate::core::session::Session;
use crate::core::track::Track;
use crate::playback::lyrics;
use crate::playback::player::{PlaybackState, Player, PlayerAction, PlayerCommand, SleepAction};

/// Block characters for fine-grained bar height rendering (1/8 increments).
const BAR_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Visualizer display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisualizerMode {
    Off,
    Normal,
    Fullscreen,
}

/// Run the TUI application.
#[allow(dead_code)]
pub fn run(player: Player, no_restore: bool) -> Result<()> {
    run_with_theme(player, no_restore, None)
}

/// Run the TUI application with an optional initial theme name.
pub fn run_with_theme(
    mut player: Player,
    no_restore: bool,
    theme_name: Option<&str>,
) -> Result<()> {
    // Session restore
    let mut restored = false;
    if !no_restore {
        let session_path = Session::session_path();
        match Session::load(&session_path) {
            Ok(session) if session.track_path.is_some() => {
                let found = player.restore_session(&session);
                if found {
                    tracing::info!("Session restored");
                    restored = true;
                }
            }
            Ok(_) => {} // empty/default session
            Err(e) => tracing::warn!("Failed to load session: {e}"),
        }
    }

    // Load and start playing the current track
    player.play_current()?;
    let mut playback_handle = player.start_playback();
    player.mark_playback_started();

    // If we restored a session, start paused so the user can resume manually
    if restored {
        let _ = player.handle_command(PlayerCommand::PlayPause);
    }

    // Enter alternate screen
    io::stdout().execute(EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let theme = match theme_name {
        Some(name) => theme::find_theme(name).unwrap_or_else(Theme::default_theme),
        None => Theme::default_theme(),
    };
    let themes = theme::all_themes();
    let mut theme_index = themes
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(theme.name))
        .unwrap_or(0);
    let mut theme = themes[theme_index].clone();
    let mut terminal = ratatui::init();

    // Start IPC server for remote control
    let ipc_stop = Arc::new(AtomicBool::new(false));
    let ipc_rx = match crate::ipc::server::start_ipc_server(ipc_stop.clone()) {
        Ok(rx) => Some(rx),
        Err(e) => {
            tracing::warn!("Failed to start IPC server: {e}");
            None
        }
    };

    // Initialize OS media controls (MPRIS/SMTC/MediaRemote)
    #[cfg(feature = "media-controls")]
    let mut media_controls = crate::media_controls::init_media_controls();

    let result = run_loop(
        &mut terminal,
        &mut player,
        &mut playback_handle,
        &mut theme,
        &themes,
        &mut theme_index,
        #[cfg(feature = "media-controls")]
        &mut media_controls,
        ipc_rx.as_ref(),
    );

    // Stop IPC server
    ipc_stop.store(true, Ordering::Relaxed);

    // Restore terminal
    ratatui::restore();
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

#[allow(clippy::too_many_arguments)]
fn run_loop(
    terminal: &mut DefaultTerminal,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
    theme: &mut Theme,
    themes: &[Theme],
    theme_index: &mut usize,
    #[cfg(feature = "media-controls")] media_controls: &mut Option<(
        souvlaki::MediaControls,
        std::sync::mpsc::Receiver<souvlaki::MediaControlEvent>,
    )>,
    ipc_rx: Option<&std::sync::mpsc::Receiver<crate::ipc::server::IpcMessage>>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let save_interval = Duration::from_secs(30);
    let mut last_save = Instant::now();
    let mut file_browser: Option<FileBrowser> = None;
    let mut podcast_view: Option<PodcastView> = None;
    let mut show_eq = false;
    let mut show_lyrics = false;
    let mut visualizer_mode = VisualizerMode::Normal;
    let mut status_message: Option<(String, Instant)> = None;

    // Gapless playback state
    let mut gapless_played_next: Option<Arc<AtomicBool>> = None;
    let mut pre_decode_triggered = false;

    // Track the last known track index + state for media controls updates
    #[cfg(feature = "media-controls")]
    let mut last_media_track_index: Option<usize> = None;
    #[cfg(feature = "media-controls")]
    let mut last_media_playing: Option<bool> = None;

    // Send initial metadata/playback state to OS media controls
    #[cfg(feature = "media-controls")]
    sync_media_controls(
        player,
        media_controls,
        &mut last_media_track_index,
        &mut last_media_playing,
    );

    loop {
        // Draw
        terminal.draw(|frame| {
            draw(
                frame,
                player,
                theme,
                file_browser.as_ref(),
                podcast_view.as_ref(),
                show_eq,
                show_lyrics,
                visualizer_mode,
                status_message.as_ref().map(|(msg, _)| msg.as_str()),
            )
        })?;

        // Clear expired status messages (show for 3 seconds)
        if let Some((_, time)) = &status_message
            && time.elapsed() > Duration::from_secs(3)
        {
            status_message = None;
        }

        // Tick sleep timer
        if let SleepAction::Stop = player.tick_sleep_timer() {
            let action = player.handle_command(PlayerCommand::Stop);
            handle_action(
                action,
                player,
                playback_handle,
                &mut gapless_played_next,
                &mut pre_decode_triggered,
            )?;
        }

        // Check if the gapless next-track was played — advance player state
        if let Some(ref flag) = gapless_played_next
            && flag.load(Ordering::Relaxed)
        {
            // The audio thread transitioned to the next track
            let action = player.on_track_finished();
            // on_track_finished already advanced current_index; if it returns
            // GaplessTransition the samples were already consumed by the audio
            // thread, so we just update player metadata from the pre-decoded data.
            match action {
                PlayerAction::GaplessTransition => {
                    if let Some(pre) = player.take_next_track_samples() {
                        player.play_predecoded(pre);
                        // No new playback thread — audio thread is still running
                    }
                }
                _ => {
                    // Unexpected — the gapless path should return GaplessTransition
                    // but handle gracefully
                    handle_action(
                        action,
                        player,
                        playback_handle,
                        &mut gapless_played_next,
                        &mut pre_decode_triggered,
                    )?;
                }
            }
            gapless_played_next = None;
            pre_decode_triggered = false;
        }

        // Check if playback thread finished (track ended naturally)
        if let Some(handle) = playback_handle.as_ref()
            && handle.is_finished()
        {
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            gapless_played_next = None;
            pre_decode_triggered = false;
            let action = player.on_track_finished();
            handle_action(
                action,
                player,
                playback_handle,
                &mut gapless_played_next,
                &mut pre_decode_triggered,
            )?;
        }

        // Pre-decode next track when past 80% of current track
        if player.state() == PlaybackState::Playing
            && !pre_decode_triggered
            && player.playback_progress() > 0.8
        {
            pre_decode_triggered = true;
            if let Err(e) = player.pre_decode_next() {
                tracing::warn!("Pre-decode failed: {e}");
            }
        }

        // Auto-save session every 30 seconds
        if last_save.elapsed() >= save_interval {
            if let Err(e) = player.save_session() {
                tracing::warn!("Failed to auto-save session: {e}");
            }
            last_save = Instant::now();
        }

        // Process OS media key events (MPRIS / SMTC / MediaRemote)
        #[cfg(feature = "media-controls")]
        if let Some((controls, rx)) = media_controls.as_mut() {
            while let Ok(event) = rx.try_recv() {
                if let Some(cmd) = crate::media_controls::map_media_event(&event) {
                    let action = player.handle_command(cmd);
                    if matches!(action, PlayerAction::Quit) {
                        if let Err(e) = player.save_session() {
                            tracing::warn!("Failed to save session on quit: {e}");
                        }
                        return Ok(());
                    }
                    handle_action(
                        action,
                        player,
                        playback_handle,
                        &mut gapless_played_next,
                        &mut pre_decode_triggered,
                    )?;
                }
            }
            let _ = controls;
        }

        // Sync media controls state after any changes
        #[cfg(feature = "media-controls")]
        sync_media_controls(
            player,
            media_controls,
            &mut last_media_track_index,
            &mut last_media_playing,
        );

        // Process IPC remote control messages
        if let Some(rx) = ipc_rx {
            while let Ok(msg) = rx.try_recv() {
                let response = handle_ipc_request(
                    &msg.request,
                    player,
                    playback_handle,
                    &mut gapless_played_next,
                    &mut pre_decode_triggered,
                );
                let _ = msg.response_tx.send(response);
            }
        }

        // Poll for keyboard input
        if event::poll(tick_rate)?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // File browser input handling (takes priority when open)
            if file_browser.is_some() {
                let mut close_browser = false;
                let mut selected_path = None;

                if let Some(ref mut browser) = file_browser {
                    match key.code {
                        KeyCode::Esc => close_browser = true,
                        KeyCode::Char('j') | KeyCode::Down => browser.navigate_down(),
                        KeyCode::Char('k') | KeyCode::Up => browser.select_previous(),
                        KeyCode::Enter => {
                            selected_path = browser.navigate_into();
                            if selected_path.is_some() {
                                close_browser = true;
                            }
                        }
                        KeyCode::Backspace | KeyCode::Char('h') => browser.navigate_up(),
                        _ => {}
                    }
                }

                if close_browser {
                    file_browser = None;
                }

                if let Some(path) = selected_path {
                    let track = Track::from_file(path);
                    let action = player.add_and_play(track);
                    handle_action(
                        action,
                        player,
                        playback_handle,
                        &mut gapless_played_next,
                        &mut pre_decode_triggered,
                    )?;
                }

                continue;
            }

            // Podcast view input handling (takes priority when open)
            if podcast_view.is_some() {
                let mut close_podcast = false;
                let mut play_episode = false;

                if let Some(ref mut pv) = podcast_view {
                    // Text input mode for adding a feed URL
                    if pv.input_mode() == InputMode::AddingFeed {
                        match key.code {
                            KeyCode::Esc => pv.cancel_input(),
                            KeyCode::Enter => pv.submit_input(),
                            KeyCode::Backspace => pv.input_backspace(),
                            KeyCode::Char(c) => pv.input_char(c),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Esc if pv.back() => {
                                close_podcast = true;
                            }
                            KeyCode::Esc => {}
                            KeyCode::Backspace if pv.back() => {
                                close_podcast = true;
                            }
                            KeyCode::Backspace => {}
                            KeyCode::Char('j') | KeyCode::Down => pv.select_down(),
                            KeyCode::Char('k') | KeyCode::Up => pv.select_up(),
                            KeyCode::Enter => {
                                play_episode = pv.enter();
                            }
                            KeyCode::Char('a') => pv.start_add_feed(),
                            KeyCode::Char('x') | KeyCode::Delete => {
                                let idx = pv.selected_feed_index();
                                if pv.mode() == PodcastViewMode::FeedList {
                                    pv.remove_feed(idx);
                                }
                            }
                            KeyCode::Char('R') => {
                                if pv.mode() == PodcastViewMode::EpisodeList {
                                    let idx = pv.selected_feed_index();
                                    pv.refresh_feed(idx);
                                } else {
                                    pv.refresh_all();
                                }
                            }
                            KeyCode::Char('D') if pv.mode() == PodcastViewMode::EpisodeList => {
                                pv.download_selected_episode();
                            }
                            KeyCode::Char('C') if pv.mode() == PodcastViewMode::EpisodeList => {
                                pv.cleanup_played_episodes();
                            }
                            KeyCode::Char('q') => {
                                if let Err(e) = player.save_session() {
                                    tracing::warn!("Failed to save session on quit: {e}");
                                }
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }

                if play_episode
                    && let Some(ref pv) = podcast_view
                    && let Some(track) = pv.selected_episode_track()
                {
                    let episode_chapters = pv.selected_episode_chapters();
                    let action = player.add_and_play(track);
                    handle_action(
                        action,
                        player,
                        playback_handle,
                        &mut gapless_played_next,
                        &mut pre_decode_triggered,
                    )?;
                    player.set_chapters(episode_chapters);
                }

                if close_podcast {
                    podcast_view = None;
                }

                continue;
            }

            // EQ view input handling
            if show_eq {
                let cmd = match key.code {
                    KeyCode::Esc => {
                        show_eq = false;
                        None
                    }
                    KeyCode::Char('e') => Some(PlayerCommand::CycleEqPreset),
                    KeyCode::Char('h') | KeyCode::Left => Some(PlayerCommand::EqBandLeft),
                    KeyCode::Char('l') | KeyCode::Right => Some(PlayerCommand::EqBandRight),
                    KeyCode::Char('k') | KeyCode::Up => Some(PlayerCommand::EqBandUp),
                    KeyCode::Char('j') | KeyCode::Down => Some(PlayerCommand::EqBandDown),
                    // Playback keys still work in EQ view
                    KeyCode::Char(' ') => Some(PlayerCommand::PlayPause),
                    KeyCode::Char('n') => Some(PlayerCommand::NextTrack),
                    KeyCode::Char('p') => Some(PlayerCommand::PrevTrack),
                    KeyCode::Char('+') | KeyCode::Char('=') => Some(PlayerCommand::VolumeUp),
                    KeyCode::Char('-') => Some(PlayerCommand::VolumeDown),
                    KeyCode::Char('q') => Some(PlayerCommand::Quit),
                    _ => None,
                };

                if let Some(cmd) = cmd {
                    let action = player.handle_command(cmd);
                    if matches!(action, PlayerAction::Quit) {
                        if let Err(e) = player.save_session() {
                            tracing::warn!("Failed to save session on quit: {e}");
                        }
                        return Ok(());
                    }
                    handle_action(
                        action,
                        player,
                        playback_handle,
                        &mut gapless_played_next,
                        &mut pre_decode_triggered,
                    )?;
                }
                continue;
            }

            let cmd = match key.code {
                KeyCode::Char(' ') => Some(PlayerCommand::PlayPause),
                KeyCode::Char('s') => Some(PlayerCommand::Stop),
                KeyCode::Char('q') => Some(PlayerCommand::Quit),
                KeyCode::Char('n') => Some(PlayerCommand::NextTrack),
                KeyCode::Char('p') => Some(PlayerCommand::PrevTrack),
                KeyCode::Right => Some(PlayerCommand::SeekForward(Duration::from_secs(5))),
                KeyCode::Left => Some(PlayerCommand::SeekBackward(Duration::from_secs(5))),
                KeyCode::Char('+') | KeyCode::Char('=') => Some(PlayerCommand::VolumeUp),
                KeyCode::Char('-') => Some(PlayerCommand::VolumeDown),
                KeyCode::Char('e') => {
                    show_eq = true;
                    None
                }
                KeyCode::Char('t') => {
                    *theme_index = (*theme_index + 1) % themes.len();
                    *theme = themes[*theme_index].clone();
                    None
                }
                KeyCode::Char('d') => {
                    match crate::backend::cpal_backend::list_devices() {
                        Ok(devices) if !devices.is_empty() => {
                            let current_name = player.device_name().unwrap_or("");
                            let current_pos = devices
                                .iter()
                                .position(|d| d.name.eq_ignore_ascii_case(current_name))
                                .unwrap_or(devices.len()); // if not found, wraps to 0
                            let next_idx = (current_pos + 1) % devices.len();
                            let next_device = &devices[next_idx];
                            let msg = if next_device.is_default {
                                format!("Device: {} (default)", next_device.name)
                            } else {
                                format!("Device: {}", next_device.name)
                            };
                            player
                                .handle_command(PlayerCommand::SetDevice(next_device.name.clone()));
                            status_message = Some((msg, Instant::now()));
                        }
                        Ok(_) => {
                            status_message =
                                Some(("No audio devices found".to_string(), Instant::now()));
                        }
                        Err(e) => {
                            status_message = Some((format!("Device error: {e}"), Instant::now()));
                        }
                    }
                    None
                }
                KeyCode::Char('o') => {
                    let start_dir = crate::core::config::KoraConfig::load()
                        .ok()
                        .and_then(|c| c.music_dir)
                        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
                    file_browser = Some(FileBrowser::new(start_dir));
                    None
                }
                KeyCode::Char('P') => {
                    let state = crate::providers::podcast::load_state().unwrap_or_default();
                    let config = crate::core::config::KoraConfig::load().ok();
                    let dl_dir = config.and_then(|c| c.podcast_download_dir);
                    let mut pv = PodcastView::with_download_dir(&state, dl_dir);
                    pv.ensure_refreshed();
                    podcast_view = Some(pv);
                    None
                }
                KeyCode::Char('f') => Some(PlayerCommand::ToggleFavorite),
                KeyCode::Char('z') => Some(PlayerCommand::ToggleShuffle),
                KeyCode::Char('r') => Some(PlayerCommand::CycleRepeat),
                KeyCode::Char('S') => Some(PlayerCommand::CycleSleepTimer),
                KeyCode::Char(']') => Some(PlayerCommand::SpeedUp),
                KeyCode::Char('[') => Some(PlayerCommand::SpeedDown),
                KeyCode::Char('v') => {
                    visualizer_mode = match visualizer_mode {
                        VisualizerMode::Off => VisualizerMode::Normal,
                        VisualizerMode::Normal => VisualizerMode::Off,
                        VisualizerMode::Fullscreen => VisualizerMode::Off,
                    };
                    None
                }
                KeyCode::Char('V') => {
                    visualizer_mode = match visualizer_mode {
                        VisualizerMode::Fullscreen => VisualizerMode::Normal,
                        _ => VisualizerMode::Fullscreen,
                    };
                    None
                }
                KeyCode::Char('y') => {
                    show_lyrics = !show_lyrics;
                    None
                }
                KeyCode::Char('.') => Some(PlayerCommand::NextChapter),
                KeyCode::Char(',') => Some(PlayerCommand::PrevChapter),
                _ => None,
            };

            if let Some(cmd) = cmd {
                let action = player.handle_command(cmd);
                if matches!(action, PlayerAction::Quit) {
                    // Save session before quitting
                    if let Err(e) = player.save_session() {
                        tracing::warn!("Failed to save session on quit: {e}");
                    }
                    return Ok(());
                }
                handle_action(
                    action,
                    player,
                    playback_handle,
                    &mut gapless_played_next,
                    &mut pre_decode_triggered,
                )?;
            }
        }
    }
}

/// Sync OS media controls with current player state (metadata + playback).
/// Only sends updates when something actually changed to avoid unnecessary IPC.
#[cfg(feature = "media-controls")]
fn sync_media_controls(
    player: &Player,
    media_controls: &mut Option<(
        souvlaki::MediaControls,
        std::sync::mpsc::Receiver<souvlaki::MediaControlEvent>,
    )>,
    last_track_index: &mut Option<usize>,
    last_playing: &mut Option<bool>,
) {
    let Some((controls, _)) = media_controls.as_mut() else {
        return;
    };

    let (current_idx, _) = player.queue_position();
    let is_playing = player.state() == PlaybackState::Playing;

    // Update metadata when the track changes
    if *last_track_index != Some(current_idx) {
        *last_track_index = Some(current_idx);
        if let Some(track) = player.current_track() {
            let meta = track.metadata.as_ref();
            crate::media_controls::update_metadata(
                controls,
                meta.and_then(|m| m.title.as_deref()),
                meta.and_then(|m| m.artist.as_deref()),
                meta.and_then(|m| m.album.as_deref()),
                meta.and_then(|m| m.duration),
            );
        }
    }

    // Update playback state when it changes
    if *last_playing != Some(is_playing) {
        *last_playing = Some(is_playing);
        let position = if player.duration().as_secs() > 0 {
            Some(player.current_position())
        } else {
            None
        };
        crate::media_controls::update_playback(controls, is_playing, position);
    }
}

/// Process a single IPC request and return a response.
fn handle_ipc_request(
    request: &crate::ipc::protocol::IpcRequest,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
    gapless_played_next: &mut Option<Arc<AtomicBool>>,
    pre_decode_triggered: &mut bool,
) -> crate::ipc::protocol::IpcResponse {
    use crate::ipc::protocol::{IpcRequest, IpcResponse, PlayerStatus};

    match request {
        IpcRequest::Status => {
            let state_str = match player.state() {
                PlaybackState::Playing => "playing",
                PlaybackState::Paused => "paused",
                PlaybackState::Stopped => "stopped",
            };
            let (pos, total) = player.queue_position();
            IpcResponse::with_status(PlayerStatus {
                state: state_str.to_string(),
                track: player.current_track().map(|t| t.display_name()),
                position_secs: player.current_position().as_secs_f64(),
                duration_secs: player.duration().as_secs_f64(),
                volume_db: player.volume_db(),
                queue_position: pos,
                queue_total: total,
            })
        }
        IpcRequest::Play if player.state() != PlaybackState::Playing => ipc_dispatch(
            PlayerCommand::PlayPause,
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
        IpcRequest::Play => IpcResponse::ok(),
        IpcRequest::Pause if player.state() == PlaybackState::Playing => ipc_dispatch(
            PlayerCommand::PlayPause,
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
        IpcRequest::Pause => IpcResponse::ok(),
        IpcRequest::Toggle => ipc_dispatch(
            PlayerCommand::PlayPause,
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
        IpcRequest::Stop => ipc_dispatch(
            PlayerCommand::Stop,
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
        IpcRequest::Next => ipc_dispatch(
            PlayerCommand::NextTrack,
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
        IpcRequest::Prev => ipc_dispatch(
            PlayerCommand::PrevTrack,
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
        IpcRequest::Volume { db } => ipc_dispatch(
            PlayerCommand::SetVolume(*db),
            player,
            playback_handle,
            gapless_played_next,
            pre_decode_triggered,
        ),
    }
}

/// Execute a player command and convert the result to an IPC response.
fn ipc_dispatch(
    cmd: PlayerCommand,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
    gapless_played_next: &mut Option<Arc<AtomicBool>>,
    pre_decode_triggered: &mut bool,
) -> crate::ipc::protocol::IpcResponse {
    use crate::ipc::protocol::IpcResponse;

    let action = player.handle_command(cmd);
    match handle_action(
        action,
        player,
        playback_handle,
        gapless_played_next,
        pre_decode_triggered,
    ) {
        Ok(()) => IpcResponse::ok(),
        Err(e) => IpcResponse::error(e.to_string()),
    }
}

fn handle_action(
    action: PlayerAction,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
    gapless_played_next: &mut Option<Arc<AtomicBool>>,
    pre_decode_triggered: &mut bool,
) -> Result<()> {
    match action {
        PlayerAction::None => {}
        PlayerAction::LoadAndPlay => {
            // Stop and join previous playback thread
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            *gapless_played_next = None;
            *pre_decode_triggered = false;

            // Try to use pre-decoded samples to skip decode time
            let current_idx = player.current_index();
            if let Some(pre) = player.take_next_track_samples() {
                if pre.track_index == current_idx {
                    tracing::info!(
                        "Using pre-decoded track [{}] — skipping decode",
                        current_idx
                    );
                    player.play_predecoded(pre);
                } else {
                    // Pre-decoded data is for a different track, decode fresh
                    player.play_current()?;
                }
            } else {
                player.play_current()?;
            }
            *playback_handle = player.start_playback();
            player.mark_playback_started();
        }
        PlayerAction::GaplessTransition => {
            // Use pre-decoded samples — eliminates decode latency at track boundary
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            *gapless_played_next = None;
            *pre_decode_triggered = false;

            if let Some(pre) = player.take_next_track_samples() {
                tracing::info!("Gapless transition to track [{}]", pre.track_index);
                player.play_predecoded(pre);
                *playback_handle = player.start_playback();
            } else {
                // Fallback: pre-decoded samples missing, decode fresh
                player.play_current()?;
                *playback_handle = player.start_playback();
            }
            player.mark_playback_started();
        }
        PlayerAction::Quit => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw(
    frame: &mut Frame,
    player: &Player,
    theme: &Theme,
    file_browser: Option<&FileBrowser>,
    podcast_view: Option<&PodcastView>,
    show_eq: bool,
    show_lyrics: bool,
    visualizer_mode: VisualizerMode,
    status_message: Option<&str>,
) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Track info + progress
            Constraint::Min(3),    // Playlist or EQ view
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    draw_track_info(frame, chunks[0], player, theme);

    if show_eq {
        draw_eq_view(frame, chunks[1], player, theme);
        draw_eq_status_bar(frame, chunks[2], player, theme);
    } else {
        match (visualizer_mode, show_lyrics) {
            (VisualizerMode::Fullscreen, _) => {
                draw_visualizer(frame, chunks[1], player, theme);
            }
            (VisualizerMode::Normal, true) => {
                // Visualizer + lyrics: split right panel between playlist, lyrics, visualizer
                let middle = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(40), // Playlist
                        Constraint::Percentage(35), // Lyrics
                        Constraint::Percentage(25), // Visualizer
                    ])
                    .split(chunks[1]);
                draw_playlist(frame, middle[0], player, theme);
                draw_lyrics(frame, middle[1], player, theme);
                draw_visualizer(frame, middle[2], player, theme);
            }
            (VisualizerMode::Normal, false) => {
                let middle = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(60), // Playlist
                        Constraint::Percentage(40), // Visualizer
                    ])
                    .split(chunks[1]);
                draw_playlist(frame, middle[0], player, theme);
                draw_visualizer(frame, middle[1], player, theme);
            }
            (VisualizerMode::Off, true) => {
                // Lyrics replace the right side
                let middle = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50), // Playlist
                        Constraint::Percentage(50), // Lyrics
                    ])
                    .split(chunks[1]);
                draw_playlist(frame, middle[0], player, theme);
                draw_lyrics(frame, middle[1], player, theme);
            }
            (VisualizerMode::Off, false) => {
                draw_playlist(frame, chunks[1], player, theme);
            }
        }
        draw_status_bar(frame, chunks[2], player, theme, status_message);
    }

    // File browser overlay
    if let Some(browser) = file_browser {
        draw_file_browser(frame, area, browser, theme);
    }

    // Podcast view overlay
    if let Some(pv) = podcast_view {
        draw_podcast_view(frame, area, pv, theme);
    }
}

fn draw_track_info(frame: &mut Frame, area: Rect, player: &Player, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" kora ")
        .title_style(theme.title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Track name
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Status line
        ])
        .split(inner);

    // Track name
    let (queue_pos, queue_total) = player.queue_position();
    let track_name = player
        .current_track()
        .map(|t| t.display_name())
        .unwrap_or_else(|| "No track".into());

    let track_line = Line::from(vec![
        Span::styled(
            format!("[{queue_pos}/{queue_total}] "),
            theme.track_position,
        ),
        Span::styled(track_name, theme.track_title),
        Span::styled(
            if player.is_current_favorited() {
                " ★"
            } else {
                ""
            },
            theme.track_title,
        ),
    ]);

    let chapter_spans: Vec<Span> =
        if let Some((ch_pos, ch_total)) = player.current_chapter_position() {
            let ch_title = player
                .current_chapter()
                .map(|c| c.title.as_str())
                .unwrap_or("—");
            vec![
                Span::styled("  ", theme.status_info),
                Span::styled(
                    format!("Ch {ch_pos}/{ch_total}: {ch_title}"),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]
        } else {
            vec![]
        };

    let mut full_track_line = track_line.spans;
    full_track_line.extend(chapter_spans);
    frame.render_widget(Paragraph::new(Line::from(full_track_line)), chunks[0]);

    // Progress bar
    let position = player.current_position();
    let duration = player.duration();
    let ratio = if duration.as_secs_f64() > 0.0 {
        (position.as_secs_f64() / duration.as_secs_f64()).min(1.0)
    } else {
        0.0
    };

    let progress_label = format!(
        "{} / {}",
        format_duration(position),
        format_duration(duration)
    );

    let gauge = Gauge::default()
        .gauge_style(theme.progress_bar.patch(theme.progress_bg))
        .ratio(ratio)
        .label(progress_label);
    frame.render_widget(gauge, chunks[1]);

    // Status line
    let (state_icon, state_style) = match player.state() {
        PlaybackState::Playing => ("▶ Playing", theme.status_playing),
        PlaybackState::Paused => ("⏸ Paused", theme.status_paused),
        PlaybackState::Stopped => ("■ Stopped", theme.status_stopped),
    };

    let eq_info = format!("  EQ: {}", player.eq_display_name());

    let shuffle_info = if player.shuffle() {
        "  Shuffle: On"
    } else {
        "  Shuffle: Off"
    };

    let repeat_info = format!("  Repeat: {}", player.repeat());

    let speed_info = if (player.speed() - 1.0).abs() >= 0.01 {
        format!("  Speed: {:.2}x", player.speed())
    } else {
        String::new()
    };

    let rg_info_str =
        if player.replaygain_mode() != crate::playback::replaygain::ReplayGainMode::Off {
            if let Some(rg) = player.current_replaygain() {
                let gain = crate::playback::replaygain::gain_to_apply(rg, player.replaygain_mode());
                match gain {
                    Some(db) => format!("  RG: {:+.1}dB", db),
                    None => String::new(),
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

    let chapter_info = {
        let pos_ms = player.current_position().as_millis() as u64;
        crate::playback::chapters::format_chapter_display(player.chapters(), pos_ms)
            .map(|s| format!("  {s}"))
            .unwrap_or_default()
    };

    let status = Line::from(vec![
        Span::styled(state_icon, state_style),
        Span::styled(
            format!(
                "  Vol: {:+.0}dB{eq_info}{shuffle_info}{repeat_info}{speed_info}{rg_info_str}{chapter_info}  Theme: {}",
                player.volume_db(),
                theme.name
            ),
            theme.status_info,
        ),
    ]);
    frame.render_widget(Paragraph::new(status), chunks[2]);
}

fn draw_playlist(frame: &mut Frame, area: Rect, player: &Player, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Playlist ")
        .title_style(theme.title);

    let items: Vec<ListItem> = player
        .tracks()
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = i == player.current_index();
            let prefix = if is_current { "▶ " } else { "  " };
            let fav = if player.favorites().contains(&track.path_string()) {
                " ★"
            } else {
                ""
            };
            let style = if is_current {
                theme.playlist_current
            } else {
                theme.playlist_normal
            };
            ListItem::new(Line::from(Span::styled(
                format!("{prefix}{}. {}{fav}", i + 1, track.display_name()),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_lyrics(frame: &mut Frame, area: Rect, player: &Player, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Lyrics ")
        .title_style(theme.title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let current_lyrics = player.lyrics();
    if current_lyrics.lines.is_empty() {
        let msg = Paragraph::new(Line::from(Span::styled(
            "  No lyrics found",
            Style::default().fg(theme.dim),
        )));
        frame.render_widget(msg, inner);
        return;
    }

    let position_ms = player.current_position().as_millis() as u64;
    let current_idx = lyrics::current_line_index(current_lyrics, position_ms);

    let visible_lines = inner.height as usize;
    if visible_lines == 0 {
        return;
    }

    let total = current_lyrics.lines.len();
    let center = current_idx.unwrap_or(0);
    let half = visible_lines / 2;
    let start = center.saturating_sub(half);
    let end = (start + visible_lines).min(total);
    // Adjust start if we're near the end
    let start = if end == total {
        total.saturating_sub(visible_lines)
    } else {
        start
    };

    let lines: Vec<Line> = current_lyrics.lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, lyric)| {
            let abs_idx = start + i;
            let style = match current_idx {
                Some(cur) if abs_idx == cur => Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
                Some(cur) if abs_idx < cur => Style::default().fg(theme.dim),
                _ => Style::default().fg(theme.fg),
            };
            let text = if lyric.text.is_empty() {
                "♪".to_string()
            } else {
                lyric.text.clone()
            };
            Line::from(Span::styled(format!(" {text}"), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn draw_status_bar(
    frame: &mut Frame,
    area: Rect,
    player: &Player,
    theme: &Theme,
    status_message: Option<&str>,
) {
    let mut spans = vec![
        Span::styled("Spc", theme.help_key),
        Span::styled(":Play/Pause ", theme.help_text),
        Span::styled("n/p", theme.help_key),
        Span::styled(":Next/Prev ", theme.help_text),
        Span::styled("s", theme.help_key),
        Span::styled(":Stop ", theme.help_text),
        Span::styled("+/-", theme.help_key),
        Span::styled(":Vol ", theme.help_text),
        Span::styled("←/→", theme.help_key),
        Span::styled(":Seek ", theme.help_text),
        Span::styled("e", theme.help_key),
        Span::styled(":EQ ", theme.help_text),
        Span::styled("t", theme.help_key),
        Span::styled(":Theme ", theme.help_text),
        Span::styled("o", theme.help_key),
        Span::styled(":Browse ", theme.help_text),
        Span::styled("P", theme.help_key),
        Span::styled(":Podcast ", theme.help_text),
        Span::styled("f", theme.help_key),
        Span::styled(":Fav ", theme.help_text),
        Span::styled("z", theme.help_key),
        Span::styled(":Shuffle ", theme.help_text),
        Span::styled("r", theme.help_key),
        Span::styled(":Repeat ", theme.help_text),
        Span::styled("v/V", theme.help_key),
        Span::styled(":Viz ", theme.help_text),
        Span::styled("y", theme.help_key),
        Span::styled(":Lyrics ", theme.help_text),
        Span::styled(",/.", theme.help_key),
        Span::styled(":Ch± ", theme.help_text),
        Span::styled("[/]", theme.help_key),
        Span::styled(":Speed ", theme.help_text),
        Span::styled("S", theme.help_key),
        Span::styled(":Sleep ", theme.help_text),
        Span::styled("d", theme.help_key),
        Span::styled(":Device ", theme.help_text),
        Span::styled("q", theme.help_key),
        Span::styled(":Quit", theme.help_text),
    ];

    if let Some(remaining) = player.sleep_remaining() {
        let sleep_text = if player.is_sleep_fading() {
            " | Sleep: fading...".to_string()
        } else {
            let total_secs = remaining.as_secs();
            let mins = total_secs / 60;
            let secs = total_secs % 60;
            format!(" | Sleep: {mins}:{secs:02}")
        };
        spans.push(Span::styled(sleep_text, theme.status_info));
    }

    if let Some(msg) = status_message {
        spans.push(Span::styled(format!(" | {msg}"), theme.status_info));
    }

    let help = Line::from(spans);
    frame.render_widget(Paragraph::new(help), area);
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

fn draw_visualizer(frame: &mut Frame, area: Rect, player: &Player, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Spectrum ")
        .title_style(theme.title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    let bars = player.spectrum().read_bars();
    let num_bars = bars.len();
    if num_bars == 0 {
        return;
    }

    let available_width = inner.width as usize;
    let chart_height = inner.height as usize;

    // Calculate bar width and spacing
    let bar_width = (available_width / num_bars).max(1);
    let total_used = bar_width * num_bars;
    let left_pad = (available_width.saturating_sub(total_used)) / 2;

    let buf = frame.buffer_mut();
    let accent_style = Style::default().fg(theme.accent);
    let dim_style = Style::default().fg(theme.dim);

    for (i, &magnitude) in bars.iter().enumerate() {
        let x_start = inner.x + left_pad as u16 + (i * bar_width) as u16;
        if x_start >= inner.x + inner.width {
            break;
        }

        // Map magnitude (0.0..1.0) to total height in eighths
        let total_eighths = (magnitude * chart_height as f32 * 8.0).round() as usize;
        let full_rows = total_eighths / 8;
        let remainder = total_eighths % 8;

        // Draw from bottom up
        for row in 0..chart_height {
            let y = inner.y + inner.height - 1 - row as u16;

            // Determine what character to draw at this row
            let ch = if row < full_rows {
                '█'
            } else if row == full_rows && remainder > 0 {
                BAR_CHARS[remainder - 1]
            } else {
                ' '
            };

            let style = if ch != ' ' { accent_style } else { dim_style };

            // Draw across bar width (leave 1-char gap between bars if width > 2)
            let draw_width = if bar_width > 2 {
                bar_width - 1
            } else {
                bar_width
            };

            for col in 0..draw_width {
                let x = x_start + col as u16;
                if x < inner.x + inner.width {
                    buf[(x, y)].set_char(ch).set_style(style);
                }
            }
        }
    }
}

fn draw_eq_view(frame: &mut Frame, area: Rect, player: &Player, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(format!(" EQ: {} ", player.eq_display_name()))
        .title_style(theme.title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 40 {
        let msg = Paragraph::new(Line::from(Span::styled(
            "  (terminal too small for EQ view)",
            theme.status_info,
        )));
        frame.render_widget(msg, inner);
        return;
    }

    let gains = player.eq_gains();
    let selected = player.eq_selected_band();
    let labels = [
        "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
    ];

    // Layout: leave 1 row for frequency labels at bottom, 1 row for gain value at top
    let chart_height = inner.height.saturating_sub(2) as i32;
    if chart_height < 2 {
        return;
    }

    // Scale: map -12..+12 dB to the chart height
    let half = chart_height / 2;
    let zero_row = half as u16; // row index of the 0dB line (from top of chart area)

    // Chart area starts 1 row below inner.y (for the gain value display row)
    let chart_y = inner.y + 1;
    let label_y = chart_y + chart_height as u16;

    // Calculate band width and spacing
    let band_count = 10u16;
    let total_width = inner.width;
    let band_width = (total_width / band_count).max(1);

    // Draw gain value header for selected band
    let gain_val = gains[selected];
    let gain_text = format!("{:+.0}dB", gain_val);
    let gain_x = inner.x + selected as u16 * band_width + band_width / 2;
    let gain_x = gain_x.saturating_sub(gain_text.len() as u16 / 2);
    let gain_span = Span::styled(
        gain_text,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    if gain_x < inner.x + inner.width {
        frame.render_widget(
            Paragraph::new(Line::from(gain_span)),
            Rect::new(gain_x, inner.y, (inner.x + inner.width - gain_x).min(6), 1),
        );
    }

    // Draw the zero line
    for x in 0..total_width {
        let abs_x = inner.x + x;
        let abs_y = chart_y + zero_row;
        if abs_y < label_y {
            let buf = frame.buffer_mut();
            if abs_x < inner.x + inner.width {
                buf[(abs_x, abs_y)]
                    .set_char('─')
                    .set_style(Style::default().fg(theme.dim));
            }
        }
    }

    // Draw each band bar
    for (i, &gain) in gains.iter().enumerate() {
        let bar_x = inner.x + i as u16 * band_width;
        let is_selected = i == selected;
        let bar_style = if is_selected {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };

        // Map gain to rows: each row = 12/half dB
        let db_per_row = if half > 0 { 12.0 / half as f32 } else { 1.0 };
        let bar_rows = (gain.abs() / db_per_row).round() as i32;
        let bar_rows = bar_rows.min(half);

        if gain >= 0.0 {
            // Draw upward from zero line
            for r in 0..bar_rows {
                let y = chart_y + zero_row - 1 - r as u16;
                if y >= chart_y {
                    draw_bar_cell(frame, bar_x, y, band_width, bar_style, inner);
                }
            }
        } else {
            // Draw downward from zero line
            for r in 0..bar_rows {
                let y = chart_y + zero_row + 1 + r as u16;
                if y < label_y {
                    draw_bar_cell(frame, bar_x, y, band_width, bar_style, inner);
                }
            }
        }

        // Draw frequency label
        if label_y < inner.y + inner.height {
            let label = labels[i];
            let lx = bar_x + band_width / 2;
            let lx = lx.saturating_sub(label.len() as u16 / 2);
            let available = (inner.x + inner.width).saturating_sub(lx);
            if available > 0 {
                let label_style = if is_selected {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dim)
                };
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(label, label_style))),
                    Rect::new(lx, label_y, available.min(label.len() as u16), 1),
                );
            }
        }
    }
}

fn draw_bar_cell(frame: &mut Frame, x: u16, y: u16, band_width: u16, style: Style, inner: Rect) {
    // Draw a bar cell with block chars, leaving 1 char padding on each side
    let start = x + 1;
    let end = (x + band_width).saturating_sub(1);
    let buf = frame.buffer_mut();
    for bx in start..end {
        if bx < inner.x + inner.width {
            buf[(bx, y)].set_char('█').set_style(style);
        }
    }
}

fn draw_eq_status_bar(frame: &mut Frame, area: Rect, player: &Player, theme: &Theme) {
    let mut spans = vec![
        Span::styled("e", theme.help_key),
        Span::styled(":Preset ", theme.help_text),
        Span::styled("h/l", theme.help_key),
        Span::styled(":Band ", theme.help_text),
        Span::styled("j/k", theme.help_key),
        Span::styled(":Gain ", theme.help_text),
        Span::styled("Esc", theme.help_key),
        Span::styled(":Close ", theme.help_text),
        Span::styled("Spc", theme.help_key),
        Span::styled(":Play/Pause ", theme.help_text),
        Span::styled("n/p", theme.help_key),
        Span::styled(":Next/Prev ", theme.help_text),
        Span::styled("+/-", theme.help_key),
        Span::styled(":Vol ", theme.help_text),
        Span::styled("q", theme.help_key),
        Span::styled(":Quit", theme.help_text),
    ];

    if let Some(remaining) = player.sleep_remaining() {
        let sleep_text = if player.is_sleep_fading() {
            " | Sleep: fading...".to_string()
        } else {
            let total_secs = remaining.as_secs();
            let mins = total_secs / 60;
            let secs = total_secs % 60;
            format!(" | Sleep: {mins}:{secs:02}")
        };
        spans.push(Span::styled(sleep_text, theme.status_info));
    }

    let help = Line::from(spans);
    frame.render_widget(Paragraph::new(help), area);
}

fn draw_file_browser(frame: &mut Frame, area: Rect, browser: &FileBrowser, theme: &Theme) {
    let popup_area = centered_rect(80, 70, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", browser.current_dir().display()))
        .border_style(theme.border)
        .title_style(theme.title);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let entries = browser.entries_for_display();

    if entries.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "  (empty directory)",
            theme.status_info,
        )));
        frame.render_widget(empty_msg, inner);
        return;
    }

    let visible_height = inner.height as usize;
    let scroll = browser.scroll_offset();
    let end = (scroll + visible_height).min(entries.len());

    let items: Vec<ListItem> = entries[scroll..end]
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let idx = scroll + i;
            let icon = if entry.is_dir { "📁 " } else { "🎵 " };
            let style = if idx == browser.selected_index() {
                theme.playlist_current
            } else if entry.is_dir {
                theme.title
            } else {
                theme.playlist_normal
            };
            ListItem::new(Line::from(Span::styled(
                format!("{icon}{}", entry.name),
                style,
            )))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn draw_podcast_view(frame: &mut Frame, area: Rect, pv: &PodcastView, theme: &Theme) {
    let popup_area = centered_rect(80, 80, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    // Reserve bottom row for help bar (and input bar when adding)
    let input_active = pv.input_mode() == InputMode::AddingFeed;
    let bottom_rows = if input_active { 2 } else { 1 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(bottom_rows)])
        .split(popup_area);

    let content_area = layout[0];
    let bottom_area = layout[1];

    match pv.mode() {
        PodcastViewMode::FeedList => {
            let title = if let Some(msg) = pv.status_message() {
                format!(" Podcasts — {msg} ")
            } else {
                " Podcasts ".to_string()
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border)
                .title(title)
                .title_style(theme.title);

            let inner = block.inner(content_area);
            frame.render_widget(block, content_area);

            let feeds = pv.feeds();
            if feeds.is_empty() {
                let msg = Paragraph::new(Line::from(Span::styled(
                    "  No podcasts subscribed. Press 'a' to add a feed.",
                    theme.status_info,
                )));
                frame.render_widget(msg, inner);
            } else {
                let visible_height = inner.height as usize;
                let scroll = pv.scroll_offset();
                let end = (scroll + visible_height).min(feeds.len());

                let items: Vec<ListItem> = feeds[scroll..end]
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| {
                        let idx = scroll + i;
                        let is_selected = idx == pv.selected_feed_index();
                        let prefix = if is_selected { "▶ " } else { "  " };
                        let ep_count = if entry.episode_count > 0 {
                            format!(" ({} eps)", entry.episode_count)
                        } else {
                            String::new()
                        };
                        let style = if is_selected {
                            theme.playlist_current
                        } else {
                            theme.playlist_normal
                        };
                        ListItem::new(Line::from(Span::styled(
                            format!("{prefix}{}{ep_count}", entry.feed.title),
                            style,
                        )))
                    })
                    .collect();

                let list = List::new(items);
                frame.render_widget(list, inner);
            }
        }
        PodcastViewMode::EpisodeList => {
            let feed_title = pv
                .feeds()
                .get(pv.selected_feed_index())
                .map(|f| f.feed.title.as_str())
                .unwrap_or("Unknown");

            let title = if let Some(msg) = pv.status_message() {
                format!(" {feed_title} — {msg} ")
            } else {
                format!(" {feed_title} ")
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border)
                .title(title)
                .title_style(theme.title);

            let inner = block.inner(content_area);
            frame.render_widget(block, content_area);

            if let Some(feed_entry) = pv.feeds().get(pv.selected_feed_index()) {
                let episodes = &feed_entry.episodes;
                if episodes.is_empty() {
                    let msg = Paragraph::new(Line::from(Span::styled(
                        "  No episodes found",
                        theme.status_info,
                    )));
                    frame.render_widget(msg, inner);
                } else {
                    let visible_height = inner.height as usize;
                    let scroll = pv.scroll_offset();
                    let end = (scroll + visible_height).min(episodes.len());

                    let items: Vec<ListItem> = episodes[scroll..end]
                        .iter()
                        .enumerate()
                        .map(|(i, ep)| {
                            let idx = scroll + i;
                            let is_selected = idx == pv.selected_episode_index();
                            let prefix = if is_selected { "▶ " } else { "  " };
                            let played_icon = if ep.played { "✓ " } else { "  " };
                            let dl_icon = if pv.is_episode_downloaded(pv.selected_feed_index(), idx)
                            {
                                "[✓] "
                            } else {
                                ""
                            };
                            let duration_str = ep
                                .duration_secs
                                .map(|s| {
                                    let mins = s / 60;
                                    let secs = s % 60;
                                    format!(" [{mins}:{secs:02}]")
                                })
                                .unwrap_or_default();
                            let date_str = ep
                                .published
                                .as_ref()
                                .map(|d| {
                                    // Show a short date prefix
                                    let short = if d.len() > 16 { &d[..16] } else { d };
                                    format!(" ({short})")
                                })
                                .unwrap_or_default();
                            let style = if is_selected {
                                theme.playlist_current
                            } else if ep.played {
                                theme.status_info
                            } else {
                                theme.playlist_normal
                            };
                            ListItem::new(Line::from(Span::styled(
                                format!(
                                    "{prefix}{played_icon}{dl_icon}{}{duration_str}{date_str}",
                                    ep.title
                                ),
                                style,
                            )))
                        })
                        .collect();

                    let list = List::new(items);
                    frame.render_widget(list, inner);
                }
            }
        }
    }

    // Bottom area: input bar and/or help bar
    if input_active {
        let input_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(bottom_area);

        let input_line = Line::from(vec![
            Span::styled(" Feed URL: ", theme.title),
            Span::styled(pv.input_buffer(), theme.track_title),
            Span::styled("█", theme.playlist_current),
        ]);
        frame.render_widget(Paragraph::new(input_line), input_layout[0]);

        let help = Line::from(vec![
            Span::styled("Enter", theme.help_key),
            Span::styled(":Submit ", theme.help_text),
            Span::styled("Esc", theme.help_key),
            Span::styled(":Cancel", theme.help_text),
        ]);
        frame.render_widget(Paragraph::new(help), input_layout[1]);
    } else {
        let help_spans = match pv.mode() {
            PodcastViewMode::FeedList => vec![
                Span::styled("j/k", theme.help_key),
                Span::styled(":Navigate ", theme.help_text),
                Span::styled("Enter", theme.help_key),
                Span::styled(":Episodes ", theme.help_text),
                Span::styled("a", theme.help_key),
                Span::styled(":Add Feed ", theme.help_text),
                Span::styled("x", theme.help_key),
                Span::styled(":Remove ", theme.help_text),
                Span::styled("R", theme.help_key),
                Span::styled(":Refresh All ", theme.help_text),
                Span::styled("Esc", theme.help_key),
                Span::styled(":Close", theme.help_text),
            ],
            PodcastViewMode::EpisodeList => vec![
                Span::styled("j/k", theme.help_key),
                Span::styled(":Navigate ", theme.help_text),
                Span::styled("Enter", theme.help_key),
                Span::styled(":Play ", theme.help_text),
                Span::styled("D", theme.help_key),
                Span::styled(":Download ", theme.help_text),
                Span::styled("C", theme.help_key),
                Span::styled(":Cleanup ", theme.help_text),
                Span::styled("R", theme.help_key),
                Span::styled(":Refresh ", theme.help_text),
                Span::styled("Bksp/Esc", theme.help_key),
                Span::styled(":Back", theme.help_text),
            ],
        };
        let help = Line::from(help_spans);
        frame.render_widget(Paragraph::new(help), bottom_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
