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
