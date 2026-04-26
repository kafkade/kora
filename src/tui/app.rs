//! TUI application — ratatui event loop, rendering, and input handling.

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use super::file_browser::FileBrowser;
use super::theme::{self, Theme};
use crate::core::session::Session;
use crate::core::track::Track;
use crate::playback::player::{PlaybackState, Player, PlayerAction, PlayerCommand};

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
    let result = run_loop(
        &mut terminal,
        &mut player,
        &mut playback_handle,
        &mut theme,
        &themes,
        &mut theme_index,
    );

    // Restore terminal
    ratatui::restore();
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    player: &mut Player,
    playback_handle: &mut Option<std::thread::JoinHandle<Result<()>>>,
    theme: &mut Theme,
    themes: &[Theme],
    theme_index: &mut usize,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let save_interval = Duration::from_secs(30);
    let mut last_save = Instant::now();
    let mut file_browser: Option<FileBrowser> = None;

    loop {
        // Draw
        terminal.draw(|frame| draw(frame, player, theme, file_browser.as_ref()))?;

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
                    handle_action(action, player, playback_handle)?;
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
                KeyCode::Char('t') => {
                    *theme_index = (*theme_index + 1) % themes.len();
                    *theme = themes[*theme_index].clone();
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

fn draw(frame: &mut Frame, player: &Player, theme: &Theme, file_browser: Option<&FileBrowser>) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Track info + progress
            Constraint::Min(3),    // Playlist
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    draw_track_info(frame, chunks[0], player, theme);
    draw_playlist(frame, chunks[1], player, theme);
    draw_status_bar(frame, chunks[2], player, theme);

    // File browser overlay
    if let Some(browser) = file_browser {
        draw_file_browser(frame, area, browser, theme);
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

    let eq_info = player
        .eq_preset_name()
        .map(|n| format!("  EQ: {n}"))
        .unwrap_or_default();

    let status = Line::from(vec![
        Span::styled(state_icon, state_style),
        Span::styled(
            format!(
                "  Vol: {:+.0}dB{eq_info}  Theme: {}",
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
            let style = if is_current {
                theme.playlist_current
            } else {
                theme.playlist_normal
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

fn draw_status_bar(frame: &mut Frame, area: Rect, _player: &Player, theme: &Theme) {
    let help = Line::from(vec![
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
        Span::styled("t", theme.help_key),
        Span::styled(":Theme ", theme.help_text),
        Span::styled("o", theme.help_key),
        Span::styled(":Browse ", theme.help_text),
        Span::styled("q", theme.help_key),
        Span::styled(":Quit", theme.help_text),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
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
