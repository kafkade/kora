# kora

A fast, multi-source terminal audio player built in Rust.

**kora** plays local audio files, internet radio stations, and podcasts from your terminal — with an equalizer, visualizer, themes, and more planned.

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

# Play with EQ preset
kora --eq-preset Rock ~/Music/

# List available EQ presets
kora --list-eq-presets
```

## Features

- **TUI** with track info, progress bar, playlist, and keyboard controls
- **10-band graphic EQ** with 11 built-in presets (Rock, Jazz, Pop, Classical, and more)
- **Internet radio** via Radio Browser API (30,000+ stations) and custom `stations.toml`
- **HTTP stream playback** — play any audio URL directly
- **Session persistence** — resume where you left off across restarts
- **Configuration file** — `config.toml` with sensible defaults
- **Nord color theme** — beautiful out of the box

### Keyboard Controls

| Key | Action |
|-----|--------|
| Space | Play / Pause |
| n / p | Next / Previous track |
| + / - | Volume up / down (1dB) |
| s | Stop |
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

## Status

🚧 **Early development** — Phase 2 (Daily Driver). Local files, internet radio, and HTTP streams are working. See [ROADMAP.md](ROADMAP.md) for the full plan.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
