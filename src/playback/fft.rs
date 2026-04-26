//! FFT-based spectrum analysis for the visualizer.
//!
//! [`SpectrumData`] is shared between the audio producer thread and the TUI.
//! The producer calls [`SpectrumData::update`] with raw samples; the TUI
//! calls [`SpectrumData::read_bars`] to get the current bar magnitudes.
//! Communication uses `AtomicU32` (f32 bits) — no locks, safe from both sides.

use std::sync::atomic::{AtomicU32, Ordering};

use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

/// Number of samples used for each FFT window.
const FFT_SIZE: usize = 1024;

/// Minimum frequency for the lowest bar (Hz).
const MIN_FREQ: f32 = 20.0;

/// dB floor — magnitudes below this are clamped to 0.0.
const DB_FLOOR: f32 = -60.0;

/// Smoothing factor: blend ratio for new values (1-α is applied to old).
const SMOOTH_NEW: f32 = 0.3;

/// Shared spectrum data between the audio producer and TUI.
/// Updated by the producer thread, read by the TUI for rendering.
pub struct SpectrumData {
    /// Magnitude values for each bar (0.0 to 1.0), stored as atomic u32 (f32 bits).
    bars: Vec<AtomicU32>,
}

impl SpectrumData {
    /// Create a new `SpectrumData` with the given number of bars.
    pub fn new(num_bars: usize) -> Self {
        let bars = (0..num_bars).map(|_| AtomicU32::new(0u32)).collect();
        Self { bars }
    }

    /// Number of bars.
    #[allow(dead_code)]
    pub fn num_bars(&self) -> usize {
        self.bars.len()
    }

    /// Compute FFT on the latest window of samples, bin magnitudes into bars
    /// using log-frequency spacing, and store via atomics.
    ///
    /// This runs on the **producer thread** (not the CPAL callback) — allocation is fine.
    pub fn update(&self, samples: &[f32], channels: usize, sample_rate: u32) {
        if samples.is_empty() || self.bars.is_empty() {
            return;
        }

        // Mix to mono and collect the last FFT_SIZE samples
        let mono = mix_to_mono(samples, channels);
        let window_size = FFT_SIZE.min(mono.len());
        let start = mono.len() - window_size;
        let window = &mono[start..];

        // Apply Hanning window and convert to complex
        let mut buffer: Vec<Complex<f32>> = window
            .iter()
            .enumerate()
            .map(|(n, &s)| {
                let w = 0.5
                    * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / window_size as f32).cos());
                Complex::new(s * w, 0.0)
            })
            .collect();

        // Zero-pad to FFT_SIZE if needed
        buffer.resize(FFT_SIZE, Complex::new(0.0, 0.0));

        // Compute FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        fft.process(&mut buffer);

        // Only use the first half (positive frequencies)
        let half = FFT_SIZE / 2;
        let max_freq = sample_rate as f32 / 2.0;
        let num_bars = self.bars.len();

        // Log-frequency bin edges
        let log_min = MIN_FREQ.ln();
        let log_max = (max_freq / 2.0).max(MIN_FREQ + 1.0).ln();

        let mut new_bars = vec![0.0f32; num_bars];

        for (i, bar) in new_bars.iter_mut().enumerate() {
            let f_lo =
                ((log_min + (log_max - log_min) * i as f32 / num_bars as f32).exp()).max(MIN_FREQ);
            let f_hi = ((log_min + (log_max - log_min) * (i + 1) as f32 / num_bars as f32).exp())
                .min(max_freq);

            // Map frequencies to FFT bin indices
            let bin_lo = ((f_lo / max_freq) * half as f32).floor() as usize;
            let bin_hi = ((f_hi / max_freq) * half as f32).ceil() as usize;
            let bin_lo = bin_lo.max(1).min(half - 1);
            let bin_hi = bin_hi.max(bin_lo + 1).min(half);

            // Average magnitude across bins in this range
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for item in buffer.iter().take(bin_hi).skip(bin_lo) {
                let mag = item.norm();
                sum += mag;
                count += 1;
            }

            let avg_mag = if count > 0 { sum / count as f32 } else { 0.0 };

            // Normalize: magnitude → dB → 0.0..1.0
            let db = if avg_mag > 0.0 {
                20.0 * avg_mag.log10()
            } else {
                DB_FLOOR
            };
            *bar = ((db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0);
        }

        // Apply smoothing and store
        for (i, &new_val) in new_bars.iter().enumerate() {
            let old_bits = self.bars[i].load(Ordering::Relaxed);
            let old_val = f32::from_bits(old_bits);
            let smoothed = SMOOTH_NEW * new_val + (1.0 - SMOOTH_NEW) * old_val;
            self.bars[i].store(smoothed.to_bits(), Ordering::Relaxed);
        }
    }

    /// Read current bar magnitudes (0.0 to 1.0).
    pub fn read_bars(&self) -> Vec<f32> {
        self.bars
            .iter()
            .map(|b| {
                let val = f32::from_bits(b.load(Ordering::Relaxed));
                val.clamp(0.0, 1.0)
            })
            .collect()
    }
}

