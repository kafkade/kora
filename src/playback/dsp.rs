//! Biquad IIR filter — Transposed Direct Form II.
//!
//! Used for the 10-band graphic EQ. Each filter is O(1) per sample with
//! two delay elements per channel. Coefficients are pre-normalized (divided
//! by a0) on creation so no per-sample division is needed.
//!
//! Reference: Robert Bristow-Johnson's Audio EQ Cookbook.

use std::f64::consts::PI;

/// Pre-normalized biquad coefficients (already divided by a0).
#[derive(Debug, Clone, Copy)]
pub struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

/// Per-channel filter state for Transposed Direct Form II.
#[derive(Debug, Clone, Copy, Default)]
struct ChannelState {
    s1: f64,
    s2: f64,
}

/// A biquad filter with per-channel state.
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    coeffs: BiquadCoeffs,
    state: Vec<ChannelState>,
}

impl BiquadCoeffs {
    /// Peaking EQ filter for mid-band frequencies.
    pub fn peaking(sample_rate: f64, freq: f64, gain_db: f64, bw_octaves: f64) -> Self {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sample_rate;

        let alpha = w0.sin() * (2.0_f64.ln() / 2.0 * bw_octaves * w0 / w0.sin()).sinh() / 2.0;

        let cos_w0 = w0.cos();

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self::normalized(b0, b1, b2, a0, a1, a2)
    }

    /// Low-shelf filter for the lowest EQ band.
    pub fn low_shelf(sample_rate: f64, freq: f64, gain_db: f64) -> Self {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * (2.0_f64).sqrt(); // S = 1.0 (slope)
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalized(b0, b1, b2, a0, a1, a2)
    }

    /// High-shelf filter for the highest EQ band.
    pub fn high_shelf(sample_rate: f64, freq: f64, gain_db: f64) -> Self {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * (2.0_f64).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalized(b0, b1, b2, a0, a1, a2)
    }

    /// Bypass (unity gain, no filtering).
    pub fn bypass() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    fn normalized(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

impl BiquadFilter {
    pub fn new(coeffs: BiquadCoeffs, channels: usize) -> Self {
        Self {
            coeffs,
            state: vec![ChannelState::default(); channels],
        }
    }

    /// Process one sample for a given channel using Transposed Direct Form II.
    /// No allocation, no branching — suitable for tight audio loops.
    #[inline(always)]
    pub fn process(&mut self, sample: f32, channel: usize) -> f32 {
        let x = sample as f64;
        let s = &mut self.state[channel];
        let c = &self.coeffs;

        let y = c.b0 * x + s.s1;
        s.s1 = c.b1 * x - c.a1 * y + s.s2;
        s.s2 = c.b2 * x - c.a2 * y;

        y as f32
    }

    /// Reset filter state (call on track boundaries to avoid clicks).
    #[allow(dead_code)] // Used by Equalizer::reset and future live EQ changes
    pub fn reset(&mut self) {
        for s in &mut self.state {
            s.s1 = 0.0;
            s.s2 = 0.0;
        }
    }

    /// Update coefficients (e.g., when EQ gain changes).
    #[allow(dead_code)] // Used by future live EQ adjustment
    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_filter_passes_signal_unchanged() {
        let mut filter = BiquadFilter::new(BiquadCoeffs::bypass(), 1);
        let input: Vec<f32> = (0..100).map(|i| (i as f32) * 0.01).collect();
        let output: Vec<f32> = input.iter().map(|&s| filter.process(s, 0)).collect();
        for (i, (a, b)) in input.iter().zip(output.iter()).enumerate() {
            assert!((a - b).abs() < 1e-6, "Sample {i}: input={a}, output={b}");
        }
    }

    #[test]
    fn peaking_filter_with_zero_gain_passes_signal() {
        let coeffs = BiquadCoeffs::peaking(44100.0, 1000.0, 0.0, 1.0);
        let mut filter = BiquadFilter::new(coeffs, 1);
        // DC signal should pass through unchanged with 0dB gain
        for _ in 0..100 {
            let out = filter.process(1.0, 0);
            // After settling, should be ~1.0
            assert!(out.is_finite());
        }
        let out = filter.process(1.0, 0);
        assert!(
            (out - 1.0).abs() < 0.01,
            "0dB peaking should pass DC: got {out}"
        );
    }

    #[test]
    fn peaking_filter_boosts_at_center_frequency() {
        let coeffs = BiquadCoeffs::peaking(44100.0, 1000.0, 12.0, 1.0);
        let mut filter = BiquadFilter::new(coeffs, 1);

        // Generate a 1kHz sine wave
        let sample_rate = 44100.0_f32;
        let freq = 1000.0_f32;
        let mut max_in: f32 = 0.0;
        let mut max_out: f32 = 0.0;

        for i in 0..4410 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
            let out = filter.process(sample, 0);
            if i > 1000 {
                // After settling
                max_in = max_in.max(sample.abs());
                max_out = max_out.max(out.abs());
            }
        }

        let gain_db = 20.0 * (max_out / max_in).log10();
        // +12dB boost should yield roughly 8-16dB measured gain
        assert!(
            gain_db > 6.0 && gain_db < 18.0,
            "Expected ~12dB boost at 1kHz, got {gain_db:.1}dB"
        );
    }

    #[test]
    fn filter_reset_clears_state() {
        let coeffs = BiquadCoeffs::peaking(44100.0, 1000.0, 12.0, 1.0);
        let mut filter = BiquadFilter::new(coeffs, 2);

        // Process some samples
        for i in 0..100 {
            filter.process((i as f32) * 0.01, 0);
            filter.process((i as f32) * 0.01, 1);
        }

        filter.reset();

        for s in &filter.state {
            assert_eq!(s.s1, 0.0);
            assert_eq!(s.s2, 0.0);
        }
    }

    #[test]
    fn low_shelf_boosts_low_frequencies() {
        let coeffs = BiquadCoeffs::low_shelf(44100.0, 100.0, 12.0);
        let mut filter = BiquadFilter::new(coeffs, 1);

        // DC (0 Hz) should be boosted
        for _ in 0..200 {
            filter.process(1.0, 0);
        }
        let dc_out = filter.process(1.0, 0);
        assert!(
            dc_out > 1.5,
            "Low shelf +12dB should boost DC: got {dc_out}"
        );
    }

    #[test]
    fn high_shelf_boosts_high_frequencies() {
        let coeffs = BiquadCoeffs::high_shelf(44100.0, 8000.0, 12.0);
        let mut filter = BiquadFilter::new(coeffs, 1);

        // Generate a ~16kHz sine wave (above the shelf frequency)
        let sample_rate = 44100.0_f32;
        let freq = 16000.0_f32;
        let mut max_in: f32 = 0.0;
        let mut max_out: f32 = 0.0;

        for i in 0..4410 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
            let out = filter.process(sample, 0);
            if i > 1000 {
                max_in = max_in.max(sample.abs());
                max_out = max_out.max(out.abs());
            }
        }

        assert!(
            max_out > max_in * 1.5,
            "High shelf should boost 16kHz: in={max_in}, out={max_out}"
        );
    }
}
