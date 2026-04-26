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

    /// Catppuccin Mocha — a soothing pastel theme.
    pub fn catppuccin_mocha() -> Self {
        // Catppuccin Mocha palette:
        // base:    #1E1E2E   text:    #CDD6F4   accent (blue): #89B4FA
        // surface0:#313244   green:   #A6E3A1   yellow: #F9E2AF
        // red:     #F38BA8   lavender:#B4BEFE   subtext0:#A6ADC8
        Self {
            name: "Catppuccin Mocha",
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            accent: Color::Rgb(137, 180, 250),
            dim: Color::Rgb(49, 50, 68),
            track_title: Style::default()
                .fg(Color::Rgb(205, 214, 244))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(166, 173, 200)),
            progress_bar: Style::default().fg(Color::Rgb(137, 180, 250)),
            progress_bg: Style::default().bg(Color::Rgb(49, 50, 68)),
            status_playing: Style::default().fg(Color::Rgb(166, 227, 161)),
            status_paused: Style::default().fg(Color::Rgb(249, 226, 175)),
            status_stopped: Style::default().fg(Color::Rgb(243, 139, 168)),
            status_info: Style::default().fg(Color::Rgb(166, 173, 200)),
            playlist_current: Style::default()
                .fg(Color::Rgb(137, 180, 250))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(205, 214, 244)),
            border: Style::default().fg(Color::Rgb(49, 50, 68)),
            title: Style::default().fg(Color::Rgb(180, 190, 254)),
            help_key: Style::default().fg(Color::Rgb(249, 226, 175)),
            help_text: Style::default().fg(Color::Rgb(205, 214, 244)),
        }
    }

    /// Gruvbox — a retro groove color scheme.
    pub fn gruvbox() -> Self {
        // Gruvbox dark palette:
        // bg:     #282828   fg:     #EBDBB2   accent (aqua): #83A598
        // bg1:    #3C3836   green:  #B8BB26   yellow: #FABD2F
        // red:    #FB4934   orange: #FE8019   blue:   #83A598
        Self {
            name: "Gruvbox",
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            accent: Color::Rgb(131, 165, 152),
            dim: Color::Rgb(60, 56, 54),
            track_title: Style::default()
                .fg(Color::Rgb(235, 219, 178))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(146, 131, 116)),
            progress_bar: Style::default().fg(Color::Rgb(131, 165, 152)),
            progress_bg: Style::default().bg(Color::Rgb(60, 56, 54)),
            status_playing: Style::default().fg(Color::Rgb(184, 187, 38)),
            status_paused: Style::default().fg(Color::Rgb(250, 189, 47)),
            status_stopped: Style::default().fg(Color::Rgb(251, 73, 52)),
            status_info: Style::default().fg(Color::Rgb(146, 131, 116)),
            playlist_current: Style::default()
                .fg(Color::Rgb(131, 165, 152))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(235, 219, 178)),
            border: Style::default().fg(Color::Rgb(60, 56, 54)),
            title: Style::default().fg(Color::Rgb(254, 128, 25)),
            help_key: Style::default().fg(Color::Rgb(250, 189, 47)),
            help_text: Style::default().fg(Color::Rgb(235, 219, 178)),
        }
    }

    /// Tokyo Night — a clean dark theme inspired by Tokyo city lights.
    pub fn tokyo_night() -> Self {
        // Tokyo Night palette:
        // bg:     #1A1B26   fg:     #A9B1D6   accent (blue): #7AA2F7
        // bg_dk:  #16161E   green:  #9ECE6A   yellow: #E0AF68
        // red:    #F7768E   purple: #BB9AF7   comment:#565F89
        Self {
            name: "Tokyo Night",
            bg: Color::Rgb(26, 27, 38),
            fg: Color::Rgb(169, 177, 214),
            accent: Color::Rgb(122, 162, 247),
            dim: Color::Rgb(86, 95, 137),
            track_title: Style::default()
                .fg(Color::Rgb(169, 177, 214))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(86, 95, 137)),
            progress_bar: Style::default().fg(Color::Rgb(122, 162, 247)),
            progress_bg: Style::default().bg(Color::Rgb(22, 22, 30)),
            status_playing: Style::default().fg(Color::Rgb(158, 206, 106)),
            status_paused: Style::default().fg(Color::Rgb(224, 175, 104)),
            status_stopped: Style::default().fg(Color::Rgb(247, 118, 142)),
            status_info: Style::default().fg(Color::Rgb(86, 95, 137)),
            playlist_current: Style::default()
                .fg(Color::Rgb(122, 162, 247))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(169, 177, 214)),
            border: Style::default().fg(Color::Rgb(86, 95, 137)),
            title: Style::default().fg(Color::Rgb(187, 154, 247)),
            help_key: Style::default().fg(Color::Rgb(224, 175, 104)),
            help_text: Style::default().fg(Color::Rgb(169, 177, 214)),
        }
    }

    /// Rosé Pine — all natural pine, faux fur, and a bit of soho vibes.
    pub fn rose_pine() -> Self {
        // Rosé Pine palette:
        // base:    #191724   text:    #E0DEF4   accent (iris): #C4A7E7
        // surface: #1F1D2E   love:    #EB6F92   gold:   #F6C177
        // pine:    #31748F   foam:    #9CCFD8   muted:  #6E6A86
        Self {
            name: "Rosé Pine",
            bg: Color::Rgb(25, 23, 36),
            fg: Color::Rgb(224, 222, 244),
            accent: Color::Rgb(196, 167, 231),
            dim: Color::Rgb(110, 106, 134),
            track_title: Style::default()
                .fg(Color::Rgb(224, 222, 244))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(110, 106, 134)),
            progress_bar: Style::default().fg(Color::Rgb(196, 167, 231)),
            progress_bg: Style::default().bg(Color::Rgb(31, 29, 46)),
            status_playing: Style::default().fg(Color::Rgb(156, 207, 216)),
            status_paused: Style::default().fg(Color::Rgb(246, 193, 119)),
            status_stopped: Style::default().fg(Color::Rgb(235, 111, 146)),
            status_info: Style::default().fg(Color::Rgb(110, 106, 134)),
            playlist_current: Style::default()
                .fg(Color::Rgb(196, 167, 231))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(224, 222, 244)),
            border: Style::default().fg(Color::Rgb(110, 106, 134)),
            title: Style::default().fg(Color::Rgb(49, 116, 143)),
            help_key: Style::default().fg(Color::Rgb(246, 193, 119)),
            help_text: Style::default().fg(Color::Rgb(224, 222, 244)),
        }
    }

    /// Dracula — a dark theme for all things.
    pub fn dracula() -> Self {
        // Dracula palette:
        // bg:      #282A36   fg:      #F8F8F2   accent (purple): #BD93F9
        // current: #44475A   green:   #50FA7B   yellow: #F1FA8C
        // red:     #FF5555   pink:    #FF79C6   cyan:   #8BE9FD
        Self {
            name: "Dracula",
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(189, 147, 249),
            dim: Color::Rgb(68, 71, 90),
            track_title: Style::default()
                .fg(Color::Rgb(248, 248, 242))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(98, 114, 164)),
            progress_bar: Style::default().fg(Color::Rgb(189, 147, 249)),
            progress_bg: Style::default().bg(Color::Rgb(68, 71, 90)),
            status_playing: Style::default().fg(Color::Rgb(80, 250, 123)),
            status_paused: Style::default().fg(Color::Rgb(241, 250, 140)),
            status_stopped: Style::default().fg(Color::Rgb(255, 85, 85)),
            status_info: Style::default().fg(Color::Rgb(98, 114, 164)),
            playlist_current: Style::default()
                .fg(Color::Rgb(189, 147, 249))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(248, 248, 242)),
            border: Style::default().fg(Color::Rgb(68, 71, 90)),
            title: Style::default().fg(Color::Rgb(139, 233, 253)),
            help_key: Style::default().fg(Color::Rgb(241, 250, 140)),
            help_text: Style::default().fg(Color::Rgb(248, 248, 242)),
        }
    }

    /// Solarized Dark — precision colors for machines and people.
    pub fn solarized_dark() -> Self {
        // Solarized Dark palette:
        // base03: #002B36   base0:  #839496   blue:   #268BD2
        // base02: #073642   green:  #859900   yellow: #B58900
        // red:    #DC322F   cyan:   #2AA198   base01: #586E75
        Self {
            name: "Solarized Dark",
            bg: Color::Rgb(0, 43, 54),
            fg: Color::Rgb(131, 148, 150),
            accent: Color::Rgb(38, 139, 210),
            dim: Color::Rgb(88, 110, 117),
            track_title: Style::default()
                .fg(Color::Rgb(131, 148, 150))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(88, 110, 117)),
            progress_bar: Style::default().fg(Color::Rgb(38, 139, 210)),
            progress_bg: Style::default().bg(Color::Rgb(7, 54, 66)),
            status_playing: Style::default().fg(Color::Rgb(133, 153, 0)),
            status_paused: Style::default().fg(Color::Rgb(181, 137, 0)),
            status_stopped: Style::default().fg(Color::Rgb(220, 50, 47)),
            status_info: Style::default().fg(Color::Rgb(88, 110, 117)),
            playlist_current: Style::default()
                .fg(Color::Rgb(38, 139, 210))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(131, 148, 150)),
            border: Style::default().fg(Color::Rgb(88, 110, 117)),
            title: Style::default().fg(Color::Rgb(42, 161, 152)),
            help_key: Style::default().fg(Color::Rgb(181, 137, 0)),
            help_text: Style::default().fg(Color::Rgb(131, 148, 150)),
        }
    }

    /// One Dark — Atom's iconic dark theme.
    pub fn one_dark() -> Self {
        // One Dark palette:
        // bg:       #282C34   fg:      #ABB2BF   accent (blue): #61AFEF
        // gutter:   #4B5263   green:   #98C379   yellow: #E5C07B
        // red:      #E06C75   purple:  #C678DD   cyan:   #56B6C2
        Self {
            name: "One Dark",
            bg: Color::Rgb(40, 44, 52),
            fg: Color::Rgb(171, 178, 191),
            accent: Color::Rgb(97, 175, 239),
            dim: Color::Rgb(75, 82, 99),
            track_title: Style::default()
                .fg(Color::Rgb(171, 178, 191))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(75, 82, 99)),
            progress_bar: Style::default().fg(Color::Rgb(97, 175, 239)),
            progress_bg: Style::default().bg(Color::Rgb(50, 55, 66)),
            status_playing: Style::default().fg(Color::Rgb(152, 195, 121)),
            status_paused: Style::default().fg(Color::Rgb(229, 192, 123)),
            status_stopped: Style::default().fg(Color::Rgb(224, 108, 117)),
            status_info: Style::default().fg(Color::Rgb(75, 82, 99)),
            playlist_current: Style::default()
                .fg(Color::Rgb(97, 175, 239))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(171, 178, 191)),
            border: Style::default().fg(Color::Rgb(75, 82, 99)),
            title: Style::default().fg(Color::Rgb(198, 120, 221)),
            help_key: Style::default().fg(Color::Rgb(229, 192, 123)),
            help_text: Style::default().fg(Color::Rgb(171, 178, 191)),
        }
    }

    /// Kanagawa — a dark theme inspired by Katsushika Hokusai's famous painting.
    pub fn kanagawa() -> Self {
        // Kanagawa palette:
        // bg:        #1F1F28   fg:       #DCD7BA   accent (crystal): #7E9CD8
        // bg_dk:     #16161D   spring green:#98BB6C  carp yellow:#E6C384
        // samurai red:#C34043  wave blue: #7E9CD8   fuji gray: #727169
        Self {
            name: "Kanagawa",
            bg: Color::Rgb(31, 31, 40),
            fg: Color::Rgb(220, 215, 186),
            accent: Color::Rgb(126, 156, 216),
            dim: Color::Rgb(114, 113, 105),
            track_title: Style::default()
                .fg(Color::Rgb(220, 215, 186))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(114, 113, 105)),
            progress_bar: Style::default().fg(Color::Rgb(126, 156, 216)),
            progress_bg: Style::default().bg(Color::Rgb(22, 22, 29)),
            status_playing: Style::default().fg(Color::Rgb(152, 187, 108)),
            status_paused: Style::default().fg(Color::Rgb(230, 195, 132)),
            status_stopped: Style::default().fg(Color::Rgb(195, 64, 67)),
            status_info: Style::default().fg(Color::Rgb(114, 113, 105)),
            playlist_current: Style::default()
                .fg(Color::Rgb(126, 156, 216))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(220, 215, 186)),
            border: Style::default().fg(Color::Rgb(114, 113, 105)),
            title: Style::default().fg(Color::Rgb(210, 126, 153)),
            help_key: Style::default().fg(Color::Rgb(230, 195, 132)),
            help_text: Style::default().fg(Color::Rgb(220, 215, 186)),
        }
    }

    /// Matte Black — a minimal high-contrast dark theme.
    pub fn matte_black() -> Self {
        // Custom matte black palette:
        // bg:     #000000   fg:     #B0B0B0   accent: #4A9BD9
        // dim:    #333333   green:  #5FA85F   yellow: #D4A843
        // red:    #C75050   title:  #6EB5FF
        Self {
            name: "Matte Black",
            bg: Color::Rgb(0, 0, 0),
            fg: Color::Rgb(176, 176, 176),
            accent: Color::Rgb(74, 155, 217),
            dim: Color::Rgb(51, 51, 51),
            track_title: Style::default()
                .fg(Color::Rgb(176, 176, 176))
                .add_modifier(Modifier::BOLD),
            track_position: Style::default().fg(Color::Rgb(85, 85, 85)),
            progress_bar: Style::default().fg(Color::Rgb(74, 155, 217)),
            progress_bg: Style::default().bg(Color::Rgb(30, 30, 30)),
            status_playing: Style::default().fg(Color::Rgb(95, 168, 95)),
            status_paused: Style::default().fg(Color::Rgb(212, 168, 67)),
            status_stopped: Style::default().fg(Color::Rgb(199, 80, 80)),
            status_info: Style::default().fg(Color::Rgb(85, 85, 85)),
            playlist_current: Style::default()
                .fg(Color::Rgb(74, 155, 217))
                .add_modifier(Modifier::BOLD),
            playlist_normal: Style::default().fg(Color::Rgb(176, 176, 176)),
            border: Style::default().fg(Color::Rgb(51, 51, 51)),
            title: Style::default().fg(Color::Rgb(110, 181, 255)),
            help_key: Style::default().fg(Color::Rgb(212, 168, 67)),
            help_text: Style::default().fg(Color::Rgb(176, 176, 176)),
        }
    }

    /// Get the default theme.
    pub fn default_theme() -> Self {
        Self::nord()
    }
}

