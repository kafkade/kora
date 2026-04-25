//! TUI application — ratatui event loop, rendering, and input handling.

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::core::session::Session;
use crate::playback::player::{PlaybackState, Player, PlayerAction, PlayerCommand};

/// Run the TUI application.
pub fn run(mut player: Player, no_restore: bool) -> Result<()> {
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

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut player, &mut playback_handle);

    // Restore terminal
    ratatui::restore();
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let save_interval = Duration::from_secs(30);
    let mut last_save = Instant::now();

    loop {
        // Draw
        terminal.draw(|frame| draw(frame, player))?;

        // Check if playback thread finished (track ended naturally)
        if let Some(handle) = playback_handle.as_ref()
            && handle.is_finished()
        {
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            let action = player.on_track_finished();
            handle_action(action, player, playback_handle)?;
        }

        // Auto-save session every 30 seconds
        if last_save.elapsed() >= save_interval {
            if let Err(e) = player.save_session() {
                tracing::warn!("Failed to auto-save session: {e}");
            }
            last_save = Instant::now();
        }

        // Poll for keyboard input
        if event::poll(tick_rate)?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
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
                handle_action(action, player, playback_handle)?;
            }
        }
    }
}

fn handle_action(
    action: PlayerAction,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
) -> Result<()> {
    match action {
        PlayerAction::None => {}
        PlayerAction::LoadAndPlay => {
            // Stop and join previous playback thread
            if let Some(h) = playback_handle.take() {
                let _ = h.join();
            }
            player.play_current()?;
            *playback_handle = player.start_playback();
            player.mark_playback_started();
        }
        PlayerAction::Quit => {}
    }
    Ok(())
}

fn draw(frame: &mut Frame, player: &Player) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Track info + progress
            Constraint::Min(3),    // Playlist
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    draw_track_info(frame, chunks[0], player);
    draw_playlist(frame, chunks[1], player);
    draw_status_bar(frame, chunks[2], player);
}

fn draw_track_info(frame: &mut Frame, area: Rect, player: &Player) {
    let block = Block::default().borders(Borders::ALL).title(" kora ");

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
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            track_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(track_line), chunks[0]);

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
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .ratio(ratio)
        .label(progress_label);
    frame.render_widget(gauge, chunks[1]);

    // Status line
    let state_icon = match player.state() {
        PlaybackState::Playing => "▶ Playing",
        PlaybackState::Paused => "⏸ Paused",
        PlaybackState::Stopped => "■ Stopped",
    };

    let eq_info = player
        .eq_preset_name()
        .map(|n| format!("  EQ: {n}"))
        .unwrap_or_default();

    let status = Line::from(vec![
        Span::styled(state_icon, Style::default().fg(Color::Green)),
        Span::styled(
            format!("  Vol: {:+.0}dB{eq_info}", player.volume_db()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(status), chunks[2]);
}

fn draw_playlist(frame: &mut Frame, area: Rect, player: &Player) {
    let block = Block::default().borders(Borders::ALL).title(" Playlist ");

    let items: Vec<ListItem> = player
        .tracks()
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = i == player.current_index();
            let prefix = if is_current { "▶ " } else { "  " };
            let style = if is_current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{prefix}{}. {}", i + 1, track.display_name()),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_status_bar(frame: &mut Frame, area: Rect, _player: &Player) {
    let help = Line::from(vec![
        Span::styled("Spc", Style::default().fg(Color::Yellow)),
        Span::raw(":Play/Pause "),
        Span::styled("n/p", Style::default().fg(Color::Yellow)),
        Span::raw(":Next/Prev "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(":Stop "),
        Span::styled("+/-", Style::default().fg(Color::Yellow)),
        Span::raw(":Vol "),
        Span::styled("←/→", Style::default().fg(Color::Yellow)),
        Span::raw(":Seek "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(":Quit"),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}
