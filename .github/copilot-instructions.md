# Copilot Instructions for kora

## Project Overview

kora is a fast, multi-source terminal audio player built in Rust. It plays local audio files, internet radio stations, and podcasts from the terminal — with an equalizer, visualizer, themes, and more planned. The project is in early development (Phase 0–1).

## Non-Negotiable Constraints

Every code contribution and architecture decision must uphold these:

1. **Performance first** — Audio playback must never stutter. The audio callback thread is real-time: no allocation, no locks, no I/O, no logging. Use lock-free ring buffers between threads.
2. **Layered architecture** — Domain core (models, traits, config) has no audio dependencies. Playback core (decode, DSP) has no TUI dependencies. Backend adapters are platform-specific. Frontends are separate. Never couple layers.
3. **Offline-first** — The player must work fully offline with local files. Network features degrade gracefully.
4. **User sovereignty** — No telemetry, no tracking, no analytics. Config is human-readable TOML. Playlists are portable. No mandatory account or internet for local playback.
5. **Daily use first** — Prefer working vertical slices over speculative architecture. Ship usable increments.

## Architecture

Single Rust crate with clear module boundaries (split into workspace when needed):

```
src/
├── core/       — Domain models, provider traits, config, session (no audio deps)
├── playback/   — Decode (symphonia), DSP (EQ, volume), playback state machine
├── backend/    — Audio output adapters (CPAL for native)
├── providers/  — Audio source implementations (local, radio, podcast)
├── tui/        — Terminal UI (ratatui + crossterm)
└── ipc/        — Remote control protocol (Unix socket / named pipe)
```

### Audio Pipeline

```
Source → [Decode Thread] → rtrb Ring Buffer → [Audio Callback] → DSP → CPAL Output
```

- Decode thread: can allocate, do I/O, block on network
- Audio callback: REAL-TIME — no alloc, no locks, no I/O, no logging
- Communication: lock-free SPSC ring buffer (rtrb)

### Real-Time Audio Rules (Non-Negotiable)

These rules apply to any code that runs on the CPAL audio callback thread:

- No heap allocation (`Vec::push`, `String::new`, `Box::new`, `format!`)
- No blocking I/O (file, network, stdout)
- No mutex waits — use `try_lock()` or lock-free only
- No channel blocking — use `try_recv()` / `try_send()`
- No logging — atomic flag checks only
- Underrun: output silence, set atomic flag

## Tech Stack

| Component | Crate | Notes |
|-----------|-------|-------|
| Decoding | `symphonia` | Pure Rust, MP3/FLAC/OGG/WAV/Opus |
| Audio output | `cpal` | Cross-platform (ALSA, CoreAudio, WASAPI) |
| Ring buffer | `rtrb` | Lock-free SPSC for real-time audio |
| TUI | `ratatui` + `crossterm` | Terminal rendering |
| CLI | `clap` v4 (derive) | Argument parsing |
| Config | `serde` + `toml` | Human-readable config |
| Async | `tokio` | Network I/O, IPC |
| HTTP | `reqwest` | Radio streams, podcast feeds |
| Tags | `lofty` | ID3, Vorbis, MP4 metadata |
| Logging | `tracing` | Structured, async-aware |
| Errors | `thiserror` + `anyhow` | Library vs binary error handling |

## Conventions

- **License**: Dual MIT + Apache 2.0 — all contributions must be compatible
- **Error handling**: Wrap with context (`anyhow::Context`). Surface user-facing messages from `main.rs` only.
- **Module naming**: Lowercase, matches directory name
- **Build tags**: Platform-specific files use `_linux.rs` / `_macos.rs` / `_windows.rs` suffixes if needed
- **Testing**: Table-driven tests. Mock audio backend for CI. Property-based tests for DSP.
- **PR title format**: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`
- **Config paths**: XDG on Linux, `~/Library/Application Support/kora/` on macOS, `%APPDATA%/kora/` on Windows

## Provider Contract

Providers implement capability-based traits (evolving — do not stabilize until 3+ providers exist):

- `Browsable` — list content (directories, station lists, feed episodes)
- `Searchable` — search by query
- `Streamable` — resolve a track to a playable audio stream

## Non-Goals & Red Lines

- No DRM circumvention
- No stream recording / "save to disk" from streaming sources
- No automatic telemetry
- No plaintext credential storage without explicit user opt-in
- No plugin system before 1.0
- No mobile app before CLI/TUI has proven architecture

## Reference Documents

The full product roadmap with architecture decisions, data model, competitive analysis, and phased milestones is in `ROADMAP.md`.
