# ADR-004: Session Persistence via TOML

## Status

Accepted

## Context

kora needs to persist playback state (current track, position, queue, volume, EQ preset,
theme, shuffle/repeat) so users can resume where they left off after quitting.

Options: TOML file, JSON file, SQLite database, binary format.

## Decision

Use a **TOML file** (`~/.config/kora/session.toml`) for session persistence.

Example:

```toml
[session]
track = "~/Music/album/01-track.flac"
position_ms = 142000
volume_db = -3.0
eq_preset = "Flat"
theme = "Nord"
shuffle = false
repeat = "off"
visualizer = "bars"

[[queue]]
path = "~/Music/album/01-track.flac"

[[queue]]
path = "~/Music/album/02-track.flac"
```

**Behavior**:
- Auto-save on quit and every 30 seconds during playback
- On startup: restore state, resume paused (user presses play to continue)
- Crash recovery: the periodic auto-save provides a recent snapshot

## Alternatives Considered

- **SQLite**: Rejected for session state — overkill for a single document of settings.
  SQLite is appropriate for the library index (Phase 6) but not for config/session.
- **JSON**: Rejected — less human-readable than TOML, no comments support.
- **Binary (bincode/MessagePack)**: Rejected — not human-editable, makes debugging harder.

## Consequences

- Users can manually edit session.toml to fix issues or set state
- Session files are portable and syncable via cloud drives (iCloud, Google Drive, etc.)
- TOML serialization/deserialization via `serde` + `toml` crate is trivial
- File paths in session use `~/` prefix for cross-device portability
- When library indexing arrives (Phase 6), SQLite handles that separately
