# ADR-001: Audio Pipeline Threading Model

## Status

Accepted

## Context

kora's audio pipeline must decode audio from various sources (files, HTTP streams) and
output it through the system audio device with zero audible glitches. The primary
constraint is that the audio output callback (CPAL) runs on a real-time thread where
blocking, allocation, and I/O are forbidden.

We need to decide how threads communicate and what guarantees each thread provides.

## Decision

Three-thread architecture with a lock-free SPSC ring buffer:

```
[Decode Thread] --→ rtrb Ring Buffer --→ [Audio Callback Thread]
       ↑                                          ↓
  (can block,                              (REAL-TIME:
   allocate,                                no alloc,
   do I/O)                                  no locks,
                                            no I/O)
```

A separate main/TUI thread handles user input and rendering.

**Real-time rules for the audio callback thread** (non-negotiable):

1. No heap allocation (`Vec::push`, `String::new`, `Box::new`, `format!`)
2. No blocking I/O (file, network, stdout)
3. No mutex waits — lock-free only
4. No channel blocking — `try_recv()` / `try_send()` only
5. No logging — atomic flag checks only
6. Underrun: output silence (zeros), set atomic flag

**Communication**:
- Decode → Audio: `rtrb` lock-free SPSC ring buffer (~200ms of audio)
- Audio → Main: atomic flags (underrun, playback position)
- Main → Decode: `std::sync::mpsc` or tokio channel (seek, stop, next)

## Consequences

- The decode thread can safely allocate, parse metadata, and do network I/O without
  affecting audio playback
- The audio callback is simple: pop from ring buffer, apply DSP, output
- Seeking requires flushing the ring buffer (brief silence is acceptable)
- Gapless playback requires the decode thread to pre-fill the buffer with the next
  track's samples before the current track ends
- This model maps cleanly to all target platforms (native CPAL, Web AudioWorklet,
  AVAudioEngine) — only the audio output adapter changes
