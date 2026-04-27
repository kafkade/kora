//! Daemon mode — headless event loop controlled via IPC.
//!
//! Runs the player without a TUI, printing minimal status to stdout.
//! Remote control is available via `kora pause`, `kora next`, etc.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;

use crate::ipc::protocol::{IpcRequest, IpcResponse, PlayerStatus};
use crate::ipc::server::IpcMessage;
use crate::playback::player::{PlaybackState, Player, PlayerAction, PlayerCommand};

/// Run the player in daemon (headless) mode.
///
/// Starts the IPC server, plays the first track, and enters a polling loop
/// that handles IPC commands and track transitions until stopped.
pub fn run_daemon(mut player: Player) -> Result<()> {
    // Start IPC server for remote control
    let ipc_stop = Arc::new(AtomicBool::new(false));
    let ipc_rx = match crate::ipc::server::start_ipc_server(ipc_stop.clone()) {
        Ok(rx) => Some(rx),
        Err(e) => {
            tracing::warn!("Failed to start IPC server: {e}");
            None
        }
    };

    // Load and play the first track
    let mut playback_handle: Option<std::thread::JoinHandle<Result<()>>> = None;
    if !player.tracks().is_empty() {
        player.play_current()?;
        playback_handle = player.start_playback();
        player.mark_playback_started();
    }

    println!("kora daemon started (PID {})", std::process::id());
    if ipc_rx.is_some() {
        println!("Control with: kora pause, kora next, kora status");
    } else {
        println!("IPC not available — playing tracks sequentially");
    }
    print_now_playing(&player);

    loop {
        // Process IPC commands
        if let Some(ref rx) = ipc_rx {
            while let Ok(msg) = rx.try_recv() {
                let (response, should_quit) =
                    handle_ipc_message(&msg, &mut player, &mut playback_handle);
                let _ = msg.response_tx.send(response);
                if should_quit {
                    stop_daemon(&player, &ipc_stop, playback_handle);
                    return Ok(());
                }
            }
        }

        // Check if playback thread finished (track ended naturally)
        if let Some(ref handle) = playback_handle {
            if handle.is_finished() {
                if let Some(h) = playback_handle.take() {
                    let _ = h.join();
                }
                let action = player.on_track_finished();
                match action {
                    PlayerAction::LoadAndPlay => {
                        player.play_current()?;
                        playback_handle = player.start_playback();
                        player.mark_playback_started();
                        print_now_playing(&player);
                    }
                    PlayerAction::GaplessTransition => {
                        if let Some(pre) = player.take_next_track_samples() {
                            player.play_predecoded(pre);
                            playback_handle = player.start_playback();
                            player.mark_playback_started();
                            print_now_playing(&player);
                        }
                    }
                    PlayerAction::Quit | PlayerAction::None => {
                        // All tracks played or queue empty
                        stop_daemon(&player, &ipc_stop, playback_handle);
                        return Ok(());
                    }
                }
            }
        } else if player.state() == PlaybackState::Stopped && ipc_rx.is_none() {
            // No IPC and no playback — nothing to do
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    stop_daemon(&player, &ipc_stop, playback_handle);
    Ok(())
}

/// Handle a single IPC message, returning the response and whether to quit.
fn handle_ipc_message(
    msg: &IpcMessage,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
) -> (IpcResponse, bool) {
    match &msg.request {
        IpcRequest::Status => {
            let state_str = match player.state() {
                PlaybackState::Playing => "playing",
                PlaybackState::Paused => "paused",
                PlaybackState::Stopped => "stopped",
            };
            let (pos, total) = player.queue_position();
            let resp = IpcResponse::with_status(PlayerStatus {
                state: state_str.to_string(),
                track: player.current_track().map(|t| t.display_name()),
                position_secs: player.current_position().as_secs_f64(),
                duration_secs: player.duration().as_secs_f64(),
                volume_db: player.volume_db(),
                queue_position: pos,
                queue_total: total,
            });
            (resp, false)
        }
        IpcRequest::Play if player.state() != PlaybackState::Playing => {
            dispatch_command(PlayerCommand::PlayPause, player, playback_handle)
        }
        IpcRequest::Play => (IpcResponse::ok(), false),
        IpcRequest::Pause if player.state() == PlaybackState::Playing => {
            dispatch_command(PlayerCommand::PlayPause, player, playback_handle)
        }
        IpcRequest::Pause => (IpcResponse::ok(), false),
        IpcRequest::Toggle => dispatch_command(PlayerCommand::PlayPause, player, playback_handle),
        IpcRequest::Stop => {
            let (resp, _) = dispatch_command(PlayerCommand::Stop, player, playback_handle);
            (resp, true)
        }
        IpcRequest::Next => {
            let result = dispatch_command(PlayerCommand::NextTrack, player, playback_handle);
            print_now_playing(player);
            result
        }
        IpcRequest::Prev => {
            let result = dispatch_command(PlayerCommand::PrevTrack, player, playback_handle);
            print_now_playing(player);
            result
        }
        IpcRequest::Volume { db } => {
            dispatch_command(PlayerCommand::SetVolume(*db), player, playback_handle)
        }
    }
}

/// Execute a player command and produce an IPC response.
fn dispatch_command(
    cmd: PlayerCommand,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
) -> (IpcResponse, bool) {
    let action = player.handle_command(cmd);
    match handle_action(action, player, playback_handle) {
        Ok(quit) => (IpcResponse::ok(), quit),
        Err(e) => (IpcResponse::error(e.to_string()), false),
    }
}

/// Execute a `PlayerAction` — returns `true` if the daemon should quit.
fn handle_action(
    action: PlayerAction,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
) -> Result<bool> {
    match action {
        PlayerAction::None => Ok(false),
        PlayerAction::LoadAndPlay => {
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            let current_idx = player.current_index();
            if let Some(pre) = player.take_next_track_samples() {
                if pre.track_index == current_idx {
                    player.play_predecoded(pre);
                } else {
                    player.play_current()?;
                }
            } else {
                player.play_current()?;
            }
            *playback_handle = player.start_playback();
            player.mark_playback_started();
            Ok(false)
        }
        PlayerAction::GaplessTransition => {
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            if let Some(pre) = player.take_next_track_samples() {
                player.play_predecoded(pre);
                *playback_handle = player.start_playback();
            } else {
                player.play_current()?;
                *playback_handle = player.start_playback();
            }
            player.mark_playback_started();
            Ok(false)
        }
        PlayerAction::Quit => Ok(true),
    }
}

fn print_now_playing(player: &Player) {
    if let Some(track) = player.current_track() {
        println!("Playing: {}", track.display_name());
    }
}

fn stop_daemon(
    player: &Player,
    ipc_stop: &Arc<AtomicBool>,
    playback_handle: Option<std::thread::JoinHandle<Result<()>>>,
) {
    player.save_session().ok();
    ipc_stop.store(true, Ordering::Relaxed);
    if let Some(h) = playback_handle {
        let _ = h.join();
    }
    println!("kora daemon stopped");
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    /// Verify `--daemon` flag is accepted by the CLI parser.
    #[test]
    fn daemon_flag_accepted() {
        let cli = crate::Cli::try_parse_from(["kora", "--daemon", "file.mp3"]);
        assert!(cli.is_ok(), "Failed to parse --daemon flag: {cli:?}");
        let cli = cli.unwrap();
        assert!(cli.daemon);
        assert_eq!(cli.inputs, vec!["file.mp3"]);
    }

    /// Verify `--daemon` without inputs is accepted.
    #[test]
    fn daemon_flag_without_inputs() {
        let cli = crate::Cli::try_parse_from(["kora", "--daemon"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert!(cli.daemon);
        assert!(cli.inputs.is_empty());
    }
}
