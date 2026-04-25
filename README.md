# kora

A fast, multi-source terminal audio player built in Rust.

**kora** plays local audio files, internet radio stations, and podcasts from your terminal — with an equalizer, visualizer, themes, and more planned.

## Quick Start

```sh
# Play a file
kora song.mp3

# Play a directory
kora ~/Music/

# Play multiple files
kora *.flac
```

## Supported Formats

MP3, FLAC, OGG Vorbis, Opus, WAV — decoded natively in Rust via [symphonia](https://github.com/pdeljanov/Symphonia) (no C dependencies).

## Install

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

## Status

🚧 **Early development** — Phase 0 (First Sound). See [ROADMAP.md](ROADMAP.md) for the full plan.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