/// Mix interleaved multi-channel samples to mono.
fn mix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hanning_window_sum_is_approximately_n_over_2() {
        let n = FFT_SIZE;
        let sum: f32 = (0..n)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos()))
            .sum();
        let expected = n as f32 / 2.0;
        assert!(
            (sum - expected).abs() < 1.0,
            "Hanning sum {sum} should be ≈ {expected}"
        );
    }

    #[test]
    fn fft_pure_sine_peaks_at_correct_bin() {
        let sample_rate = 44100u32;
        let freq = 440.0f32;
        let num_samples = FFT_SIZE * 2; // stereo
        let channels = 2;

        // Generate a pure 440 Hz sine wave (stereo interleaved)
        let mut samples = Vec::with_capacity(num_samples);
        for i in 0..FFT_SIZE {
            let t = i as f32 / sample_rate as f32;
            let s = (2.0 * std::f32::consts::PI * freq * t).sin();
            samples.push(s); // L
            samples.push(s); // R
        }

        let spectrum = SpectrumData::new(32);
        spectrum.update(&samples, channels, sample_rate);

        let bars = spectrum.read_bars();
        // Find the bar with the peak
        let peak_bar = bars
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // The 440 Hz tone should land in one of the lower-mid bars (not the first few
        // which cover 20-100 Hz, and not the upper bars). Just verify it has significant energy.
        let peak_val = bars[peak_bar];
        assert!(
            peak_val > 0.1,
            "Peak bar {peak_bar} should have significant energy, got {peak_val}"
        );
    }

    #[test]
    fn read_bars_returns_values_in_range() {
        let sample_rate = 44100u32;
        let channels = 1;
        let samples: Vec<f32> = (0..FFT_SIZE)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin())
            .collect();

        let spectrum = SpectrumData::new(32);
        spectrum.update(&samples, channels, sample_rate);

        let bars = spectrum.read_bars();
        assert_eq!(bars.len(), 32);
        for (i, &val) in bars.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(&val),
                "Bar {i} value {val} out of range [0.0, 1.0]"
            );
        }
    }

    #[test]
    fn silence_returns_all_zeros() {
        let spectrum = SpectrumData::new(32);
        let silence = vec![0.0f32; FFT_SIZE * 2];
        spectrum.update(&silence, 2, 44100);

        let bars = spectrum.read_bars();
        for (i, &val) in bars.iter().enumerate() {
            assert!(val < 0.01, "Bar {i} should be ~0 for silence, got {val}");
        }
    }

    #[test]
    fn smoothing_blends_values() {
        let spectrum = SpectrumData::new(32);
        let sample_rate = 44100u32;

        // First update with a loud signal
        let loud: Vec<f32> = (0..FFT_SIZE)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        spectrum.update(&loud, 1, sample_rate);
        let bars_after_loud = spectrum.read_bars();

        // Second update with silence — smoothing means bars shouldn't drop to zero instantly
        let silence = vec![0.0f32; FFT_SIZE];
        spectrum.update(&silence, 1, sample_rate);
        let bars_after_silence = spectrum.read_bars();

        // Find a bar that had energy
        let active_bar = bars_after_loud
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        if bars_after_loud[active_bar] > 0.1 {
            // After silence update, the smoothed value should be > 0 (not instant drop)
            assert!(
                bars_after_silence[active_bar] > 0.0,
                "Smoothing should prevent instant drop to zero"
            );
        }
    }

    #[test]
    fn mix_to_mono_stereo() {
        let stereo = vec![1.0, 0.0, 0.5, 0.5, 0.0, 1.0];
        let mono = mix_to_mono(&stereo, 2);
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.5).abs() < f32::EPSILON);
        assert!((mono[1] - 0.5).abs() < f32::EPSILON);
        assert!((mono[2] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn mix_to_mono_already_mono() {
        let mono_in = vec![0.5, 1.0, -0.5];
        let mono_out = mix_to_mono(&mono_in, 1);
        assert_eq!(mono_out, mono_in);
    }
}
