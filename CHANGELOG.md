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
- Spectrum visualizer: press `v` to toggle 32-bar frequency display, `V` for full-screen mode
- Visualizer uses FFT with Hanning window, log-frequency binning, and smoothing for natural motion
- Gapless playback: next track pre-decoded at 80% progress, seamless ring buffer transition with no audible gap
- Gapless works across same-format tracks; falls back gracefully on sample rate or channel mismatch
- Playback speed control: press `]`/`[` to adjust ±0.25x (range 0.25x–3.0x), speed shown in status bar
- ReplayGain volume normalization: reads tags from ID3v2, Vorbis Comments, and MP4 files
- ReplayGain applies track gain by default, album gain as option, configurable via `replaygain` in `config.toml`
- ReplayGain info displayed in TUI status bar when active
- Audio output device selection: `--list-devices` to list, `--device <NAME>` to select, press `d` to cycle in TUI
- Default audio device configurable via `audio_device` in `config.toml`
- Graceful fallback to default device when configured device is disconnected
- Synced lyrics display: press `y` to toggle, auto-scrolling with current line highlighted
- Lyrics loaded from `.lrc` sidecar files or embedded tags (ID3v2 USLT, Vorbis LYRICS)
- AAC and ALAC (M4A) format support via symphonia
- Helpful error message when attempting to play WMA files (requires ffmpeg)
- OPML import/export: `--import-opml <FILE>` and `--export-opml <FILE>` for podcast subscription portability
- Podcast subscription management in TUI: press `P` to open podcast view
- Add podcast feeds by URL, remove subscriptions, refresh feeds from within the TUI
- Podcast episode browser with played/unplayed indicators, duration, and publish dates
- Podcast episode downloads: press `D` in podcast view to download, `C` to clean up played episodes
- Configurable download directory, auto-delete played episodes, and storage limit in `config.toml`
- Downloaded episodes play from local files instead of streaming
- Podcast chapter support: Podlove Simple Chapters (PSC) parsed from RSS feeds
- Current chapter displayed in status bar (e.g., "Ch 2/5: Interview")
- IPC remote control: control a running kora instance via Unix socket from another terminal
- CLI subcommands: `kora play`, `kora pause`, `kora toggle`, `kora stop`, `kora next`, `kora prev`, `kora volume <DB>`, `kora status`
- `kora status` returns JSON with current state, track, position, volume, and queue info
- MPRIS / media key integration via souvlaki (Linux D-Bus, macOS MediaRemote, Windows SMTC)
- Hardware media keys (play/pause/next/prev) control kora from keyboard or Bluetooth headphones
- Now-playing metadata displayed in system media widgets (Control Center, playerctl, etc.)
- Shell completions: `kora completions bash|zsh|fish|powershell` generates shell-specific completions
- Headless/daemon mode: `kora --daemon` runs without TUI, controlled entirely via IPC subcommands

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
