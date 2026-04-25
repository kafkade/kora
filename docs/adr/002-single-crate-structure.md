# ADR-002: Single Crate with Module Boundaries

## Status

Accepted

## Context

kora's architecture defines four layers: domain core, playback core, audio backend
adapters, and frontend adapters. We need to decide whether to implement these as
separate Cargo workspace crates from day one or as modules within a single crate.

## Decision

Start as a **single crate** with clear module boundaries:

```
src/
├── core/       — Domain models, traits, config (no audio deps)
├── playback/   — Decode, DSP, state machine (symphonia, rtrb)
├── backend/    — Audio output adapters (CPAL)
├── providers/  — Audio source implementations
├── tui/        — Terminal UI (ratatui)
└── ipc/        — Remote control protocol
```

**Split into a Cargo workspace when**:
- A second frontend needs the core (Phase 7+ web/mobile)
- Compile times exceed 60 seconds incremental
- A module's dependencies block cross-compilation (e.g., CPAL blocking WASM)

## Alternatives Considered

- **Multi-crate workspace from day one**: Rejected — premature separation adds friction
  for a solo developer (more Cargo.toml files, cross-crate dependency management,
  slower iteration). The module boundaries preserve the option to split later.

## Consequences

- Faster iteration during early development
- `cargo build` builds everything in one pass
- Module boundaries are enforced by convention, not by the compiler — requires
  discipline (e.g., `tui/` must not import from `playback/` internals directly)
- When splitting occurs, it should be mechanical: move `src/core/` to `crates/core/`,
  update imports, add workspace Cargo.toml