/// All built-in themes in cycle order.
pub fn all_themes() -> Vec<Theme> {
    vec![
        Theme::nord(),
        Theme::catppuccin_mocha(),
        Theme::gruvbox(),
        Theme::tokyo_night(),
        Theme::rose_pine(),
        Theme::dracula(),
        Theme::solarized_dark(),
        Theme::one_dark(),
        Theme::kanagawa(),
        Theme::matte_black(),
    ]
}

/// Find a theme by name (case-insensitive).
pub fn find_theme(name: &str) -> Option<Theme> {
    all_themes()
        .into_iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
}

/// List all available theme names.
pub fn theme_names() -> Vec<&'static str> {
    vec![
        "Nord",
        "Catppuccin Mocha",
        "Gruvbox",
        "Tokyo Night",
        "Rosé Pine",
        "Dracula",
        "Solarized Dark",
        "One Dark",
        "Kanagawa",
        "Matte Black",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_have_non_empty_names() {
        for theme in all_themes() {
            assert!(!theme.name.is_empty(), "Theme has empty name");
        }
    }

    #[test]
    fn find_theme_case_insensitive() {
        assert!(find_theme("nord").is_some());
        assert!(find_theme("NORD").is_some());
        assert!(find_theme("Nord").is_some());
        assert!(find_theme("dracula").is_some());
        assert!(find_theme("DRACULA").is_some());
        assert!(find_theme("gruvbox").is_some());
        assert!(find_theme("TOKYO NIGHT").is_some());
        assert!(find_theme("nonexistent").is_none());
    }

    #[test]
    fn all_themes_returns_at_least_10() {
        assert!(
            all_themes().len() >= 10,
            "Expected at least 10 themes, got {}",
            all_themes().len()
        );
    }

    #[test]
    fn theme_names_matches_all_themes() {
        let names = theme_names();
        let themes = all_themes();
        assert_eq!(names.len(), themes.len());
        for (name, theme) in names.iter().zip(themes.iter()) {
            assert_eq!(*name, theme.name);
        }
    }

    #[test]
    fn default_theme_is_nord() {
        let theme = Theme::default_theme();
        assert_eq!(theme.name, "Nord");
    }
}
