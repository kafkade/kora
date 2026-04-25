# 🎵 Product Roadmap — kora

> A comprehensive roadmap for **kora**, a Rust-based cross-platform audio player beginning as a CLI/TUI application. This document covers architecture, features, phasing, legal considerations, and execution strategy for a solo developer working part-time.

---

## SECTION 0: ASSUMPTIONS TABLE & CLARIFYING QUESTIONS

Before the roadmap begins, the following assumptions anchor every decision. Each assumption is stated, defended, and risk-assessed.

| # | Question | Default | Reasoning | Risk if Wrong |
|---|----------|---------|-----------|---------------|
| 1 | Team size | Solo developer, part-time, with occasional community PRs | Prompt states intermediate Rust developer, part-time. Roadmap must be ruthlessly scoped. | If a team materializes, phases compress but architecture stays valid. |
| 2 | Target timeline | Open-ended, milestone-driven (ship when ready) | Solo part-time work is unpredictable. Fixed dates create pressure to cut corners on audio correctness. | Sponsors or users may want predictability — publish phase goals, not dates. |
| 3 | Budget constraints | Zero budget. No paid APIs, no licensed codecs, no paid infra. Donations welcome. | Open-source, user-sovereign philosophy. Paid dependencies create friction for contributors and users. | Some codecs (AAC) may require ffmpeg as optional runtime dep — acceptable trade-off. |
| 4 | MVP focus | CLI/TUI exclusively. No web interface until post-1.0. | Principle 6 (Daily Use First). A web UI doubles the frontend surface area before the core is proven. | If web demand is high, the layered architecture allows adding it later without rework. |
| 5 | Initial distribution | `cargo install` + GitHub Releases (pre-built binaries via cargo-dist). Add Homebrew and AUR post-MVP. | `cargo install` is free, reaches the primary audience (Rust/terminal users), and requires zero infra. Pre-built binaries via cargo-dist cover non-Rust users. | Miss Linux distro package managers initially — acceptable; community can contribute packaging. |
| 6 | Primary user persona | Terminal Power User | This persona will adopt first, give feedback, and contribute. Other personas are served in later phases. | If the wedge persona is wrong, the project still works as a personal tool. |
| 7 | Primary wedge & switching trigger | Terminal Power User switching from cmus/mpv because they want streaming sources + modern TUI + Rust reliability in one tool. | cmus is minimal (no streaming), mpv is not music-focused, ncmpcpp requires MPD. A unified player fills the gap. | If terminal users don't switch, the project remains useful as the developer's personal player. |
| 8 | Marketing positioning | "No tracking. No accounts. Your music, your way." — lead with user sovereignty and open-source. | Privacy-conscious users are the natural audience for a terminal audio player. Differentiates from Spotify/Apple Music. | May limit mainstream appeal — acceptable; mainstream is not the initial target. |
| 9 | Top 3 acquisition channels | 1) GitHub trending / r/rust, 2) Hacker News, 3) Reddit r/commandline + r/unixporn | Terminal tools go viral on HN and Reddit. Rust community is supportive of new projects. | Growth may be slow — acceptable for a solo project. Quality > marketing. |
| 10 | MVP audio formats | Tier 1 (MVP): MP3, FLAC, OGG Vorbis, Opus, WAV/PCM — all via symphonia (pure Rust). | symphonia decodes all five natively with no C dependencies. Covers 95%+ of local music libraries. [Validated] | If a user's library is mostly AAC/M4A, they must wait for post-MVP ffmpeg integration. |
| 11 | Hi-res / audiophile | CD quality (44.1kHz/16-bit) and below for MVP. Support up to 192kHz/24-bit if symphonia handles it, but no bit-perfect/exclusive mode claims until measured. | Audiophile features are a rabbit hole. Most users won't notice. Defer exclusive mode to post-1.0. | Lose audiophile niche — acceptable; they have foobar2000 and dedicated hardware players. |
| 12 | Audio output backend | CPAL as the sole audio output abstraction for all native platforms (Linux ALSA/PipeWire, macOS CoreAudio, Windows WASAPI). | CPAL is the de facto Rust audio output crate, actively maintained, supports all three desktop OSes. Avoids maintaining three platform-specific backends. [Validated] | CPAL may have edge cases (device switching, suspend/resume) — mitigate with PoC gate (Section 9.1). |
| 13 | Gapless playback | Deferred to Phase 3 (Audio Polish). Not MVP. | Gapless requires pre-decoding and careful buffer management. MVP should focus on reliable single-track playback first. | Album listeners notice gaps — acceptable trade-off for faster MVP. |
| 14 | ReplayGain | Deferred to Phase 3. | Requires metadata parsing + gain application in DSP chain. Not essential for daily use. | Volume jumps between tracks — minor annoyance, not a blocker. |
| 15 | Radio directory | Radio Browser API as primary. Custom TOML stations as secondary/fallback. | Radio Browser is free, open, community-run, 30k+ stations, REST API, multiple mirrors. [Validation Required] — verify rate limits and uptime. | If Radio Browser goes down, custom TOML stations still work. User is never locked out. |
| 16 | Podcast scope (initial) | Simple "paste RSS URL and play" for Phase 2. Full podcast client in Phase 4. | A full podcast client (OPML, episode tracking, downloads) is a product in itself. Phase 2 should prove network audio works; Phase 4 builds the full experience. | Podcast users may find Phase 2 too limited — acceptable; Phase 4 follows. |
| 17 | Spotify integration | Research only. Do not commit engineering time until ToS/API feasibility is validated. | Spotify's Web API does NOT provide raw audio streams for third-party playback. The Connect SDK has restrictive terms. [Validation Required] — Spotify Developer ToS for open-source players. | If Spotify is infeasible, the project still serves local + radio + podcast + open server users. |
| 18 | YouTube integration | Research only. yt-dlp as optional runtime dependency if pursued. High ToS risk. | YouTube ToS explicitly prohibit downloading/ripping. yt-dlp operates in a legal grey area. [Validation Required] — legal counsel recommended before shipping. | If YouTube is dropped, the project still has strong value from other sources. |
| 19 | WASM/Web audio | Research phase (Phase 9). Web Audio API + AudioWorklet is technically viable but has latency constraints and CORS limitations for streaming URLs. [Validation Required] | WASM audio is an emerging area. Prototype must validate decode performance and streaming feasibility before commitment. | If web target is impractical, CLI + native apps still serve all core personas. |
| 20 | Apple platform FFI | UniFFI as the recommended Rust → Swift bridging tool. Prototype required before commitment. | UniFFI generates Swift bindings automatically from Rust, maintained by Mozilla. [Validation Required] — verify async callback delivery and iOS static library packaging. | If UniFFI fails, manual cbindgen is the fallback (more maintenance, but proven). |
| 21 | Desktop app framework | Tauri v2 when/if a desktop GUI is built. | Tauri is Rust-native, lighter than Electron, has native system tray and updater. | Tauri's webview rendering may have quirks — acceptable risk for post-1.0 feature. |

---

## SECTION 1: USER PERSONAS

### 1. Terminal Power User — "Alex"
- **Archetype**: Software engineer, lives in tmux + Neovim, keyboard-only workflow
- **Listening**: Indie rock, electronic, lo-fi. Local FLAC library (~3k files) + internet radio for background. 2–6 hour sessions.
- **Tech comfort**: Expert. Writes shell scripts, customizes everything.
- **Platforms**: Linux (primary), macOS (secondary). Terminal exclusively.
- **Pain point**: cmus has no streaming; mpv isn't music-focused; ncmpcpp requires MPD setup. Wants one tool for local + radio + podcasts.
- **Accessibility**: None specific. Values fast keyboard navigation.
- **Roadmap phases**: **Phase 0–2** (core target from day one)

### 2. Podcast Commuter — "Sam"
- **Archetype**: Knowledge worker, listens to 8+ podcasts weekly during commute and chores
- **Listening**: Tech podcasts, news, interview shows. 30–90 min sessions, daily.
- **Tech comfort**: Moderate. Uses terminal occasionally, prefers polished UX.
- **Platforms**: Laptop (CLI), wants iOS eventually.
- **Pain point**: Apple Podcasts sync is unreliable. Wants open-source, cross-device podcast management with speed control and resume.
- **Accessibility**: Needs playback speed control (1.5x–2x).
- **Roadmap phases**: **Phase 4** (full podcast client), **Phase 10** (iOS)

### 3. Music Collector — "Jordan"
- **Archetype**: Audiophile-adjacent, large organized library, cares about tags and lossless
- **Listening**: Jazz, classical, progressive rock. Full albums, gapless critical. 15k+ files, FLAC/ALAC.
- **Tech comfort**: High. Comfortable with CLI but wants library browsing.
- **Platforms**: Linux/macOS desktop.
- **Pain point**: No terminal player handles large libraries well with proper gapless, EQ, and tag browsing.
- **Accessibility**: None specific.
- **Roadmap phases**: **Phase 3** (gapless, EQ), **Phase 6** (library management)

### 4. Casual Listener — "Riley"
- **Archetype**: Developer who wants background music while coding. Minimal configuration.
- **Listening**: Lo-fi streams, jazz radio, ambient playlists. All-day background sessions.
- **Tech comfort**: Moderate. Uses terminal but doesn't want to configure much.
- **Platforms**: macOS/Linux terminal.
- **Pain point**: Wants a "just works" terminal radio player with nice visuals. Existing tools require too much setup.
- **Accessibility**: None specific. Values beautiful defaults.
- **Roadmap phases**: **Phase 2** (radio), **Phase 3** (visualizer, themes)

### 5. Multi-Device User — "Morgan"
- **Archetype**: Uses laptop at desk (CLI), phone on the go, wants continuity
- **Listening**: Mix of podcasts and music. Switches devices 3–4 times daily.
- **Tech comfort**: High.
- **Platforms**: macOS CLI → iOS → possibly Apple Watch.
- **Pain point**: No open-source player syncs state across CLI and mobile.
- **Accessibility**: None specific.
- **Roadmap phases**: **Phase 10** (iOS app, sync server)

### 6. Developer/Contributor — "Casey"
- **Archetype**: Rust developer interested in audio programming, wants to contribute or build plugins
- **Listening**: Varies. Uses the player partly to explore the codebase.
- **Tech comfort**: Expert. Reads source code for fun.
- **Platforms**: Any.
- **Pain point**: Most audio players have opaque C/C++ codebases. Wants clean Rust with good docs and contribution paths.
- **Accessibility**: None specific.
- **Roadmap phases**: **Phase 5** (IPC/API), **Phase 8** (plugin system)

### 7. Radio Enthusiast — "Kai"
- **Archetype**: Discovers and bookmarks internet radio stations across genres and countries
- **Listening**: World music, jazz, classical, electronic radio. Long sessions, station-hopping.
- **Tech comfort**: Moderate to high.
- **Platforms**: Linux/macOS terminal.
- **Pain point**: No good terminal radio browser with favorites, metadata display, and a large station directory.
- **Accessibility**: None specific.
- **Roadmap phases**: **Phase 2** (radio integration), **Phase 3** (favorites)

---

## SECTION 2: CROSS-PLATFORM ARCHITECTURE & ENGINE DESIGN

### 2.1 — Layered Core Architecture

The architecture is four layers with strict dependency rules:

