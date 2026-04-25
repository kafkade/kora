# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

### Fixed

- Audio playback crash caused by unsafe ring buffer Consumer handling in CPAL callback
