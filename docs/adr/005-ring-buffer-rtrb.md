# ADR-005: Ring Buffer Selection (rtrb)

## Status

Accepted

## Context

The audio pipeline needs a lock-free, single-producer single-consumer (SPSC) ring buffer
to pass decoded audio samples from the decode thread to the CPAL audio callback thread.
The callback thread is real-time — the buffer must be lock-free and allocation-free in
steady state.

## Decision

Use **rtrb** (Real-Time Ring Buffer) as the SPSC ring buffer implementation.

## Evaluation

| Crate | Lock-free | SPSC | Allocation-free | Audio-focused | Maintained |
|-------|-----------|------|-----------------|---------------|------------|
| `rtrb` | ✅ | ✅ | ✅ (after init) | ✅ (designed for audio) | ✅ |
| `ringbuf` | ✅ | ✅ | ✅ | ❌ (general purpose) | ✅ |
| `crossbeam-queue` | ✅ | ❌ (MPMC) | ✅ | ❌ | ✅ |
| `std::sync::mpsc` | ❌ (locks) | N/A | ❌ | ❌ | ✅ |

## Key Properties

- **Lock-free**: `push()` and `pop()` never block or acquire locks
- **Bounded**: fixed capacity allocated at creation, no runtime allocation
- **Wait-free for single producer/consumer**: exactly one thread pushes, one pops
- **Cache-friendly**: contiguous memory layout, minimal false sharing
- **`slots()` method**: producer can check available space without blocking

## Consequences

- The decode thread pushes samples via `producer.push()`, sleeping briefly if the buffer
  is full (backpressure — this is safe since the decode thread is not real-time)
- The audio callback pops samples via `consumer.pop()`, outputting silence on underrun
- Buffer size (~200ms at 44.1kHz stereo ≈ 17,640 samples) provides enough runway to
  absorb decode jitter without audible gaps
- The `Consumer` must be moved into the CPAL callback closure — do NOT use `unsafe`
  `ptr::read` tricks; Rust ownership handles this correctly