**Layer 1: Domain Core** (`core` crate)
- Compiles to: native (all OS), WASM, FFI-safe
- Contains: Track/Playlist/Queue models, Provider trait definitions, Session state, Configuration model (serde + TOML), Metadata types (not extraction), EQ preset model, Theme model
- Dependencies: serde, toml, uuid — no audio, no TUI, no platform crates
- All types must be `Send + Sync` and FFI-representable where needed
- All APIs synchronous (no async runtime dependency)

**Layer 2: Playback Core** (`playback` crate)
- Compiles to: native (all OS), potentially WASM with limitations
- Contains: Audio decode pipeline (format detection → codec dispatch → sample conversion), DSP chain (EQ biquad filters, volume, ReplayGain gain, crossfade mixing, FFT for visualizer data), Playback state machine (play/pause/stop/seek/next/prev), Metadata extraction (ID3v2, Vorbis comments, MP4 atoms via lofty)
- Dependencies: `core`, symphonia, rubato (resampling), lofty (metadata), realfft (FFT)
- Does NOT depend on: cpal, any platform audio output, TUI, network, async runtime
- Internal sample format: f32 throughout the DSP chain (sufficient precision, hardware-friendly)
- Exposes a pull-based API: the audio backend calls `playback.fill_buffer(&mut [f32])` to get samples
- Thread model: decode runs on a dedicated worker thread, pushes samples into a lock-free ring buffer. DSP processing happens either on the decode thread (pre-buffer) or on the audio callback thread (post-buffer). **Recommendation**: DSP on the decode thread, so the audio callback only reads from the ring buffer. This keeps the callback path trivially simple.

**Layer 3: Audio Backend Adapters** (one crate per platform)
- `backend-cpal`: CPAL for Linux/macOS/Windows. Implements `AudioOutput` trait. Runs the audio callback that pulls from the ring buffer.
- `backend-web` (future): Web Audio API / AudioWorklet for WASM target.
- `backend-avfoundation` (future): AVAudioEngine for iOS/macOS native apps if CPAL is insufficient.
- Common trait:
```rust
trait AudioOutput: Send {
    fn start(&mut self, config: AudioConfig) -> Result<()>;
    fn stop(&mut self);
    fn set_volume(&mut self, volume_db: f32);
    fn sample_rate(&self) -> u32;
    fn device_name(&self) -> &str;
}
```

**Layer 4: Frontend Adapters** (each a separate binary/crate)
- `cli` crate: TUI (ratatui + crossterm) — the main binary for MVP
- `daemon` (future): headless IPC server, no TUI
- `web-ui` (future): WASM + web framework
- `app-shell` (future): SwiftUI consuming FFI bindings

**Boundary rules**:
- Domain core has zero audio dependencies. It can be used by any frontend for playlist management, config, state.
- Playback core has zero platform dependencies. It can be tested with a mock output.
- Audio backends are the ONLY place platform-specific audio code lives.
- Frontend adapters are the ONLY place UI code lives.
- No layer may depend on a layer above it.

### 2.2 — Crate/Module Architecture

**Recommendation: Start as a single crate with clear module boundaries. Split into workspace crates at Phase 2.**

**Reasoning**: Premature crate separation in Rust means longer compile times (each crate is a separate compilation unit), more boilerplate (pub exports, feature flags), and friction when APIs are still evolving. A single crate with `mod core`, `mod playback`, `mod backend`, `mod tui` enforces the same boundaries via module visibility (`pub(crate)`) while allowing rapid iteration.

**Split trigger**: When the first non-TUI consumer appears (IPC daemon, Phase 5), extract `core` and `playback` into workspace crates. The module boundaries established in Phase 0–1 become crate boundaries.

**Target workspace layout (Phase 2+)**:
```
project/
├── crates/
│   ├── core/            # Domain core: models, traits, config, state
│   ├── playback/        # Decode, DSP, state machine
│   ├── backend-cpal/    # CPAL audio output
│   ├── providers/
│   │   ├── local/       # Local file provider
│   │   ├── radio/       # Internet radio provider
│   │   └── podcast/     # Podcast RSS provider
│   ├── cli/             # TUI application binary
│   └── ipc/             # IPC protocol (Phase 5)
├── config/              # Example configs, default themes
├── tests/               # Integration tests, golden files
└── docs/                # User and developer documentation
```

**Rejected alternatives**:
- Multi-crate from day one: too much overhead for a solo developer in early phases
- Single crate forever: doesn't support the cross-platform vision; multiple binaries need shared libraries

### 2.3 — Audio Pipeline Architecture

```
Source → Decoder → Sample Rate Converter → DSP Chain → Ring Buffer → Audio Callback → Output Device
           │                                    │
     [Decode Thread]                    [Decode Thread]       [Audio Callback Thread]
                                             │
                                    ┌────────┴────────┐
                                    │ Volume (f32 mul) │
                                    │ EQ (biquad bank) │
                                    │ ReplayGain       │
                                    │ Crossfade         │
                                    │ FFT → viz buffer  │
                                    └─────────────────┘
```

**Stage details**:

| Stage | Thread | Crate | Sample Format | Buffering | Notes |
|-------|--------|-------|---------------|-----------|-------|
| Source read (file/network) | I/O thread (async for network, sync for files) | providers | bytes | Tokio for HTTP, std::fs for local | Network uses reqwest with streaming body |
| Decode | Decode worker thread | playback (symphonia) | i16/i32/f32 → convert to f32 | symphonia's internal buffering | One decoder instance per track |
| Sample rate conversion | Decode worker thread | playback (rubato) | f32 | Rubato's internal buffers | Only when source rate ≠ output rate |
| DSP chain | Decode worker thread | playback | f32 | In-place processing on decode buffer | EQ, volume, gain — all applied before ring buffer |
| FFT for visualizer | Decode worker thread | playback (realfft) | f32 | Writes to shared atomic/lock-free viz buffer | Read-only by TUI thread |
| Ring buffer | Shared (lock-free) | playback | f32 | Fixed-size ring buffer (~200ms) | Producer: decode thread. Consumer: audio callback. |
| Audio callback | Audio callback thread (OS-managed) | backend-cpal | f32 → device format | CPAL's internal buffer | Copies from ring buffer. NOTHING else. |

**Seeking**: Decode thread flushes the ring buffer, seeks in the decoder, refills. Brief silence (~50ms) is acceptable for MVP. Gapless improvement in Phase 3.

**Gapless playback (Phase 3)**: Pre-decode next track's first buffer while current track is in its final ~2 seconds. Swap decoders seamlessly. No crossfade for gapless (that's a separate feature).

### 2.3.1 — Real-Time Audio Safety Rules

**Hard rules for the audio callback thread** (the CPAL callback):

1. **No heap allocation** — no `Vec::push`, `String::new`, `Box::new`, `format!()`
2. **No blocking I/O** — no file reads, no network, no `println!`
3. **No mutex locks** — use `try_lock` with silent fallback, or lock-free structures only
4. **No channel receives that block** — use `try_recv` only
5. **No logging** — at most an atomic flag check for error signaling
6. **No metadata parsing** — all metadata is resolved before samples reach the ring buffer
7. **Bounded ring buffer only** — fixed capacity, pre-allocated at startup

