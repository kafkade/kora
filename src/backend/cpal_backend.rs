use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use anyhow::{Context, Result};
use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::RingBuffer;

use crate::playback::fft::SpectrumData;

/// Information about an available audio output device.
#[derive(Debug)]
#[allow(dead_code)] // Used in tests and future device selection UI
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

/// List all available audio output devices.
#[allow(dead_code)] // Used in tests and future device selection UI
pub fn list_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.description().ok())
        .map(|d| d.name().to_string())
        .unwrap_or_default();

    let devices: Vec<AudioDevice> = host
        .output_devices()
        .context("Failed to enumerate audio output devices")?
        .filter_map(|d| {
            let desc = d.description().ok()?;
            let name = desc.name().to_string();
            Some(AudioDevice {
                is_default: name == default_name,
                name,
            })
        })
        .collect();

    Ok(devices)
}

/// Play pre-decoded f32 samples through CPAL.
///
/// Uses an rtrb ring buffer: the main thread pushes samples,
/// the CPAL audio callback pulls them. The callback thread obeys
/// real-time safety rules (no alloc, no locks, no I/O).
#[allow(dead_code)] // Used by headless/non-TUI mode (future)
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
        .context("No audio output device found. Check your system audio settings.")?;

    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown".into());
    tracing::info!("Audio device: {device_name}");

    let config = StreamConfig {
        channels: channels as u16,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    // Ring buffer: ~200ms of audio
    let buffer_frames = (sample_rate as usize * channels * 200) / 1000;
    let (mut producer, mut consumer) = RingBuffer::new(buffer_frames);

    // Build the output stream. The consumer is moved into the callback
    // and owned exclusively by it — no unsafe needed.
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // REAL-TIME SAFE: only pop from lock-free ring buffer
                for sample in data.iter_mut() {
                    match consumer.pop() {
                        Ok(s) => *sample = s,
                        Err(_) => *sample = 0.0, // Underrun: silence
                    }
                }
            },
            move |err| {
                eprintln!("Audio stream error: {err}");
            },
            None,
        )
        .with_context(|| {
            format!(
                "Failed to build audio stream on '{device_name}' \
                 ({}Hz, {}ch). The device may not support this format.",
                sample_rate, channels
            )
        })?;

    stream
        .play()
        .with_context(|| format!("Failed to start playback on '{device_name}'"))?;

    // Push samples into the ring buffer from the main thread.
    // Volume scaling happens here (producer side), not in the callback.
    let mut pos = 0;
    while pos < samples.len() && !stop.load(Ordering::Relaxed) {
        let slots = producer.slots();
        if slots == 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        let chunk_end = (pos + slots).min(samples.len());
        let chunk = &samples[pos..chunk_end];
        for &s in chunk {
            let _ = producer.push(s * volume);
        }
        pos = chunk_end;
    }

    // Wait for the ring buffer to drain (finish playing)
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if producer.slots() >= buffer_frames {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Grace period for the last samples to reach the speaker
    if !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    drop(stream);
    Ok(())
}

/// Play audio with position tracking and pause/resume support.
///
/// `volume` is a shared atomic storing the f32 linear gain as raw bits
/// (`f32::to_bits()`). The producer reads it on each chunk, so volume
/// changes from the Player take effect immediately — no track restart needed.
#[allow(clippy::too_many_arguments)]
pub fn play_audio_with_position(
    samples: &[f32],
    sample_rate: u32,
    channels: usize,
    volume: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    samples_played: &AtomicU64,
    spectrum: &Arc<SpectrumData>,
) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device found.")?;

    let config = StreamConfig {
        channels: channels as u16,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let buffer_frames = (sample_rate as usize * channels * 200) / 1000;
    let (mut producer, mut consumer) = RingBuffer::new(buffer_frames);

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    match consumer.pop() {
                        Ok(s) => *sample = s,
                        Err(_) => *sample = 0.0,
                    }
                }
            },
            move |err| {
                eprintln!("Audio stream error: {err}");
            },
            None,
        )
        .context("Failed to build audio stream")?;

    stream.play().context("Failed to start playback")?;

    let mut pos = 0;
    let mut spectrum_counter: usize = 0;
    const SPECTRUM_INTERVAL: usize = 2048;
    while pos < samples.len() && !stop.load(Ordering::Relaxed) {
        // When paused, sleep without pushing — the callback drains to silence
        if pause.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let slots = producer.slots();
        if slots == 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        // Read current volume once per chunk for live volume control
        let vol = f32::from_bits(volume.load(Ordering::Relaxed));

        let chunk_end = (pos + slots).min(samples.len());
        let chunk = &samples[pos..chunk_end];
        for &s in chunk {
            let _ = producer.push(s * vol);
        }
        let chunk_len = chunk_end - pos;
        pos = chunk_end;
        samples_played.store(pos as u64, Ordering::Relaxed);

        // Update spectrum data periodically (on the producer thread — safe to allocate)
        spectrum_counter += chunk_len;
        if spectrum_counter >= SPECTRUM_INTERVAL {
            let start = pos.saturating_sub(SPECTRUM_INTERVAL);
            spectrum.update(&samples[start..pos], channels, sample_rate);
            spectrum_counter = 0;
        }
    }

    // Wait for drain
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if producer.slots() >= buffer_frames {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    if !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    drop(stream);
    Ok(())
}

