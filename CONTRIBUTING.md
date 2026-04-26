# Contributing to kora

Thank you for your interest in contributing to kora! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). Please be respectful and constructive in all interactions.

## How to Contribute

### Reporting Bugs

Before reporting a bug:

1. Check existing [issues](https://github.com/kafkade/kora/issues) to avoid duplicates
2. Gather relevant information:
   - kora version (`kora --version`)
   - Operating system and version
   - Audio backend (Linux: PipeWire, PulseAudio, or ALSA)
   - Steps to reproduce
   - Audio file format and details (if relevant)
   - Log output (`RUST_LOG=debug kora <args>`)

Use the [bug report template](https://github.com/kafkade/kora/issues/new?template=bug_report.yml) to create an issue.

### Suggesting Features

1. Check existing issues and discussions for similar suggestions
2. Open a [feature request](https://github.com/kafkade/kora/issues/new?template=feature_request.yml) with:
   - Clear description of the feature
   - Use case and motivation
   - Proposed implementation (if you have ideas)

### Submitting Code

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Run lints (`cargo clippy -- -D warnings`)
6. Format code (`cargo fmt`)
7. Commit with a descriptive message using conventional commits
8. Push to your fork
9. Open a Pull Request

## Development Setup

### Prerequisites

- **Rust**: 1.80 or later ([rustup.rs](https://rustup.rs/))
- **Linux**: ALSA development headers (`sudo apt install libasound2-dev` on Debian/Ubuntu, `sudo dnf install alsa-lib-devel` on Fedora)
- **macOS**: No extra dependencies (CoreAudio)
- **Windows**: No extra dependencies (WASAPI)

### Building

```sh
# Clone your fork
git clone https://github.com/YOUR_USERNAME/kora.git
cd kora

# Build debug version
cargo build

# Build release version
cargo build --release

# Run tests
cargo test

# Run clippy lints
cargo clippy -- -D warnings

# Format code
cargo fmt

# Run with debug logging
RUST_LOG=debug cargo run -- song.mp3
```

### Running Tests

```sh
# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

## Project Structure

```
kora/
├── src/
│   ├── main.rs              # Entry point, CLI parsing
│   ├── core/                # Domain models, traits, config (no audio deps)
│   │   ├── config.rs        # KoraConfig, config.toml loading
│   │   ├── session.rs       # Session persistence (save/restore state)
│   │   ├── track.rs         # Track, TrackMetadata, TrackSource types
│   │   └── types.rs         # Volume, shared types
│   ├── playback/            # Decode, DSP, state machine
│   │   ├── decoder.rs       # symphonia file decode pipeline
│   │   ├── stream_decoder.rs # HTTP stream decode (non-seekable sources)
│   │   ├── dsp.rs           # Biquad IIR filters (Transposed Direct Form II)
│   │   ├── eq.rs            # 10-band graphic equalizer with presets
│   │   ├── engine.rs        # Sequential playback (non-TUI mode)
│   │   └── player.rs        # Player controller (TUI ↔ audio bridge)
│   ├── backend/             # Audio output adapters
│   │   └── cpal_backend.rs  # CPAL output via rtrb ring buffer
│   ├── providers/           # Audio source implementations
│   │   ├── local.rs         # Local file resolver
│   │   ├── radio.rs         # Radio Browser API client
│   │   └── stations.rs      # Custom stations from stations.toml
│   └── tui/                 # Terminal UI
│       ├── app.rs           # TUI event loop, rendering, input
│       └── theme.rs         # Nord color theme
├── docs/
│   └── adr/                 # Architecture Decision Records
├── config.toml.example      # Example user configuration
├── stations.toml.example    # Example custom radio stations
├── ROADMAP.md               # Full product roadmap
└── CHANGELOG.md             # Keep a Changelog format
```

### Key Architecture Decisions

See [docs/adr/](docs/adr/) for Architecture Decision Records documenting key technical decisions.

### Layer Rules

kora follows a strict layered architecture. Please respect these boundaries:

- **core/** — No audio crate dependencies. No TUI dependencies.
- **playback/** — May depend on symphonia, rtrb. No TUI dependencies.
- **backend/** — Platform-specific audio output only. No TUI.
- **providers/** — May depend on core/. No playback internals.
- **tui/** — May depend on everything above. Not depended on by anything.

### Real-Time Audio Rules

Code running on the CPAL audio callback thread must follow these rules (see ADR-001):

- No heap allocation
- No blocking I/O
- No mutex waits — lock-free only
- No logging — atomic flag checks only
- Underrun: output silence, set atomic flag

## Coding Standards

- Follow standard Rust conventions and idioms
- Use `rustfmt` for formatting (default settings)
- Address all `clippy` warnings
- Use `thiserror` for error types, `anyhow` for application errors
- Wrap errors with context (`anyhow::Context`)
- Add doc comments for public APIs
- Use table-driven tests where possible

## Commit Messages

Use [conventional commits](https://www.conventionalcommits.org/):

- `feat:` — New feature
- `fix:` — Bug fix
- `docs:` — Documentation only
- `test:` — Adding or updating tests
- `refactor:` — Code change that neither fixes a bug nor adds a feature
- `chore:` — Build process, CI, dependencies

Examples:

```
feat: add internet radio playback via Radio Browser API
fix: correct audio dropout when seeking near end of track
docs: add ADR-006 for podcast state management
test: add golden file tests for MP3 decoder
```

## License

By contributing to kora, you agree that your contributions will be licensed under the same dual license as the project: MIT OR Apache-2.0.