**Backpressure strategy**: If the ring buffer is empty (decode can't keep up), output silence and set an atomic `underrun` flag. The UI thread reads this flag and displays a warning. The decode thread detects the underrun and increases its priority/reduces DSP load if possible.

**Overrun handling**: If the ring buffer is full (decode is faster than playback), the decode thread blocks on the ring buffer's `push` (this is fine — the decode thread is allowed to block). This naturally rate-limits decoding to playback speed.

**Underrun recovery**: After an underrun, the decode thread fills the buffer to 50% capacity before the callback resumes reading, preventing oscillation.

### 2.4 — Platform Compilation Targets

| Platform | Target | Audio Output | UI | FFI | Phase |
|----------|--------|-------------|-----|-----|-------|
| Linux x86_64/aarch64 | native | CPAL (ALSA/PipeWire) | ratatui | N/A | MVP |
| macOS x86_64/aarch64 | native | CPAL (CoreAudio) | ratatui | N/A | MVP |
| Windows x86_64 | native | CPAL (WASAPI) | ratatui (crossterm) | N/A | MVP |
| Web/WASM | wasm32-unknown-unknown | Web Audio API | Custom (WASM) | wasm-bindgen | Research (Phase 9) |
| iOS | aarch64-apple-ios | AVAudioEngine | SwiftUI | UniFFI | Moonshot (Phase 10) |
| macOS App | native | CoreAudio/CPAL | SwiftUI/Tauri | UniFFI | Research (Phase 9) |
| watchOS | arm64_32-apple-watchos | WKAudioSession | WatchKit | UniFFI | Moonshot (Phase 10) |

### 2.5 — State Management & Persistence

**Recommendation: TOML for configuration and session state. SQLite for library index (Phase 6+).**

**Reasoning**: TOML is human-readable, version-control-friendly, and aligns with Principle 5 (User Sovereignty). For MVP through Phase 5, all state fits comfortably in TOML files. SQLite is introduced only when the library index needs queryable, structured storage (Phase 6).

**State files**:
- `config.toml` — user preferences, provider settings, keybindings, paths
- `session.toml` — current playback state (track, position, queue, volume, EQ, theme)
- `playlists/` — directory of `.toml` playlist files
- `stations.toml` — custom radio station bookmarks/favorites
- `podcasts.toml` — subscribed feeds and episode state (upgrades to SQLite in Phase 6)

**Platform paths**:
- Linux: `$XDG_CONFIG_HOME/kora/` (default `~/.config/kora/`)
- macOS: `~/Library/Application Support/kora/`
- Windows: `%APPDATA%\kora\`

**Resume-on-restart**: On quit (or crash), `session.toml` is written with current state. On startup, if `session.toml` exists and `--no-resume` is not passed, restore playback state. Auto-save every 30 seconds during playback.

### 2.6 — Sync Strategy

**Phase 1 (MVP through 1.0)**: All state is local. Config and playlists are portable TOML files. Users sync manually via cloud drives (iCloud Drive, Syncthing, Google Drive).

**Phase 2 (Post-1.0) Recommendation: File-based sync via cloud drives with conflict detection.**

**Reasoning**: A sync server is a second product. CRDTs are over-engineering for this use case. File-based sync with last-modified timestamps and simple conflict detection (prompt user on conflict) covers 90% of multi-device use cases. The migration path: TOML files → TOML files with embedded version vectors → optional sync server (Phase 10).

**Sync classification per entity**:

| Entity | Sync? | Conflict Strategy | Device-Local? | Contains Secrets? | Has Local Paths? |
|--------|-------|-------------------|---------------|-------------------|------------------|
| UserConfig | Yes | Last-write-wins | Partially (device-specific audio settings) | No | Yes (library paths) |
| Playlists | Yes | Merge (track list union) | No | No | Yes (file paths — use provider IDs instead) |
| Favorites | Yes | Merge (union) | No | No | No (provider IDs) |
| Session state | No | N/A | Yes | No | Yes |
| Podcast subscriptions | Yes | Merge (union) | No | No | No |
| Episode play state | Yes | Last-write-wins (latest position) | No | No | No |
| EQ presets | Yes | Last-write-wins | No | No | No |
| Themes | Yes | Last-write-wins | No | No | No |
| Provider credentials | No (per-device) | N/A | Yes | Yes | No |
| Listening history | Yes (append-only) | Merge (union) | No | No | No |
| Library index | No (rebuilt per device) | N/A | Yes | No | Yes |

---

## SECTION 3: PROVIDER SYSTEM ARCHITECTURE

### 3.1 — Provider Trait Design

**Recommendation: Capability-based trait design with async methods. Internal and evolvable — NOT a public stable ABI.**

The provider interface starts as a set of capability traits. A provider implements only the traits matching its capabilities:

```rust
// All provider methods are async (network I/O) and return Result types
#[async_trait]
trait ProviderInfo {
    fn id(&self) -> &str;            // "local", "radio", "podcast"
    fn display_name(&self) -> &str;
    fn capabilities(&self) -> Capabilities; // bitflags
}

#[async_trait]
trait Browsable: ProviderInfo {
    async fn browse(&self, path: &BrowsePath, pagination: Pagination) 
        -> Result<BrowsePage>;
}

#[async_trait]
trait Searchable: ProviderInfo {
    async fn search(&self, query: &str, pagination: Pagination) 
        -> Result<SearchResults>;
}

#[async_trait]
trait Resolvable: ProviderInfo {
    async fn resolve(&self, track_id: &TrackId) -> Result<AudioStreamHandle>;
}

#[async_trait]
trait MetadataProvider: ProviderInfo {
    async fn metadata(&self, track_id: &TrackId) -> Result<TrackMetadata>;
}
```

**Design decisions**:
- Async throughout (even local files — for consistency and future network providers)
- Pagination on browse/search (providers have different page sizes)
- `AudioStreamHandle` is an enum: `FileHandle(PathBuf)` for local, `HttpStream(Url)` for network
- Error types are provider-specific but implement a common error trait
- Cancellation via `tokio::CancellationToken` passed to long-running operations
- Timeout configurable per-provider in config

**Why not one big trait**: Local files don't need auth. Radio doesn't need search (the directory does). Podcasts don't support browse-by-genre the same way. Small traits compose better.

**Stabilization rule**: Do NOT freeze the provider API until local, radio, and podcast providers are all implemented and the shared patterns are empirically validated. Target API freeze at 1.0.

### 3.2 — Provider Implementation Matrix

| Provider | Auth | Browse | Search | Stream | Metadata | Offline | Phase |
|----------|------|--------|--------|--------|----------|---------|-------|
| Local Files | None | Directory tree | Filename + tag search | File read | ID3/Vorbis/MP4 via lofty | Full | MVP |
| Internet Radio | None | Radio Browser API | Station search | HTTP/ICY stream | ICY metadata | No | Phase 2 |
| Podcasts (RSS) | None | Feed list | Episode search | HTTP download/stream | RSS metadata | Download cache | Phase 2 (basic), Phase 4 (full) |
| Navidrome/Subsonic | Subsonic auth | Albums/Artists | Full catalog | Subsonic stream | Subsonic API | Cache | Phase 7 |
| Jellyfin | API key | Library | Full catalog | Jellyfin stream | Jellyfin API | No | Phase 7 |
| Plex | OAuth | Library | Full catalog | Plex stream | Plex API | No | Phase 7 |
| Spotify | OAuth | Library/Playlists | Full catalog | ⛔ No raw audio via Web API | Spotify API | No | Research |
| YouTube | yt-dlp (runtime) | N/A | yt-dlp search | yt-dlp stream | yt-dlp metadata | No | Research |

### 3.3 — Provider-Specific Concerns

**Local Files Provider**:
- Data flow: `PathBuf → std::fs::File → symphonia::MediaSource → decode pipeline`
- Buffering: symphonia handles internal read buffering. Ring buffer between decode and output.
- Error handling: File not found → skip and log. Corrupt file → skip with user notification. Permission denied → clear error message.
- Metadata: Read on first play or scan, cached in memory. Written to library index (Phase 6).
- Offline: Always works.

**Internet Radio Provider**:
- Data flow: `URL → reqwest streaming GET → ICY-aware byte parser → symphonia decode`
- Buffering: 5-second pre-buffer before playback starts. Adaptive rebuffering on underrun.
- Error handling: Connection drop → retry 3x with exponential backoff → notify user. Invalid stream → clear error. Redirect → follow up to 5 hops.
- Metadata: ICY metadata parsed inline from stream (title, artist if available). Station metadata from Radio Browser API.
- Offline: Not available. Show last-known station list from cache.

**Podcast Provider**:
- Data flow: `RSS URL → feed-rs parse → episode URL → reqwest download/stream → symphonia decode`
- Buffering: For streaming: same as radio. For downloaded episodes: same as local files.
- Error handling: Feed parse error → show error, keep stale data. Download failure → retry with resume. Episode gone (404) → mark unavailable.
- Metadata: From RSS feed XML (title, description, duration, pub date, image URL).
- Offline: Downloaded episodes play offline. Feed list cached locally.

### 3.4 — Radio Provider Deep Dive

| Source | API Type | Station Count | Free? | Rate Limits | Reliability | Recommendation |
|--------|----------|---------------|-------|-------------|-------------|----------------|
| Radio Browser | REST JSON | 30k+ | Yes, open source | Generous (no auth needed) | Community-run, multiple mirrors | ✅ **Primary** |
| radio.garden | Unofficial scraping | Global | [Validation Required] — no public API documented | Unknown | [Validation Required] | ❌ Reject — no stable API, scraping risk |
| Curated TOML | Local file | User-defined | Yes | N/A | Full user control | ✅ **Secondary** (always available) |
| TuneIn | Commercial API | Large | [Validation Required] — likely requires partnership | [Validation Required] | Commercial, reliable | ❌ Reject — likely requires commercial agreement |

**Recommendation**: Radio Browser API as the primary station directory. Custom TOML stations as a persistent secondary source that works offline and supplements the directory. Users add favorites from Radio Browser or manually enter station URLs in their `stations.toml`.

**Coexistence model**: The radio browser shows two sources in the TUI: "Directory" (Radio Browser) and "My Stations" (TOML). Users can browse the directory and star stations, which copies them to My Stations. My Stations is always available offline.

### 3.5 — Podcast Provider Deep Dive

**Phase 2 (Basic)**: Paste RSS URL → parse with `feed-rs` → list episodes → stream playback. No persistence of episode state beyond session.

**Phase 4 (Full Client)**:
- **RSS/Atom parsing**: `feed-rs` crate — handles RSS 2.0, Atom, and JSON Feed. 🟢 [Validated]
- **OPML import/export**: `opml` crate or simple custom XML serializer (OPML is trivial XML). 🟢
- **Episode state tracking**: played/in-progress (with position)/new. Stored in `podcasts.toml` initially, SQLite in Phase 6.
- **Download management**: Background downloads via tokio tasks. Configurable storage limit (e.g., 5GB). Auto-cleanup: delete played episodes older than N days. Resume interrupted downloads (HTTP Range headers).
- **Playback speed**: 0.5x–3x. See Section 4.1 for pitch correction approach.
- **Chapter support**: Parse MP3 chapters (ID3v2 CHAP/CTOC frames) and podcast namespace `<podcast:chapters>`. Display chapter list in TUI. 🟡
- **Apple Podcasts directory**: [Validation Required] — Apple provides an iTunes Search API that can find podcasts. Free, no auth, but undocumented rate limits. Use for discovery only.
- **Pocket Casts**: [Validation Required] — no public API. Likely not viable. Reject.

---

## SECTION 3A: MEDIA RIGHTS, PROVIDER LEGALITY & TRUST BOUNDARIES

### 3A.1 — DRM and Streaming Provider Reality Check

| Provider | Raw Audio Access | Official SDK? | Open Source Allowed? | ToS Risk | Classification |
|----------|-----------------|---------------|---------------------|----------|----------------|
| Spotify | ⛔ Web API returns metadata only, no audio streams. Connect SDK controls Spotify app, doesn't provide raw audio. [Validation Required] | Spotify Connect SDK (limited) | [Validation Required] — Connect SDK ToS restrict open-source redistribution | High | ⛔ Not viable for independent playback |
| YouTube / YT Music | Via yt-dlp only (no official audio SDK). YouTube ToS §5.1 prohibits downloading. [Validation Required] | No | No official support | High | 🔴 High risk — functional but legally grey |
| SoundCloud | Via yt-dlp or unofficial API. [Validation Required] | [Validation Required] | [Validation Required] | Medium | 🔴 High risk |
| Apple Music | DRM-protected streams. MusicKit on Apple platforms only. [Validation Required] | MusicKit (Apple platforms only) | Apple platform apps only | High | ⛔ Not viable for CLI/cross-platform |
| Navidrome/Subsonic | Open Subsonic API, user's own server, raw audio access | Open API | Yes | 🟢 Low | 🟢 Fully viable |
| Plex | Plex API provides audio streams to authenticated clients. [Validation Required] — verify third-party client policy | Official API | [Validation Required] | Medium | 🟡 Likely viable with API compliance |
| Jellyfin | Open API, open source server, raw audio access | Open API | Yes | 🟢 Low | 🟢 Fully viable |
| Internet Radio | Direct HTTP streams, publicly available | N/A | Yes | 🟢 Low | 🟢 Fully viable |
| Podcasts (RSS) | Direct HTTP downloads, RSS is open standard | N/A | Yes | 🟢 Low | 🟢 Fully viable |

**Strategic conclusion**: Focus engineering effort on 🟢 providers (local, radio, podcasts, Subsonic/Navidrome, Jellyfin). Treat Spotify/YouTube as Research — validate before committing any engineering time. Never build features that require DRM circumvention.

### 3A.2 — Copyright and Content Risk

| Feature | Legal Status | Risk | Recommendation |
|---------|-------------|------|----------------|
| Lyrics fetching/display | Most lyrics are copyrighted. LRCLIB provides community-contributed synced lyrics. [Validation Required] — LRCLIB terms | Medium | Opt-in feature. Use LRCLIB. Cache locally. Display attribution. Never bundle lyrics. |
| Album art fetching/caching | Varies. Cover art from local files (embedded) is fine. Fetching from external sources needs attribution. | Low–Medium | Embedded art: always display. External fetch: use MusicBrainz Cover Art Archive (free, CC-licensed). |
| Podcast episode downloads | RSS publishers explicitly provide download URLs. Downloading is intended behavior. | Low | ✅ Safe. Respect `<itunes:block>` if present. |
| Stream recording ("save to disk") | Recording radio streams may violate station ToS or copyright. | High | ❌ Do NOT build stream recording. Explicitly listed as a non-goal. |
| YouTube extraction via yt-dlp | YouTube ToS prohibit it. yt-dlp legality is contested. | High | Research only. If shipped, make it a clearly opt-in plugin, not a default feature. Disclose risk to users. |
| "Now playing" metadata sharing | Song title/artist is factual information. Sharing is fine. | Low | ✅ Safe. |

### 3A.3 — Metadata Exposure & Privacy Matrix

| Data/Metadata | Who Can See It | Risk Level | Mitigation |
|---|---|---|---|
| Local file paths | Only the local player process | None | No exposure |
| Podcast feed URLs requested | Feed host, DNS provider, ISP | Medium | Cache feeds locally; refresh intervals configurable |
| Radio station stream requests | Station server, ISP | Low–Medium | Normal network behavior — documented in privacy statement |
| Lyrics search queries | LRCLIB API | Medium | Opt-in only; cache locally after first fetch |
| Radio Browser API queries | Radio Browser mirrors | Low | Public directory queries; no user identity sent |
| Last.fm/ListenBrainz scrobbles | Scrobble service | High (full listening history) | Opt-in only, explicit consent required in config |
| Discord Rich Presence | Discord, Discord friends | Medium | Opt-in only, off by default |
| yt-dlp requests | YouTube/Google | High | Clearly disclosed; user initiates each request |
| Crash reports/logs | Maintainer | Medium | Never automatic. User must manually copy/share logs |

### 3A.4 — Credential & Token Storage

**Recommendation: OS keychain as primary, with explicit insecure plaintext fallback.**

- **Linux**: `libsecret` (GNOME Keyring) or `kwallet` (KDE) via `keyring` crate [Validated]
- **macOS**: macOS Keychain via `keyring` crate [Validated]
- **Windows**: Windows Credential Manager via `keyring` crate [Validated]
- **Fallback**: If no keychain is available (headless server, minimal Linux), offer plaintext file storage with explicit warning: "Credentials will be stored in plaintext at <path>. Continue? [y/N]"
- **Web**: Browser `localStorage`/`sessionStorage` (future, Phase 9)
- **iOS**: iOS Keychain via the Swift layer (future, Phase 10)

Provider tokens are stored as `provider:<provider_id>:token` keys in the keychain. Never stored in `config.toml`.

### 3A.5 — Non-Goals & Red Lines

Explicitly, this project will NOT:

1. ❌ Circumvent DRM or content protection of any kind
2. ❌ Download/rip from subscription streaming services without explicit API support
3. ❌ Collect telemetry, analytics, or usage data — not even opt-in (no infra to receive it)
4. ❌ Store provider credentials in plaintext config by default
5. ❌ Build a plugin system before core playback is stable and in daily use (post-1.0)
6. ❌ Build mobile apps before the CLI/TUI has proven architecture and real users
7. ❌ Build a sync server before the local state model is stable
8. ❌ Claim bit-perfect or audiophile quality until independently measured
9. ❌ Implement a self-update mechanism before package-manager distribution is established
10. ❌ Require an account, login, or internet connection for local file playback
11. ❌ Build stream recording / "save to disk" for radio or streaming sources
12. ❌ Record or transmit any user behavior without explicit user action

---
## SECTION 4: CORE FEATURE SET

### 4.1 — Audio Playback Engine

**Format Support Tier List**:

| Format | Decoder | Patent/License | Native Rust? | Phase | Complexity | Notes |
|--------|---------|---------------|-------------|-------|------------|-------|
| MP3 | symphonia | Expired patents (2017) | Yes | MVP | 🟢 | Universal format. [Validated] |
| FLAC | symphonia | Free, open standard | Yes | MVP | 🟢 | Lossless standard. [Validated] |
| OGG Vorbis | symphonia | Free, open standard | Yes | MVP | 🟢 | Common in games/Linux. [Validated] |
| Opus | symphonia | Free, BSD-licensed | Yes | MVP | 🟢 | Modern, efficient. [Validated] |
| WAV/PCM | symphonia | No IP | Yes | MVP | 🟢 | Trivial decode. [Validated] |
| AAC (M4A) | symphonia (partial) / ffmpeg | Patent pool (some expired). [Validation Required] | Partial | Phase 3 | 🟡 | symphonia has experimental AAC. Fallback to ffmpeg as optional runtime dep. |
| ALAC | symphonia | Apple open-sourced (Apache 2.0) | Partial | Phase 3 | 🟡 | symphonia support [Validation Required]. Fallback to ffmpeg. |
| WMA | ffmpeg only | Microsoft proprietary | No | Post-1.0 | 🔴 | Requires ffmpeg. Low priority. |
| APE | ffmpeg only | Free but niche | No | Post-1.0 | 🟡 | Monkey's Audio. Niche format. |

**Gapless Playback** 🟡 (Phase 3):
- Pre-decode next track's first ~200ms while current track's final ~2 seconds play
- Seamless buffer swap in the ring buffer (no silence gap, no click)
- Requires knowing the next track in queue before current track ends

**Seeking** 🟢 (MVP):
- Absolute seek: jump to position (seconds or percentage)
- Relative seek: skip ±5s, ±30s (configurable)
- Flush ring buffer on seek, refill from new position
- Brief silence (~30–50ms) during seek is acceptable for MVP

**Playback Speed Control** 🔴 (Phase 3):
- Simple approach (MVP compromise): resample to change speed — this WILL change pitch. Acceptable for podcasts where pitch shift at 1.25x–1.5x is tolerable.
- Proper approach (Phase 3): pitch-preserving time-stretch via WSOLA (Waveform Similarity Overlap-Add) algorithm. **Recommendation**: Use the `rubato` crate for resampling and implement a basic WSOLA in Rust. If too complex, use `soundtouch` C library via FFI as a fallback. [Validation Required] — rubato's suitability for time-stretch vs pure resampling.
- Range: 0.5x–3.0x with pitch correction. No pitch correction below 0.5x or above 3.0x.

**Volume Control** 🟢 (MVP):
- dB-based: -60dB (silence) to +12dB (boost), 0dB = unity gain
- Applied as f32 multiplication in the DSP chain: `sample *= 10.0f32.powf(gain_db / 20.0)`
- Smooth ramping (fade over ~10ms) to avoid clicks on volume change

**Mono Downmix** 🟢 (MVP):
- Toggle in config and via keybinding
- `mono_sample = (left + right) * 0.5`

### 4.2 — 10-Band Graphic Equalizer 🟡 (Phase 3)

**Band frequencies** (ISO standard): 31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz

**Implementation**: Bank of 10 biquad peaking EQ filters in series, applied in the DSP chain on the decode thread. Each filter has configurable gain (-12dB to +12dB). Filter coefficients recalculated on gain change (cheap operation).

**Presets** (shipped defaults):
Flat, Rock, Pop, Jazz, Classical, Electronic, Hip Hop, Acoustic, Bass Boost, Treble Boost, Vocal, Podcast (voice clarity), Custom (user-defined)

**Real-time adjustment**: Changing EQ gain updates filter coefficients. The decode thread picks up new coefficients on the next buffer cycle (~5ms latency). No playback interruption.

**TUI display**: Vertical bars for each band, adjustable with arrow keys. Visual feedback of current gain per band.

**Persistence**: Active preset saved in `session.toml`. Custom presets saved in `config.toml`.

### 4.3 — Spectrum Visualizer 🟡 (Phase 3)

**Modes**:
- **Bars**: 32–64 frequency bins rendered as vertical bars. Classic spectrum analyzer. 🟢
- **Wave**: Waveform oscilloscope — renders raw sample amplitude over time. 🟢
- **Spectrogram**: Scrolling time × frequency heatmap. 🟡 (more complex rendering)
- Future: Matrix rain, particle effects (plugin-extensible)

**FFT pipeline**: The decode thread computes FFT on the most recent ~2048 samples using `realfft` crate (pure Rust, fast). FFT magnitude data is written to a shared lock-free buffer (e.g., `arc-swap` or atomic array). The TUI thread reads this buffer at its own refresh rate.

**Refresh rate**: 20–30 FPS for TUI (configurable). The FFT is computed at decode rate (~44100/2048 ≈ 21 Hz for 2048-sample windows), which naturally matches TUI refresh.

**Performance constraint**: Visualizer computation is on the decode thread (not the audio callback). TUI rendering is on the UI thread. Neither affects the audio callback. If TUI rendering is slow, frames are dropped — audio continues unaffected.

### 4.4 — Synced Lyrics 🟡 (Phase 3)

- **LRC parsing**: Custom parser (LRC format is trivial: `[mm:ss.xx]lyric line`). No crate needed. 🟢
- **Embedded lyrics**: Extract from ID3v2 `USLT`/`SYLT` tags and Vorbis `LYRICS` comment via lofty. 🟢
- **Auto-scroll**: TUI lyrics panel highlights the current line based on playback position. 🟢
- **External fetch**: LRCLIB API for community-contributed synced lyrics. [Validation Required] — verify LRCLIB API terms, availability, and rate limits. Opt-in feature. 🟡
- **Podcast chapters**: Display chapter titles in the lyrics panel position for podcast episodes. 🟢

### 4.5 — Playlist Management 🟢 (MVP for basic, 🟡 for full)

**MVP (Phase 1)**: Queue from CLI args (`player song1.mp3 song2.flac dir/`). Basic next/prev/shuffle/repeat.

**Full (Phase 2+)**:
- **TOML playlists**: Native format. Human-editable. Stored in `playlists/` directory.
- **M3U/M3U8 import/export**: Parse with a simple custom parser (M3U is trivial). 🟢
- **PLS import/export**: Similar simplicity. 🟢
- **Operations**: Create, rename, delete, reorder tracks, duplicate playlist.
- **Queue**: Separate from playlist. "Play next" inserts at queue head. "Add to queue" appends.
- **Shuffle**: True random (Fisher-Yates), shuffle-no-repeat (full cycle before repeating).
- **Repeat**: Off, repeat all, repeat one.
- **TUI**: Scrollable playlist panel with search/filter within playlist.

### 4.6 — Themes & Visual Customization 🟢 (Phase 2)

**Theme format**: TOML file defining named colors for each UI element.

**Shipped themes (10+)**: Nord, Catppuccin (Mocha, Latte), Gruvbox (Dark, Light), Tokyo Night, Rosé Pine, Dracula, Solarized (Dark, Light), One Dark, Monokai, Default Dark, Default Light.

**Hot-swap**: Cycle themes with a keybinding (e.g., `T`). Theme applied immediately — no restart.

**Custom themes**: User creates `.toml` files in `themes/` config directory. Auto-discovered on startup.

**Theme elements**: background, foreground, primary accent, secondary accent, progress bar (filled/empty), EQ bars (active/inactive), visualizer colors (gradient), border, selection highlight, status bar bg/fg, error color, warning color.

### 4.7 — Session Persistence & Resume 🟢 (MVP)

**Recommendation: TOML for session files.**

Saved state:
- Current track identifier (file path or URL)
- Playback position (seconds, f64)
- Queue contents and order
- Volume (dB)
- Active EQ preset name
- Visualizer mode
- Active theme name
- Shuffle/repeat state
- Paused/playing state

**Auto-save**: Every 30 seconds and on graceful quit. On crash: last auto-save is recovery point (max 30s lost).

**Named sessions** (Phase 3): Save/load named sessions (e.g., "work", "commute", "bedtime"). Default unnamed session is always auto-saved.

### 4.8 — User Configuration 🟢 (MVP)

**File**: `config.toml` in platform-specific config directory.

```toml
[audio]
default_volume = -6       # dB
buffer_ms = 200           # ring buffer size in ms
# output_device = "default"

[ui]
theme = "nord"
visualizer = "bars"
# compact = false

[providers]
enabled = ["local", "radio", "podcast"]

[providers.local]
library_paths = ["~/Music", "/mnt/nas/music"]

[providers.radio]
directory = "radio-browser"   # or "custom-only"

[keybindings]
play_pause = "space"
next = "n"
prev = "p"
volume_up = "+"
volume_down = "-"
quit = "q"
```

**Validation**: On startup, parse and validate config with clear error messages pointing to the specific invalid field. Unknown fields are warnings (forward compatibility), not errors.

**CLI override**: Every config value can be overridden via CLI flag for the session. CLI flags take precedence over config file.

### 4.9 — IPC Remote Control 🟡 (Phase 5)

**Protocol**: Unix domain socket on Linux/macOS. Named pipe on Windows. JSON-RPC 2.0 message format.

**Architecture**: The player process runs an IPC server (tokio task). CLI subcommands act as thin clients:
```bash
player pause                    # send pause command
player status --json            # query current state
player queue add song.mp3       # add to queue
player volume -3                # adjust volume by -3dB
```

**Commands**: play, pause, toggle, stop, next, prev, seek (absolute/relative), volume (absolute/relative), status, queue (list/add/remove/clear), playlist (load/save), shuffle, repeat, eq-preset, theme, visualizer.

**Status response** (JSON):
```json
{
  "state": "playing",
  "track": {"title": "Song", "artist": "Artist", "path": "/music/song.flac"},
  "position": 127.4,
  "duration": 312.0,
  "volume_db": -6,
  "eq_preset": "flat",
  "shuffle": false,
  "repeat": "off"
}
```

### 4.10 — Media Key & System Integration 🟡 (Phase 5)

- **Linux MPRIS**: D-Bus `org.mpris.MediaPlayer2` interface via `mpris-server` crate. Integrates with `playerctl`, GNOME/KDE media controls, Polybar, Waybar. 🟢 [Validated]
- **macOS NowPlaying**: `MPNowPlayingInfoCenter` via objc bindings or `souvlaki` crate. System media keys, Control Center, Touch Bar. 🟡 [Validation Required] — verify souvlaki macOS support
- **Windows**: Global media keys via `souvlaki` crate. [Validation Required]

### 4.11 — CLI Flags & Per-Session Overrides 🟢 (MVP, expanded over phases)

Built with `clap` v4 derive macros. Key flags:
```
player [OPTIONS] [FILES/URLS...]

Options:
  --volume <dB>          Override volume (-60 to +12)
  --shuffle              Enable shuffle
  --repeat <MODE>        off|all|one
  --mono                 Mono downmix
  --theme <NAME>         Override theme
  --eq-preset <NAME>     Override EQ preset
  --visualizer <MODE>    bars|wave|spectrogram|off
  --compact              Constrained-width mode
  --no-resume            Don't restore previous session
  --config <PATH>        Alternative config file
  -h, --help             Show help
  -V, --version          Show version

Subcommands (Phase 5 — IPC):
  status                 Show current playback status
  pause / play / toggle  Control playback
  next / prev            Track navigation
  volume <dB>            Set/adjust volume
  queue <SUBCOMMAND>     Queue management
```

---

## SECTION 5: FEATURES THE USER MAY HAVE MISSED

| # | Feature | Why It Matters | Phase | Complexity |
|---|---------|---------------|-------|------------|
| 1 | **File Browser** — in-player directory navigation for discovering and queuing local files | Core UX for local files. Without it, users must specify every file path on CLI. | Phase 2 | 🟢 |
| 2 | **Bookmark / Favorites System** — star tracks, stations, podcast episodes | Personalization. Quick access to preferred content across sessions. | Phase 2 | 🟢 |
| 3 | **Last.fm / ListenBrainz Scrobbling** — social listening history | Community feature, free marketing (profile pages link to player). Opt-in only. | Phase 6 | 🟡 |
| 4 | **ReplayGain / Volume Normalization** — consistent volume across tracks | Prevents jarring volume changes between tracks from different albums/sources. | Phase 3 | 🟡 |
| 5 | **Crossfade** — smooth transitions between tracks | Enhances casual listening. Configurable duration (0–12 seconds). | Phase 3 | 🟡 |
| 6 | **Audio Device Selection** — switch output devices at runtime | Users with multiple audio devices (speakers, headphones, DAC) need this. | Phase 3 | 🟡 |
| 7 | **Sleep Timer** — auto-stop after N minutes | Essential for bedtime listening. Trivial to implement. | Phase 2 | 🟢 |
| 8 | **Album Art in Terminal** — render via Sixel, Kitty protocol, or Unicode blocks | Visual appeal. Makes the player feel polished. Kitty protocol for supported terminals, fallback to none. | Phase 3 | 🟡 |
| 9 | **Shell Completions** — bash, zsh, fish, PowerShell | UX polish for terminal users. Auto-generated by `clap` v4. | Phase 2 | 🟢 |
| 10 | **Man Page Generation** — auto-generated from CLI definition | Standard Unix distribution practice. `clap_mangen` crate. | Phase 5 | 🟢 |
| 11 | **Headless/Daemon Mode** — run without TUI, controlled via IPC | Enables background playback, scripting, and remote control use cases. | Phase 5 | 🟡 |
| 12 | **Discord Rich Presence** — show currently playing in Discord status | Popular with younger users. Opt-in. `discord-rpc` crate. | Phase 6 | 🟢 |
| 13 | **Smart Playlists** — auto-generated from rules (genre, recently added, most played) | Power feature for large libraries. Requires library index (Phase 6). | Phase 6 | 🟡 |
| 14 | **Listening Statistics** — play counts, listening time, genre distribution | Personal insights. "Your year in music" style reports. Requires listening history data. | Phase 6 | 🟡 |
| 15 | **Accessibility / High Contrast Themes** — screen reader hints, accessible color schemes | Inclusive design. High contrast themes are easy; screen reader support in TUI is harder. | Phase 5 | 🟡 |

---

## SECTION 6: MUSIC LIBRARY MANAGEMENT (Phase 6 — Future)

### 6.1 — Library Scanning & Indexing 🟡

- **Scanner**: Walk configured directories recursively. Extract metadata via `lofty` for each audio file.
- **Index**: SQLite database with FTS5 extension for full-text search across title, artist, album, genre.
- **Schema**: `tracks(id, path, title, artist, album, genre, year, track_num, duration_ms, format, file_size, modified_at, scanned_at)`
- **Incremental re-scan**: Compare file `modified_at` timestamps. Only re-read metadata for changed files.
- **File watcher**: `notify` crate for filesystem change detection (inotify on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows). 🟢 [Validated]
- **Performance**: Target 100k+ files indexed. SQLite handles this easily. Initial scan of 10k files should complete in <30 seconds.

### 6.2 — Library Browsing 🟡

- Browse by: artist → albums → tracks, album (grid/list), genre, year, recently added, most played
- Search: FTS5 query across all metadata fields. Sub-100ms response for 100k library.
- Album view: Track list with metadata, album art (if available), total duration.

### 6.3 — Tag Management 🟡

- **Read**: All formats via `lofty`. [Validated]
- **Write**: `lofty` supports writing tags for MP3 (ID3v2), FLAC (Vorbis comments), OGG, MP4. [Validated]
- **Batch edit**: Select multiple tracks → set common fields (artist, album, genre, year).
- **MusicBrainz auto-tag**: [Validation Required] — MusicBrainz API is free with rate limits (1 req/sec). Use acoustic fingerprinting via `chromaprint` (C library, FFI) for identification. 🔴 (FFI complexity)

### 6.4 — Architecture Consideration

The library index is a **separate module** within the local files provider. It does NOT live in the domain core (it's provider-specific — only local files have a filesystem index). The library module exposes `Browsable` and `Searchable` traits that the local files provider delegates to. Playlists reference tracks by a stable `TrackId` (content hash or path-based) that works whether the library is indexed or not.

---

## SECTION 7: PLUGIN & EXTENSION SYSTEM (Post-1.0 — Phase 8)

**Prerequisite**: Core playback must be stable and in daily use. Internal APIs must be settled. Do not build this until Phase 6 is complete.

### 7.1 — Plugin Architecture

| Runtime | Language | Sandboxing | Performance | Ecosystem | Recommendation |
|---------|----------|-----------|-------------|-----------|----------------|
| Lua (mlua) | Lua 5.4 | Moderate (restrict stdlib) | Good | Mature, proven in games/tools | ✅ **Primary** |
| WASM (wasmtime) | Any→WASM | Excellent (capability-based) | Good | Growing | Runner-up — consider as secondary |
| Rhai | Rhai | Good | Good | Small | ❌ Too niche, limited community |
| Native (dylib) | Rust/C | None | Best | N/A | ❌ No sandboxing — security risk |
| Python (PyO3) | Python | Weak | Moderate | Large | ❌ Heavy runtime, poor sandboxing |

**Recommendation: Lua via `mlua` crate as the primary plugin runtime.** Lua is lightweight (~200KB runtime), fast, proven for embedding (Neovim, Redis, game engines), and `mlua` is well-maintained. Sandboxing is achieved by not exposing `os`, `io`, `debug` standard library modules by default — plugins must request permissions in their manifest.

**Future secondary**: WASM via `wasmtime` for complex plugins that need language-agnostic compilation. Add only after Lua plugin ecosystem is established.

### 7.2 — Plugin API Surface

Plugins can:
- **Subscribe to events**: `on("track.change", fn)`, `on("playback.pause", fn)`, `on("app.quit", fn)`
- **Read state**: `player.current_track()`, `player.position()`, `player.volume()`, `player.eq_bands()`
- **Register commands**: `register_command("scrobble", fn)` — accessible via IPC
- **Register keybindings**: `register_key("ctrl+l", fn)` — with permission
- **Make HTTP requests**: `http.get(url)`, `http.post(url, body)` — requires `network` permission
- **File I/O**: `fs.read(path)`, `fs.write(path, data)` — sandboxed to `plugins/<name>/data/`
- **Set timers**: `set_timeout(fn, ms)`, `set_interval(fn, ms)`
- **Log**: `log.info(msg)`, `log.error(msg)` — written to `plugins/<name>/log.txt`

**Security**: Permission manifest in `plugin.toml`:
```toml
[plugin]
name = "lastfm-scrobbler"
version = "0.1.0"
permissions = ["network", "events.track.change"]
```

### 7.3 — Plugin Distribution 🟡

- Plugins in `~/.config/kora/plugins/<name>/`
- Each plugin: `plugin.toml` (manifest) + `main.lua` (entry point) + optional data files
- CLI management: `player plugins install <github-url>`, `player plugins list`, `player plugins remove <name>`
- Future: community plugin registry on project website

---

## SECTION 8: MOONSHOT FEATURES

### 8.1 — Web Interface 🔴 (Phase 9 — Research)

- **Engine**: Compile playback core to WASM. Decode in WASM, output via AudioWorklet.
- **Frontend recommendation: Leptos** (Rust-native WASM framework). Keeps the entire stack in Rust, simplifies build pipeline, leverages Rust expertise. Alternative: Svelte (lighter JS bundle, better ecosystem) — choose based on developer preference.
- **Modes**: Standalone (local files via File API, streaming via fetch) and Remote Control (WebSocket to running CLI instance).
- **Limitations**: CORS restrictions on radio stream URLs. Some radio stations don't send CORS headers — would need a proxy. [Validation Required]
- **PWA**: Service worker for offline support, installable as web app.

### 8.2 — Native macOS/iOS App 🔴 (Phase 10 — Moonshot)

- **FFI**: Rust core compiled as static library, exposed via UniFFI-generated Swift bindings.
- **iOS features**: Background audio (`AVAudioSession`), Lock Screen controls (`MPNowPlayingInfoCenter`), CarPlay (audio app template), Home Screen widget.
- **macOS features**: Menu bar player (mini player in menu bar), native notifications, Handoff.
- **Apple Watch**: Now Playing complication, basic transport controls (play/pause/skip). Extremely limited — audio processing happens on paired iPhone. [Validation Required] — watchOS audio streaming capabilities.
- **iCloud sync**: Sync config, playlists, favorites, podcast state via iCloud Key-Value Store or CloudKit.

### 8.3 — Desktop App (Tauri v2) 🟡 (Phase 9 — Research)

- Tauri wraps the web frontend (Leptos/Svelte) with native audio backend (CPAL, not Web Audio).
- System tray with mini player controls.
- Global hotkeys via Tauri's API.
- Native file system access (no File API limitations).
- Auto-update via Tauri's built-in updater.

### 8.4 — Self-Hosted Sync Server 🔴 (Phase 10 — Moonshot)

- Lightweight Rust server (axum + SQLite).
- REST API for CRUD on playlists, favorites, podcast state, listening history, config.
- No audio data stored — metadata and state only.
- Docker image for easy self-hosting.
- Authentication: simple token-based auth (no OAuth complexity for self-hosted).
- Future: act as podcast episode cache proxy.

### 8.5 — Social & Sharing 🟡 (Phase 6+)

- **Last.fm/ListenBrainz**: Scrobble API integration. Opt-in, configured per-provider in config. `rustfm-scrobble` crate or direct API calls. 🟢
- **Discord Rich Presence**: Show track info in Discord. `discord-rpc` crate. Opt-in. 🟢
- **Share "Now Playing"**: Copy formatted text to clipboard. 🟢
- **Collaborative playlists** and **public profiles**: Require sync server. Phase 10.

---

## SECTION 9: TECH STACK RECOMMENDATIONS

| Component | Recommendation | Justification | Alternatives Rejected |
|-----------|---------------|---------------|----------------------|
| **Language** | Rust (stable channel) | Memory safety, zero-cost abstractions, excellent concurrency, growing audio ecosystem | Go (cliamp's choice — no zero-cost DSP, GC pauses risk audio glitches), C++ (unsafe, harder to contribute to), Zig (immature ecosystem) |
| **Audio Decoding** | symphonia | Pure Rust, no C deps, supports MP3/FLAC/OGG/Opus/WAV/AAC(experimental). Actively maintained. [Validated] | ffmpeg bindings (C dependency, complex build), gstreamer (heavy, Linux-centric), rodio (uses symphonia internally but hides control) |
| **Audio Output** | cpal | De facto Rust audio output. Supports ALSA, PipeWire, CoreAudio, WASAPI. Actively maintained. [Validated] | rodio (higher-level, less control over buffer management), platform-specific (3x maintenance) |
| **DSP / EQ** | Custom biquad filters (in-house) | Biquad EQ filters are well-documented (~50 lines of code per filter). No crate needed. Full control over the DSP chain. | dasp (too general), external DSP library (unnecessary dependency for biquad filters) |
| **Sample Rate Conversion** | rubato | Pure Rust, high quality (sinc interpolation), async resampling API fits the pipeline. [Validated] | libsamplerate (C FFI), custom (too much work) |
| **TUI Framework** | ratatui + crossterm | ratatui is the successor to tui-rs, actively maintained, rich widget set. crossterm is cross-platform terminal backend. [Validated] | cursive (different paradigm, less flexible layout), tui-rs (unmaintained) |
| **CLI Parsing** | clap v4 (derive) | Most popular Rust CLI framework. Derive macros reduce boilerplate. Built-in completions and man page generation. [Validated] | argh (minimal, fewer features), structopt (merged into clap v4) |
| **Configuration** | toml + serde | TOML is human-readable, Rust ecosystem standard for config. serde makes (de)serialization trivial. [Validated] | figment (layered config — adds complexity without clear benefit for MVP), config-rs (less maintained) |
| **Async Runtime** | tokio | Industry standard. Required by reqwest, needed for network I/O, IPC server. [Validated] | async-std (smaller community), smol (minimal — less ecosystem support) |
| **HTTP Client** | reqwest | Built on tokio/hyper, supports streaming responses (essential for radio/podcast), cookies, redirects. [Validated] | ureq (blocking — doesn't support streaming well), hyper (too low-level) |
| **IPC** | Unix domain socket + JSON-RPC 2.0 (via tokio) | Simple, no external deps, fast. Named pipes on Windows. JSON-RPC is human-debuggable. | D-Bus (Linux-only), gRPC (heavy for local IPC), MQTT (overkill) |
| **Metadata/Tags** | lofty | Pure Rust, supports ID3v2, Vorbis, MP4, FLAC, APE tags. Read and write. Actively maintained. [Validated] | id3 (MP3 only), symphonia metadata (read-only, no write) |
| **RSS Parsing** | feed-rs | Supports RSS 2.0, Atom, JSON Feed. Handles real-world feed quirks. [Validated] | rss crate (RSS only, no Atom), custom (feeds are messy — don't hand-roll) |
| **Lyrics (LRC)** | Custom parser | LRC is trivial (~30 lines of Rust). No crate dependency justified. | lrc crate (if it exists — likely unmaintained) |
| **Database** | SQLite via rusqlite (Phase 6+) | Proven, embedded, zero-config, supports FTS5. Perfect for library index. [Validated] | sled (Rust-native but less mature, no FTS), redb (too simple for library queries) |
| **Full-text Search** | SQLite FTS5 | Comes free with rusqlite. No additional dependency. Sufficient for music library search. | tantivy (overkill for this use case, adds large dependency) |
| **Plugin Runtime** | Lua via mlua (Phase 8) | See Section 7.1. Lightweight, proven, good sandboxing story. | WASM/wasmtime (future secondary), Rhai (too niche) |
| **FFI (mobile)** | UniFFI (Phase 10) | Auto-generates Swift/Kotlin bindings from Rust. Mozilla-maintained. [Validation Required] | cbindgen (manual, more maintenance), cxx (C++ focused) |
| **WASM Bindings** | wasm-bindgen + wasm-pack (Phase 9) | Standard Rust→WASM toolchain. [Validated] | stdweb (unmaintained) |
| **Web Frontend** | Leptos (Phase 9) | Rust-native WASM framework. Keeps entire stack in Rust. Fine-grained reactivity, small bundle. | Svelte (good alternative if JS is preferred), React (heavy), Solid (smaller community) |
| **Desktop Wrapper** | Tauri v2 (Phase 9) | Rust-native, lighter than Electron, native system tray, built-in updater. [Validated] | Electron (heavy, memory-hungry), native per-platform (too expensive for solo dev) |
| **CI/CD** | GitHub Actions | Free for open source, excellent Rust support, matrix builds for cross-platform. [Validated] | GitLab CI (less community visibility) |
| **Distribution** | cargo-dist (GitHub Releases) + Homebrew (Phase 5) + AUR (Phase 5) | cargo-dist auto-generates release binaries for all platforms. Homebrew/AUR added when user base grows. | Manual releases (error-prone), Nix (niche, add later via community) |
| **Logging** | tracing | Structured, async-aware, span-based logging. Industry standard in Rust. [Validated] | log + env_logger (less structured, no spans) |
| **Error Handling** | thiserror (library errors) + anyhow (application errors) | thiserror for typed errors in core/playback crates. anyhow for CLI binary where error context matters more than types. [Validated] | eyre (similar to anyhow, less popular), miette (fancy diagnostics — overkill for audio player) |
| **Testing** | Built-in #[test] + proptest (property-based) | Standard Rust testing. proptest for DSP correctness (e.g., "EQ at 0dB gain = passthrough"). | quickcheck (less ergonomic than proptest) |

**MSRV Policy**: Track stable Rust minus 2 releases (e.g., if current stable is 1.82, MSRV is 1.80). This balances access to new features with distro packager needs.

**Dependency Policy**: Minimize dependencies. Every new crate must justify its inclusion. Prefer pure Rust crates over C FFI. Audit `cargo tree` regularly. Use `cargo deny` for license and advisory checking in CI.

**Build Time**: Use `cargo-nextest` for parallel test execution. CI caches `target/` and `~/.cargo/registry`. Workspace incremental compilation. Consider `sccache` if CI times exceed 10 minutes.

### 9.1 — Proof-of-Concept Gates

**Gate 1: CPAL Audio Output (Before Phase 0 completion)**
- [ ] Linux: CPAL plays decoded MP3 samples through ALSA and PipeWire
- [ ] macOS: CPAL plays decoded MP3 samples through CoreAudio
- [ ] Windows: CPAL plays decoded MP3 samples through WASAPI
- [ ] Device enumeration: list available output devices
- [ ] Suspend/resume: laptop sleep → wake → audio resumes without crash
- [ ] Underrun: artificially delay decode thread → verify silence (not crash)
- **Fallback if CPAL fails**: Platform-specific backends (much more work — escalates project risk significantly). Likelihood of failure: Low. [Validated] — CPAL is widely used.

**Gate 2: WASM Audio (Before Phase 9 commitment)**
- [ ] Decode MP3 entirely in WASM (symphonia compiled to wasm32)
- [ ] AudioWorklet plays decoded samples with acceptable latency (<100ms)
- [ ] Fetch and decode a remote audio URL (CORS permitting)
- [ ] Local file playback via File API
- **Fallback if WASM fails**: Web frontend as remote control only (no in-browser decoding). The web UI controls a running CLI instance via WebSocket.

**Gate 3: UniFFI for Apple Platforms (Before Phase 10 commitment)**
- [ ] Minimal Rust library callable from Swift on macOS via UniFFI
- [ ] Async events (track change, position update) delivered from Rust to Swift callback
- [ ] Static library packaging for iOS (no dynamic linking)
- [ ] watchOS: basic feasibility test (can Rust code run on watchOS at all?)
- **Fallback if UniFFI fails**: Manual cbindgen with C-compatible API surface. More boilerplate, but proven approach.

---

## SECTION 9A: AUDIO CORRECTNESS & RELIABILITY TESTING

### 9A.1 — Decoder Tests

- **Golden file tests**: Maintain a set of reference audio files (one per format) with known decoded sample checksums. Decode and compare. Run in CI.
- **Format coverage**: MP3 (CBR, VBR), FLAC (16-bit, 24-bit), OGG Vorbis (multiple quality settings), Opus, WAV (PCM 16/24/32, float 32).
- **Edge cases**: Files <1 second, files >2 hours, variable bitrate MP3, files with corrupted headers (should error gracefully, not panic), truncated files, zero-length files, files with extensive metadata (large embedded album art).
- **Metadata accuracy**: Compare extracted metadata against known values for reference files.

### 9A.2 — DSP & Pipeline Tests

- **EQ passthrough**: All bands at 0dB → output equals input (within floating-point tolerance). Use `proptest` for this.
- **EQ frequency response**: Boost band at 1kHz by +6dB → measure output energy at 1kHz vs other frequencies using FFT. Verify boost is within ±1dB of target.
- **Volume accuracy**: Set gain to -6dB → output samples are 0.5× input (within tolerance).
- **Resampler quality**: Resample 44.1kHz→48kHz→44.1kHz → compare to original. SNR should be >90dB.
- **Seek accuracy**: Seek to 60.0s → next decoded sample should be within ±50ms of target.
- **Gapless test (Phase 3)**: Generate two synthetic tracks (sine waves at different frequencies). Play back-to-back. Record output. Verify no silence gap >1ms and no click artifact at the transition.
- **Speed change test (Phase 3)**: Play at 2.0x → verify output duration is ~50% of original.

### 9A.3 — Playback Reliability Tests

- **Network fault injection**: Simulate disconnects, redirects (301/302), slow streams (1 byte/sec), invalid ICY metadata, HTTPS cert errors. Verify graceful recovery or clear error messages.
- **Underrun simulation**: Artificially throttle the decode thread (sleep between buffer fills). Verify silence output (not crash/noise) and recovery when throttle is removed.
- **Format switching**: Play MP3 → FLAC → OGG → Opus in sequence without restarting. Verify seamless transitions (correct sample rate conversion between formats).
- **Large playlist**: Load 10k+ track queue. Verify memory stays bounded (<100MB), UI remains responsive, next/prev and shuffle work correctly.
- **24-hour stability test**: Play internet radio for 24+ hours. Monitor for memory leaks (RSS should be stable ±10%), file descriptor leaks, CPU creep.

### 9A.4 — CI & Platform Testing

- **Mock audio backend**: Implement `AudioOutput` trait with a `MockOutput` that writes samples to a buffer/file instead of a real device. All pipeline tests use this — no real audio hardware needed in CI. 🟢
- **Platform matrix**: GitHub Actions runners: Ubuntu (ALSA headers installed), macOS, Windows. Compile and run tests on all three.
- **Performance budgets** (validate after Phase 1, enforce from Phase 2):
  - Startup time: <500ms to first UI render (no audio playing)
  - Memory: <50MB RSS while playing a local FLAC file
  - CPU: <5% during steady-state MP3 playback on a modern machine
  - (Adjust thresholds based on actual measurement — these are initial targets)

### 9A.5 — TUI Responsiveness Tests

- **Input latency**: Measure time from keypress event to visual update. Target: <50ms.
- **Terminal resize**: Resize terminal during playback. UI must re-layout without crash or visual corruption. Test with `resize` events in crossterm.
- **Visualizer frame rate**: Measure actual FPS under load. Target: 20+ FPS during playback. If TUI thread falls behind, skip frames — never block audio.

---
---
## SECTION 10: DATA MODEL

### Entity Relationships

```
UserConfig (1) ---- (N) ProviderConfig
     |
     +---- (1) Session ---- (1) Queue ---- (N) Track
                                              |
Track ---- (0..1) TrackMetadata               |
                                              |
Playlist (N) ---- (N) Track ------------------+

RadioStation (standalone, favoritable)
PodcastFeed (1) ---- (N) PodcastEpisode
Favorite ---- Track | RadioStation | PodcastEpisode
ListeningHistory (N) ---- Track
EQPreset (standalone, referenced by Session)
Theme (standalone, referenced by Session)
```

### Storage Strategy

| Entity | Storage | Format | Syncs? |
|--------|---------|--------|--------|
| UserConfig | `config.toml` | TOML | Yes |
| Session | `session.toml` | TOML | Yes |
| Playlists | `playlists/<name>.toml` | TOML | Yes |
| EQPreset (custom) | In `config.toml` | TOML | Yes |
| Theme (custom) | `themes/<name>.toml` | TOML | Yes |
| RadioStations (custom) | `stations.toml` | TOML | Yes |
| PodcastFeeds + EpisodeState | `podcasts.toml` | TOML | Yes |
| Favorites | `favorites.toml` | TOML | Yes |
| ListeningHistory | `history.toml` -> SQLite (Phase 6) | TOML->SQLite | Yes |
| TrackMetadata (cache) | In-memory HashMap | Memory | No |
| Library Index (Phase 6) | `library.db` | SQLite | No (local cache) |
| Credentials | OS keychain | Platform-specific | No |

### Migration Strategy
- Phase 0-5: TOML files only. Schema changes = add fields with defaults (backward compatible).
- Phase 6: Introduce SQLite for library + history. TOML remains for config/session/playlists.
- Cross-platform sync: TOML files use relative paths (`~/Music/...`), no platform-specific separators.
- Track identity: content hash (for local) or provider-specific ID (for streaming) — not file path.

---

## SECTION 11: DESIGN BRIEF

### 11.1 -- TUI Design Principles

1. **Keyboard-Native**: Every action accessible via keyboard. Mouse optional.
2. **Information-Dense, Uncluttered**: Track info, progress, playlist, and controls on one screen.
3. **Responsive**: Graceful degradation from 200x60 down to 80x24. Panels collapse at breakpoints.
4. **Beautiful by Default**: Attractive default theme. First impression matters.
5. **Vim-like Navigation**: j/k, g/G, Ctrl+D/U everywhere lists appear.
6. **Discoverable**: ? or Ctrl+K shows all keybindings. Status bar hints for common actions.

### 11.2 -- Main TUI Layout

```
+-- Track Info -------------------------------------------------+
| Artist -- Title                              Album            |
| > ================================= 2:22 / 5:04              |
| Vol: -3dB  EQ: Rock  Shuffle: Off  Repeat: Off               |
+-- Playlist ----------------------+-- Visualizer --------------+
| > 1. Currently Playing           |  ##                        |
|   2. Next Track                  |  ## ##                     |
|   3. Another Track               |  ## ## ## ##               |
|   4. ...                         |  ## ## ## ## ##            |
+----------------------------------+----------------------------+
| Spc:Play/Pause  n/p:Next/Prev  Up/Dn:Select  ?:Help          |
+---------------------------------------------------------------+
```

At small terminals (< 100 cols): hide visualizer panel.
At very small (80x24): collapse to track info + playlist only.
`--compact` flag: cap width at 80 columns.

### 11.3 -- Default Keybinding Table

| Key | Action | Category |
|-----|--------|----------|
| Space | Play / Pause | Transport |
| s | Stop | Transport |
| n / p | Next / Previous | Transport |
| <- / -> | Seek +/-5s | Transport |
| Shift+<- / -> | Seek +/-30s | Transport |
| + / - | Volume +/-1dB | Transport |
| ] / [ | Speed +/-0.25x | Transport |
| j / k | Scroll down / up | Navigation |
| g / G | Top / Bottom of list | Navigation |
| Ctrl+D / Ctrl+U | Half-page down / up | Navigation |
| Enter | Play selected track | Navigation |
| / | Search/filter playlist | Navigation |
| Tab | Focus next panel | Navigation |
| f | Toggle favorite | Action |
| o | Open file browser | Action |
| u | Load URL | Action |
| e | Cycle EQ preset | Audio |
| t | Cycle theme | UI |
| v | Cycle visualizer mode | UI |
| V | Full-screen visualizer | UI |
| r | Cycle repeat (off/all/one) | Playback |
| z | Toggle shuffle | Playback |
| m | Toggle mono downmix | Audio |
| ? / Ctrl+K | Show all keybindings | Help |
| q | Quit (auto-saves session) | System |

### 11.4 -- Future Platform UI Approach

- **Web**: Responsive SPA, dark mode default, Media Session API for browser media keys.
- **macOS App**: Sidebar + content layout, menu bar mini-player, global hotkeys.
- **iOS**: Tab-based nav (Now Playing, Library, Radio, Podcasts), Lock Screen widget, CarPlay.
- **Apple Watch**: Now Playing complication, basic transport controls glance.

---

## SECTION 12: COMPETITIVE ANALYSIS

### 12.1 -- Terminal Audio Players

| Player | Language | Key Features | Platforms | Strengths | Weaknesses |
|--------|---------|-------------|-----------|-----------|------------|
| **cliamp** | Go | Full-featured, 10+ providers, EQ, viz, plugins, themes | Linux, macOS | Most complete TUI player | No mobile/web, single-platform architecture |
| **ncmpcpp** | C++ | MPD client, tag editor, visualizer | Linux, macOS | Mature, MPD ecosystem | Requires MPD, dated UI |
| **cmus** | C | Lightweight, fast, Vi-like | Linux, macOS, *BSD | Very fast, minimal | No streaming, minimal features |
| **musikcube** | C++ | Library management, streaming server | Linux, macOS, Win | Library + server | Complex setup |
| **termusic** | Rust | TUI player, podcast support | Linux, macOS | Rust, some features | Smaller feature set |
| **mpv** | C | Universal media player | All | Plays everything | Not music-focused, no TUI |

### 12.2 -- Differentiation

This project's unique position:
1. **Pure Rust audio pipeline** -- no C dependencies for core codecs
2. **TUI-first with rich visuals** -- EQ, visualizer, themes, lyrics
3. **Multi-source** -- local + radio + podcasts in one player
4. **Layered architecture** -- designed for WASM/mobile expansion `[Validation Required]`
5. **Open source, zero telemetry** -- counter-position to Spotify/Apple Music

---

## SECTION 13: LICENSING & OPEN SOURCE STRATEGY

### License: Dual MIT + Apache 2.0

Rust ecosystem standard. Maximum contribution friendliness. Apache 2.0 patent grant. No copyleft friction. Compatible with all major Rust crate licenses.

**Rejected**: GPL v3 (copyleft friction), AGPL (too restrictive for library crate), MIT-only (no patent grant).

### Contribution Model

- **DCO** (Developer Certificate of Origin) -- `Signed-off-by` trailer, simpler than CLA
- CONTRIBUTING.md: build instructions, architecture overview, coding conventions, PR process
- Issue templates: Bug Report, Feature Request, Provider Request
- "Good first issue" label for onboarding new contributors

### Monetization

**Primary**: GitHub Sponsors. Integrated, simple, no tier complexity initially.
**Secondary**: Ko-fi for casual one-time donations.

---

## SECTION 14: PHASED ROADMAP WITH MILESTONES

### Phase 0: First Sound
**Goal**: `kora song.mp3` produces audio on Linux, macOS, and Windows.
**Deliverables**: Cargo project, module structure, symphonia -> rtrb -> CPAL pipeline, basic CLI, CI.
**Acceptance**: Audio plays correctly on all 3 platforms. CI green.
**Cut line**: Windows can slip to Phase 1 if WASAPI issues arise.

### Phase 1: Minimum Playable Player (MVP)
**Goal**: A TUI player the developer uses daily for local file playback.
**Deliverables**: ratatui TUI (track info, progress, controls), queue from CLI args, keyboard controls, one theme, session persistence, config.toml.
**Acceptance**: Developer uses it instead of mpv/cmus for 1 week. Quit -> restart -> resume works.
**Cut line**: Config can be minimal (volume + theme only).

### Phase 2: Daily Driver (Beta)
**Goal**: Local + radio + basic podcast. Multiple themes.
**Deliverables**: HTTP stream playback, Radio Browser integration + custom stations, basic podcast RSS, file browser, 10+ themes, shuffle/repeat, sleep timer, favorites.
**Acceptance**: Play a radio station with ICY metadata. Play a podcast episode. Resume position.
**Cut line**: Podcast downloads defer to Phase 4.

### Phase 3: Audio Polish
**Goal**: The player sounds great.
**Deliverables**: 10-band graphic EQ, spectrum visualizer (bars), gapless playback, speed control, AAC/ALAC, synced lyrics, ReplayGain, audio device selection.
**Acceptance**: EQ audibly changes sound. Gapless album playback. Visualizer syncs with audio.
**Cut line**: Lyrics and ReplayGain can defer.

### Phase 4: Podcast Client
**Goal**: Full podcast experience.
**Deliverables**: OPML, subscriptions, episode tracking, downloads, chapters, per-feed speed.
**Acceptance**: Import OPML, subscribe, auto-refresh, download episodes, track played state.

### Phase 5: Power User & Ecosystem
**Goal**: Scriptable, system-integrated.
**Deliverables**: IPC remote control, MPRIS/media keys (souvlaki), CLI flags, shell completions, headless/daemon mode, man page.
**Acceptance**: `kora pause` controls a running instance. Media keys work on all platforms.

### Phase 6: Library & Social (1.0 Release)
**Goal**: Library management. This is the 1.0 milestone.
**Deliverables**: SQLite library index, library browser, statistics, Last.fm/ListenBrainz scrobbling (opt-in), smart playlists, comprehensive docs, Homebrew/AUR/cargo-dist packaging.
**Acceptance**: Index 10k+ files in <30s. Browse by artist/album/genre. Scrobble to Last.fm.

### Phase 7: Streaming Providers (Post-1.0)
Open providers first: Navidrome (Subsonic), Jellyfin, Plex `[V.R.]`. OS keychain credentials.
Spotify/YouTube: **Research only** -- pursue only after ToS validation.

### Phase 8: Plugin System (Post-1.0)
Lua runtime (mlua), plugin API, manager CLI. Only after core APIs stabilize.

### Phase 9: Cross-Platform Expansion (Research)
Each requires a proof-of-concept gate (Section 9.1): WASM engine, web frontend, Tauri desktop, macOS/iOS FFI.

### Phase 10: Native Mobile & Sync (Moonshot)
iOS app, Apple Watch, self-hosted sync server (axum + SQLite), cross-device state sync.

---

## SECTION 15: FIRST 90 DAYS -- EXECUTION PLAN

### 15.1 -- Technical Spikes (Weeks 1-2)

| # | Spike | Success Criteria | Effort |
|---|-------|-----------------|--------|
| 1 | symphonia decode MP3/FLAC/OGG -> f32 | Correct output, <1ms/frame | S |
| 2 | CPAL output on Linux (ALSA + PipeWire), macOS, Windows | Audio plays on all three | M |
| 3 | rtrb ring buffer: decode thread -> audio callback | No underruns at 100ms buffer | S |
| 4 | ratatui: render progress bar + respond to keypress | <50ms input latency | S |
| 5 | TOML session: serialize/deserialize playback state | Round-trip preserves all fields | S |
| 6 | HTTP stream: reqwest -> decode -> play | Radio URL plays with ICY metadata | M |

Defer from first 90 days: IPC, plugins, provider abstraction, WASM/FFI, full podcast client.

### 15.2 -- First 10 Epics

| # | Epic | Effort | Dependencies | Complexity |
|---|------|--------|-------------|------------|
| 1 | Audio pipeline (decode -> ring buffer -> CPAL) | M | Spikes 1-3 | Y |
| 2 | Basic CLI (`kora file.mp3` plays) | S | Epic 1 | G |
| 3 | Multi-file queue (play *.mp3, next/prev) | M | Epic 2 | G |
| 4 | TUI shell (track info, progress bar, controls) | M | Epic 3, Spike 4 | Y |
| 5 | Seek & volume controls | S | Epic 4 | G |
| 6 | Session persistence (quit -> resume) | S | Epic 5, Spike 5 | G |
| 7 | Config file (config.toml, basic settings) | S | Epic 6 | G |
| 8 | Theme support (one default theme) | S | Epic 4 | G |
| 9 | CI pipeline (GitHub Actions, 3 platforms) | M | Epic 1 | G |
| 10 | First release (v0.1.0 on GitHub Releases) | S | Epic 9 | G |

### 15.3 -- Due Diligence Backlog

- [ ] Radio Browser API: test rate limits, mirror availability, response format
- [ ] LRCLIB lyrics API: verify availability, terms, response format
- [ ] souvlaki crate: verify MPRIS + macOS + Windows support quality
- [ ] keyring crate: test on Linux (libsecret), macOS, Windows
- [ ] symphonia AAC decoder: quality and completeness
- [ ] Pitch-preserving time-stretch: evaluate Rust crates or C bindings
- [ ] Spotify Developer ToS: open-source player feasibility
- [ ] yt-dlp legal status: community consensus

### 15.4 -- Architecture Decision Records (ADRs)

1. **ADR-001**: Audio pipeline threading model (decode -> ring buffer -> callback)
2. **ADR-002**: Single crate with module boundaries vs multi-crate workspace
3. **ADR-003**: Provider trait design (capability-based, evolvable)
4. **ADR-004**: Session persistence format (TOML, not SQLite)
5. **ADR-005**: Ring buffer implementation choice (rtrb)

---

## SECTION 16: DEPENDENCY MAP

```
Phase 0 --> Phase 1 --> Phase 2 --+-- Phase 3 --> Phase 5 --> Phase 6 (1.0)
                                   |
                                   +-- Phase 4 ----------------+
                                                                |
(Parallelizable: CI, docs, themes, config design)               |
                                                                v
Post-1.0: Phase 7 (providers), Phase 8 (plugins) -- independent of each other

Research: Phase 9 (requires PoC gates) --> Phase 10 (requires Phase 9 success)
```

**Critical path**: Phase 0 -> Phase 1. The audio pipeline is the single critical dependency. Everything downstream requires reliable decode -> output. If this takes longer than expected, all phases shift.

**Parallelizable with Phase 0**: Theme design, config format, documentation, CI setup, Radio Browser API investigation, competitive analysis.

---

## SECTION 17: FEASIBILITY & COMPROMISE MATRIX

| Challenge | Ideal Solution | Compromise | Impact of Compromise | Recommendation |
|-----------|---------------|-----------|---------------------|----------------|
| Gapless playback | Pre-decode next track, seamless buffer swap | 50ms crossfade between tracks | Audiophiles notice | Start with crossfade; implement true gapless in Phase 3 |
| Cross-platform audio | CPAL everywhere | Platform-specific fallbacks for edge cases | More code, more testing | CPAL first; add fallbacks only if bugs found |
| Pitch-preserving speed | WSOLA/phase vocoder | Accept pitch change with speed | Podcasters want pitch preservation | Pitch shift for MVP; investigate time-stretch for Phase 4 |
| WASM audio | Full engine in WASM + AudioWorklet | Separate lightweight web player | Code duplication | Defer until PoC validates feasibility |
| Spotify integration | Official API + raw audio | Skip entirely for 1.0 | Can't play Spotify in our player | **Skip.** Focus on open providers. |
| Lyrics | Real-time synced from multiple sources | LRC sidecar files + embedded only | No lyrics for streaming content | LRC + embedded first; external API later |
| Visualizer performance | 60fps dedicated render thread | 20fps on TUI thread, drop frames | Less smooth | 20fps on TUI thread; optimize later if needed |
| Plugin sandboxing | WASM isolation | Lua sandbox (restrict stdlib) | Less isolation than WASM | Lua sandbox with restricted stdlib; require permission manifest |

---

## SECTION 18: NAMING & BRANDING (OPTIONAL)

**Top candidates** (all `[Validation Required]` -- check crates.io, GitHub, domains):

| Name | Strengths | Weaknesses |
|------|-----------|------------|
| **kora** | Short (4 chars), musical (West African string instrument), distinctive, easy to type | May conflict with existing projects |
| **cadence** | Musical term (rhythm), elegant, 7 chars | May conflict with Linux Cadence audio tool |
| **riff** | Musical (guitar riff), short (4 chars), punchy | Very common word, likely conflicts |
| **forte** | Musical (loud), short (5 chars), strong feel | May conflict |
| **resonance** | Descriptive, audio-themed, professional | Long to type (9 chars) |
| **wavelet** | Audio/DSP reference, technical, short-ish | Niche/academic feel |
| **rustamp** | Clear Rust + Winamp heritage | A bit literal |
| **oxide** | Rust-themed (iron oxide), short | Overused in Rust ecosystem |

**Recommendation**: **kora** -- Short, distinctive, musical, easy to type, unlikely to conflict.
`kora play ~/Music/`, `kora status --json`, `kora --theme Nord`.

**Runner-ups**: **cadence** (if no Linux conflict), **riff** (if crate available).

---

## SECTION 19: FAILURE MODE ANALYSIS

| # | Failure Mode | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | Audio pipeline complexity delays MVP | Medium | High (blocks everything) | Phase 0 dedicated to proving pipeline. Technical spikes before TUI work. |
| 2 | CPAL platform inconsistencies | Medium | Medium | Test all platforms in Phase 0. rodio as higher-level fallback. Accept platform workarounds. |
| 3 | Feature creep | High | High (burnout, never ships) | 3-5 deliverables per phase. Release tier definitions. "Daily use" as MVP criterion. |
| 4 | Streaming provider APIs restrictive | High (Spotify/YT) | Low (for 1.0) | 1.0 excludes restricted providers. Open providers (radio, podcast, Subsonic) are sufficient. |
| 5 | WASM/web target impractical | Medium | Low (moonshot) | PoC gate required. Failure = web is separate app. |
| 6 | Rust -> Swift FFI too complex | Medium | Low (moonshot) | PoC gate required. Failure = no mobile app. |
| 7 | **Solo developer burnout** | **High** | **Critical** | Open-ended timeline. Ship usable increments. Daily use as motivation. Accept contributions. **The project succeeds if Phase 1 ships -- everything after is bonus.** |

---

## SECTION 20: OPEN QUESTIONS & DECISION LOG

| # | Decision | Recommendation | Status |
|---|----------|----------------|--------|
| 1 | Audio backend | CPAL | Decided |
| 2 | Workspace structure | Single crate with clear modules; split when needed | Decided |
| 3 | Plugin runtime | Lua (mlua), post-1.0 | Decided |
| 4 | Podcast scope (beta) | Simple RSS playback | Decided |
| 5 | Radio directory | Radio Browser API + user TOML | Decided |
| 6 | Session persistence | TOML files | Decided |
| 7 | Sync strategy | File-based via cloud drives; sync server post-1.0 | Decided |
| 8 | License | Dual MIT + Apache 2.0 | Decided |
| 9 | Web frontend framework | Leptos (Rust WASM) or Svelte | Open -- defer to Phase 9 |
| 10 | FFI for Apple platforms | UniFFI | Open -- defer to Phase 9 |
| 11 | Visualizer thread model | TUI thread at 20fps | Decided |
| 12 | Project name | **kora** (recommended) | Open -- validate availability |
| 13 | Pitch-preserving speed | Defer; accept pitch shift for MVP | Decided |
| 14 | Spotify viability | Skip for 1.0; Research only | Decided |
| 15 | Stream recording | Red line -- will not implement | Decided |

---

*This roadmap is a living document. Revisit decisions as the project evolves. The most important thing is Phase 0: get sound out of the speakers. Everything else follows.*