/// Play audio with gapless transition to a next track.
///
/// After the current track's samples finish, continues pushing `next_samples`
/// into the same ring buffer and CPAL stream without tearing down the audio
/// pipeline. The caller must ensure `next_samples` has the same sample rate
/// and channel count. Sets `played_next` to `true` if `next_samples` were played.
#[allow(dead_code)] // Available for true zero-gap playback in future
#[allow(clippy::too_many_arguments)]
pub fn play_audio_gapless(
    samples: &[f32],
    next_samples: Option<&[f32]>,
    sample_rate: u32,
    channels: usize,
    volume: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    samples_played: &AtomicU64,
    spectrum: &Arc<SpectrumData>,
    played_next: &AtomicBool,
) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device found.")?;

    let config = StreamConfig {
        channels: channels as u16,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let buffer_frames = (sample_rate as usize * channels * 200) / 1000;
    let (mut producer, mut consumer) = RingBuffer::new(buffer_frames);

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    match consumer.pop() {
                        Ok(s) => *sample = s,
                        Err(_) => *sample = 0.0,
                    }
                }
            },
            move |err| {
                eprintln!("Audio stream error: {err}");
            },
            None,
        )
        .context("Failed to build audio stream")?;

    stream.play().context("Failed to start playback")?;

    // Push current track samples
    let mut pos = 0;
    let mut spectrum_counter: usize = 0;
    const SPECTRUM_INTERVAL: usize = 2048;
    while pos < samples.len() && !stop.load(Ordering::Relaxed) {
        if pause.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let slots = producer.slots();
        if slots == 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let vol = f32::from_bits(volume.load(Ordering::Relaxed));
        let chunk_end = (pos + slots).min(samples.len());
        let chunk = &samples[pos..chunk_end];
        for &s in chunk {
            let _ = producer.push(s * vol);
        }
        let chunk_len = chunk_end - pos;
        pos = chunk_end;
        samples_played.store(pos as u64, Ordering::Relaxed);

        spectrum_counter += chunk_len;
        if spectrum_counter >= SPECTRUM_INTERVAL {
            let start = pos.saturating_sub(SPECTRUM_INTERVAL);
            spectrum.update(&samples[start..pos], channels, sample_rate);
            spectrum_counter = 0;
        }
    }

    // Gapless: continue pushing next track samples without stopping the stream
    if let Some(next) = next_samples
        && !stop.load(Ordering::Relaxed)
        && !next.is_empty()
    {
        samples_played.store(0, Ordering::Relaxed);
        played_next.store(true, Ordering::Relaxed);
        let mut next_pos = 0;
        spectrum_counter = 0;

        while next_pos < next.len() && !stop.load(Ordering::Relaxed) {
            if pause.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }

            let slots = producer.slots();
            if slots == 0 {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }

            let vol = f32::from_bits(volume.load(Ordering::Relaxed));
            let chunk_end = (next_pos + slots).min(next.len());
            let chunk = &next[next_pos..chunk_end];
            for &s in chunk {
                let _ = producer.push(s * vol);
            }
            let chunk_len = chunk_end - next_pos;
            next_pos = chunk_end;
            samples_played.store(next_pos as u64, Ordering::Relaxed);

            spectrum_counter += chunk_len;
            if spectrum_counter >= SPECTRUM_INTERVAL {
                let start = next_pos.saturating_sub(SPECTRUM_INTERVAL);
                spectrum.update(&next[start..next_pos], channels, sample_rate);
                spectrum_counter = 0;
            }
        }
    }

    // Wait for drain
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if producer.slots() >= buffer_frames {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    if !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    drop(stream);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_devices_returns_without_panic() {
        // This test validates that device enumeration doesn't panic.
        // On CI (no audio device), it may return an empty list — that's OK.
        let result = list_devices();
        match result {
            Ok(devices) => {
                for d in &devices {
                    assert!(!d.name.is_empty(), "Device name should not be empty");
                }
                // If there's a default device, exactly one should be marked default
                if !devices.is_empty() {
                    let default_count = devices.iter().filter(|d| d.is_default).count();
                    assert!(
                        default_count <= 1,
                        "At most one device should be the default"
                    );
                }
            }
            Err(_) => {
                // On headless CI, device enumeration may fail — acceptable
            }
        }
    }

    #[test]
    fn play_audio_with_empty_samples_returns_ok() {
        // Playing zero samples should complete immediately without crash
        let stop = Arc::new(AtomicBool::new(false));
        // On CI without audio device, this will fail at device selection — that's OK
        let result = play_audio(&[], 44100, 2, 1.0, &stop);
        match result {
            Ok(()) => {} // Completed fine
            Err(e) => {
                let msg = format!("{e:#}");
                assert!(
                    msg.contains("No audio output device")
                        || msg.contains("Failed to build")
                        || msg.contains("Failed to start"),
                    "Unexpected error: {msg}"
                );
            }
        }
    }
}
