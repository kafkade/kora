# kora

A fast, multi-source terminal audio player built in Rust.

**kora** plays local audio files, internet radio stations, and podcasts from your terminal — with an equalizer, visualizer, themes, lyrics, gapless playback, and more.

### Why "kora"?

The [kora](https://en.wikipedia.org/wiki/Kora_(instrument)) is a 21-string West African harp — one of the most beautiful and versatile acoustic instruments in the world. Built from a calabash gourd, a hardwood neck, and cowhide, it produces a sound often compared to a harp or a flamenco guitar: rich, resonant, and unmistakable.

The name reflects the project's values:

- **Craftsmanship** — the kora is handmade by artisans, each one unique. This player is built with the same care: hand-tuned audio pipeline, no shortcuts.
- **Versatility** — a single kora covers melody, bass, and rhythm simultaneously. This player handles local files, radio, and podcasts in one tool.
- **Tradition meets modernity** — the kora is an ancient instrument that thrives in modern music, from Toumani Diabaté to electronic collaborations. This player brings a modern Rust architecture to the timeless act of listening.
- **Short and distinctive** — four characters, easy to type in a terminal, easy to remember.

## Quick Start

```sh
# Play a file
kora song.mp3

# Play a directory
kora ~/Music/

# Play multiple files
kora *.flac

# Play an HTTP stream
kora https://example.com/stream.mp3

# Search and play internet radio
kora --radio "lofi hip hop"

# Play a podcast
kora --podcast "https://feeds.example.com/show.rss"

# Play with EQ preset
kora --eq-preset Rock ~/Music/

# Run as a background daemon (controlled via IPC)
kora --daemon ~/Music/
```

## Features

- **TUI** with track info, progress bar, playlist, and full keyboard controls
- **10-band graphic EQ** with 11 built-in presets, interactive visual adjustment, and custom presets in `config.toml`
- **Spectrum visualizer** — 32-bar FFT display with `v` toggle and `V` full-screen mode
- **10 color themes** — Nord, Catppuccin, Gruvbox, Tokyo Night, Rosé Pine, Dracula, and more. Press `t` to cycle.
- **Internet radio** via Radio Browser API (30,000+ stations) and custom `stations.toml`
- **Podcast client** — RSS feeds, OPML import/export, episode downloads, chapter support, subscription management
- **HTTP stream playback** — play any audio URL directly
- **Gapless playback** — pre-decode next track, seamless transitions
- **Synced lyrics** — LRC files and embedded tags, auto-scrolling display
- **Playback speed** — 0.25x to 3.0x with `]`/`[` keys
- **ReplayGain** — automatic volume normalization from tags
- **Session persistence** — resume where you left off across restarts
- **Shuffle and repeat** — Fisher-Yates shuffle, repeat all/one modes
- **Favorites** — star tracks with `f`, persisted in `favorites.toml`
- **Sleep timer** — Shift+S to set, volume fade-out in last 30 seconds
- **Audio device selection** — list, switch, and persist output device
- **IPC remote control** — `kora pause`, `kora next`, `kora status` from another terminal
- **MPRIS / media keys** — system media controls on Linux, macOS, and Windows
- **Headless/daemon mode** — `--daemon` runs without TUI, controlled via IPC
- **Shell completions** — bash, zsh, fish, PowerShell
- **Configuration file** — `config.toml` with sensible defaults

### Keyboard Controls

| Key | Action |
|-----|--------|
| Space | Play / Pause |
| n / p | Next / Previous track |
| + / - | Volume up / down (1dB) |
| ] / [ | Speed up / down (0.25x) |
| s | Stop |
| z | Toggle shuffle |
| r | Cycle repeat (Off → All → One) |
| f | Toggle favorite ★ |
| e | Toggle EQ view (h/l: band, j/k: gain) |
| v / V | Toggle visualizer / full-screen |
| y | Toggle synced lyrics |
| t | Cycle color theme |
| d | Cycle audio device |
| o | Open file browser |
| P | Open podcast manager |
| Shift+S | Cycle sleep timer |
| q | Quit (auto-saves session) |

## Supported Formats

MP3, FLAC, OGG Vorbis, Opus, WAV, AAC, ALAC (M4A) — decoded natively in Rust via [symphonia](https://github.com/pdeljanov/Symphonia) (no C dependencies).

## Install

### From crates.io

```sh
cargo install kora-player
```

### From source

```sh
git clone https://github.com/kafkade/kora.git
cd kora
cargo install --path .
```

### Prerequisites

- **Rust** 1.80+ (install via [rustup](https://rustup.rs/))
- **Linux**: ALSA development headers (`sudo apt install libasound2-dev` on Debian/Ubuntu)
- **macOS**: No extra dependencies (CoreAudio)
- **Windows**: No extra dependencies (WASAPI)

## Shell Completions

Generate completions for your shell:

```sh
# Bash
kora completions bash > ~/.local/share/bash-completion/completions/kora

# Zsh
kora completions zsh > ~/.zfunc/_kora

# Fish
kora completions fish > ~/.config/fish/completions/kora.fish

# PowerShell
kora completions powershell > _kora.ps1
```

## Remote Control

Control a running kora instance from another terminal:

```sh
kora pause          # Pause playback
kora toggle         # Toggle play/pause
kora next           # Next track
kora prev           # Previous track
kora volume -3      # Set volume to -3dB
kora status         # JSON status output
```

## Status

**Active development** — Phases 0–5 complete. Local files, radio, podcasts, TUI, EQ, visualizer, gapless, lyrics, IPC, media keys, and daemon mode are all working. See [ROADMAP.md](ROADMAP.md) for the full plan.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
