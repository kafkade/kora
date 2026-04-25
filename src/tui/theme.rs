//! TUI color theme definitions.

use ratatui::style::{Color, Modifier, Style};

/// A color theme for the TUI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub dim: Color,
    pub track_title: Style,
    pub track_position: Style,
    pub progress_bar: Style,
    pub progress_bg: Style,
    pub status_playing: Style,
    pub status_paused: Style,
    pub status_stopped: Style,
    pub status_info: Style,
    pub playlist_current: Style,
    pub playlist_normal: Style,
    pub border: Style,
    pub title: Style,
    pub help_key: Style,
    pub help_text: Style,
}

impl Theme {
    /// The default Nord-inspired theme.
    pub fn nord() -> Self {
        // Nord palette:
        // bg:     #2E3440 (Polar Night)
        // fg:     #D8DEE9 (Snow Storm)
        // accent: #88C0D0 (Frost)
        // dim:    #4C566A (Polar Night lighter)
        // green:  #A3BE8C (Aurora)
        // yellow: #EBCB8B (Aurora)
        // red:    #BF616A (Aurora)
        // blue:   #81A1C1 (Frost)
        Self {
            name: "Nord",
            bg: Color::Rgb(46, 52, 64),
            fg: Color::Rgb(216, 222, 233),
            accent: Color::Rgb(136, 192, 208),
            dim: Color::Rgb(76, 86, 106),
            track_title: Style::default()
                .fg(Color::Rgb(216, 222, 233))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(76, 86, 106)),
            progress_bar: Style::default().fg(Color::Rgb(136, 192, 208)),
            progress_bg: Style::default().bg(Color::Rgb(59, 66, 82)),
            status_playing: Style::default().fg(Color::Rgb(163, 190, 140)),
            status_paused: Style::default().fg(Color::Rgb(235, 203, 139)),
            status_stopped: Style::default().fg(Color::Rgb(191, 97, 106)),
            status_info: Style::default().fg(Color::Rgb(76, 86, 106)),
            playlist_current: Style::default()
                .fg(Color::Rgb(136, 192, 208))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(216, 222, 233)),
            border: Style::default().fg(Color::Rgb(76, 86, 106)),
            title: Style::default().fg(Color::Rgb(129, 161, 193)),
            help_key: Style::default().fg(Color::Rgb(235, 203, 139)),
            help_text: Style::default().fg(Color::Rgb(216, 222, 233)),
        }
    }

    /// Get the default theme.
    pub fn default_theme() -> Self {
        Self::nord()
    }
}
