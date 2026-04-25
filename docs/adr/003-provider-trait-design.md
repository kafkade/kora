# ADR-003: Provider Trait Design

## Status

Accepted

## Context

kora supports multiple audio sources (local files, internet radio, podcasts, and future
streaming services). We need a common abstraction so the playback engine doesn't need to
know the specifics of each source.

## Decision

Use **capability-based trait composition** with an evolvable internal API:

```rust
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
}

pub trait Browsable: Provider {
    async fn browse(&self, path: &str) -> Result<Vec<BrowseItem>>;
}

pub trait Searchable: Provider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>>;
}

pub trait Streamable: Provider {
    async fn resolve(&self, track: &Track) -> Result<AudioSource>;
}
```

**Key decisions**:
- Each provider implements only the traits it supports (local files: Browsable +
  Streamable; radio: Browsable + Searchable + Streamable)
- The API is **internal and evolvable** — do NOT stabilize as a public plugin API until
  at least 3 providers are implemented and common patterns emerge
- All async methods support cancellation via `tokio::select!` or `CancellationToken`
- Errors use a `ProviderError` enum with variants: `NetworkError`, `AuthError`,
  `NotFound`, `RateLimited`, `Timeout`, `Unavailable`

## Alternatives Considered

- **Single monolithic Provider trait**: Rejected — forces providers to stub out methods
  they don't support (e.g., local files implementing `authenticate()`)
- **Stable public API from day one**: Rejected — premature. We don't know the right
  abstraction until we've built local, radio, and podcast providers.

## Consequences

- Adding a new provider only requires implementing the relevant subset of traits
- The playback engine queries capabilities at runtime to decide what UI to show
- The trait API will likely change as we learn from implementing providers — this is
  expected and acceptable until 1.0
