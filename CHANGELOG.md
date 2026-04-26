# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Shuffle mode: press `z` to toggle shuffle (Fisher-Yates), shuffled order respected by next/prev
- Repeat modes: press `r` to cycle Off → All → One
- Shuffle and repeat state displayed in TUI status bar and persisted in session
- Favorites: press `f` to star/unstar tracks, ★ indicator in playlist and track info
- Favorites persisted in `favorites.toml`
- Sleep timer: press `Shift+S` to cycle presets (15/30/45/60/90 minutes), countdown shown in status bar
- Sleep timer fades volume over the last 30 seconds before stopping playback
- Interactive EQ view: press `e` to toggle, `h`/`l` to select band, `j`/`k` to adjust gain ±1dB
- Visual EQ display with vertical bars per band, selected band highlighted
- Preset cycling within EQ view, display shows "Custom" when gains are manually adjusted
- Custom EQ presets definable in `config.toml`

### Changed

- Crate published as `kora-player` on crates.io (binary is still `kora`). Install with `cargo install kora-player`

## [0.2.0] - 2026-04-26

### Added

- Audio pipeline: symphonia decode → rtrb ring buffer → CPAL output
- Local file playback: MP3, FLAC, OGG Vorbis, Opus, WAV
- CLI with clap: `kora <file>` plays audio
- Multi-file queue: play directories and multiple files in sequence
- Volume control via `--volume` CLI flag
- Ctrl+C handling for clean shutdown
- CI pipeline: GitHub Actions for Linux, macOS, Windows
- Project documentation: ROADMAP.md, ADR-001 through ADR-005
- Dual MIT + Apache 2.0 licensing
- Graceful handling of corrupt and truncated audio files (skip bad frames, continue playback)
- Decode performance reporting in log output (realtime multiplier)
- TUI with track info, progress bar, and transport status (Playing/Paused/Stopped)
- Keyboard controls: Space (play/pause), n/p (next/prev), +/- (volume), s (stop), q (quit)
- Playlist panel showing queue with current track highlighted
- 10-band graphic equalizer with 11 presets (Flat, Rock, Pop, Jazz, Classical, Electronic, Hip Hop, Acoustic, Bass Boost, Treble Boost, Vocal)
- `--eq-preset` CLI flag and `--list-eq-presets` to show available presets
- Low-shelf filter at 31Hz and high-shelf at highest band for natural EQ response
- Live volume adjustment: +/- keys take effect on currently playing audio immediately
- Session persistence: auto-save on quit and every 30 seconds, restore on next launch (starts paused)
- `--no-restore` CLI flag to skip session restore and start fresh
- Configuration file: `config.toml` with defaults for volume, music directory, theme, EQ preset, and buffer size
- `config.toml.example` shipped in repo with all options documented
- CLI flags override config values for the session
- Nord color theme with distinct colors for playback state, playlist, progress bar, and help bar
- HTTP/HTTPS URL playback: `kora https://example.com/stream.mp3` streams and plays audio
- Internet radio search via Radio Browser API: `kora --radio "lofi hip hop"` finds and plays stations
- Custom radio stations via `stations.toml` configuration file
- `stations.toml.example` shipped in repo with sample stations
- Basic podcast playback: `kora --podcast <RSS_URL>` fetches feed, lists episodes, and plays the most recent
- Podcast state persistence: episode positions saved in `podcasts.toml`
- 10 built-in color themes: Nord, Catppuccin Mocha, Gruvbox, Tokyo Night, Rosé Pine, Dracula, Solarized Dark, One Dark, Kanagawa, Matte Black
- Press `t` to cycle themes during playback, theme name shown in status bar
- `--theme` CLI flag and `--list-themes` to show available themes
- File browser overlay: press `o` to browse directories, select audio files to queue and play

### Fixed

- Audio playback crash caused by unsafe ring buffer Consumer handling in CPAL callback
- Pause/resume: playback now pauses and resumes in place without re-decoding the track
