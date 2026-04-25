use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, StreamConfig};
use rtrb::{Consumer, RingBuffer};

/// Play pre-decoded f32 samples through CPAL.
///
/// Uses an rtrb ring buffer: the main thread pushes samples,
/// the CPAL audio callback pulls them. The callback thread obeys
/// real-time safety rules (no alloc, no locks, no I/O).
pub fn play_audio(
    samples: &[f32],
    sample_rate: u32,
    channels: usize,
    volume: f32,
    stop: &Arc<AtomicBool>,
) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device found")?;

    tracing::info!("Audio device: {}", device.name().unwrap_or_default());

    let config = StreamConfig {
        channels: channels as u16,
        sample_rate: SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Ring buffer: ~200ms of audio
    let buffer_frames = (sample_rate as usize * channels * 200) / 1000;
    let (mut producer, consumer) = RingBuffer::new(buffer_frames);

    let _stop_playback = stop.clone();
    let _playback_done = Arc::new(AtomicBool::new(false));

    // Build the output stream — the callback is real-time safe:
    // only reads from the lock-free ring buffer, no alloc/locks/I/O.
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut consumer = unsafe {
                    // SAFETY: we ensure only one callback runs at a time (CPAL guarantee)
                    // and the consumer is only accessed from this callback.
                    std::ptr::read(&consumer as *const Consumer<f32>)
                };
                for sample in data.iter_mut() {
                    match consumer.pop() {
                        Ok(s) => *sample = s,
                        Err(_) => {
                            *sample = 0.0; // Underrun: output silence
                        }
                    }
                }
                std::mem::forget(consumer);
            },
            move |err| {
                tracing::error!("Audio stream error: {err}");
            },
            None,
        )
        .context("Failed to build audio output stream")?;

    stream.play().context("Failed to start audio playback")?;

    // Push samples into the ring buffer from the main thread.
    // Apply volume scaling here (before the real-time callback).
    let mut pos = 0;
    while pos < samples.len() && !stop.load(Ordering::Relaxed) {
        let slots = producer.slots();
        if slots == 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        let chunk_end = (pos + slots).min(samples.len());
        for &s in &samples[pos..chunk_end] {
            let _ = producer.push(s * volume);
        }
        pos = chunk_end;
    }

    // Wait for the ring buffer to drain (finish playing)
    while producer.slots() < buffer_frames && !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Small grace period for the last samples to reach the speaker
    if !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    drop(stream);
    Ok(())
}
