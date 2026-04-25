//! 10-band graphic equalizer with presets.
//!
//! Uses biquad IIR filters (one per band) applied in series. The first band
//! is a low-shelf, the last band is a high-shelf (or bypassed near Nyquist),
//! and the middle 8 bands are peaking EQ filters.
//!
//! Filter coefficients are computed once per preset change (not per sample).
//! Processing is O(1) per sample per band — 10 multiply-accumulates total.

use super::dsp::{BiquadCoeffs, BiquadFilter};

/// Standard graphic EQ center frequencies (Hz).
pub const BAND_FREQUENCIES: [f64; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// Bandwidth in octaves for peaking EQ bands.
const BANDWIDTH_OCTAVES: f64 = 1.0;

/// A named EQ preset with 10 band gain values in dB.
#[derive(Debug, Clone)]
pub struct EqPreset {
    pub name: &'static str,
    pub gains: [f32; 10],
}

/// Built-in EQ presets.
pub const PRESETS: &[EqPreset] = &[
    EqPreset {
        name: "Flat",
        gains: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    },
    EqPreset {
        name: "Rock",
        gains: [4.0, 3.0, 0.0, -2.0, -1.0, 2.0, 4.0, 5.0, 5.0, 4.0],
    },
    EqPreset {
        name: "Pop",
        gains: [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0],
    },
    EqPreset {
        name: "Jazz",
        gains: [3.0, 2.0, 0.0, 1.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0],
    },
    EqPreset {
        name: "Classical",
        gains: [3.0, 2.0, -1.0, -1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0],
    },
    EqPreset {
        name: "Electronic",
        gains: [4.0, 3.0, 1.0, 0.0, -2.0, 1.0, 0.0, 2.0, 4.0, 4.0],
    },
    EqPreset {
        name: "Hip Hop",
        gains: [5.0, 4.0, 1.0, 2.0, -1.0, -1.0, 1.0, 0.0, 2.0, 3.0],
    },
    EqPreset {
        name: "Acoustic",
        gains: [3.0, 1.0, 0.0, 1.0, 2.0, 1.0, 2.0, 3.0, 2.0, 1.0],
    },
    EqPreset {
        name: "Bass Boost",
        gains: [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    },
    EqPreset {
        name: "Treble Boost",
        gains: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 4.0, 5.0, 6.0],
    },
    EqPreset {
        name: "Vocal",
        gains: [-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 3.0, 1.0, 0.0, -1.0],
    },
];

/// Find a preset by name (case-insensitive).
pub fn find_preset(name: &str) -> Option<&'static EqPreset> {
    PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// List all available preset names.
pub fn preset_names() -> Vec<&'static str> {
    PRESETS.iter().map(|p| p.name).collect()
}

/// The 10-band graphic equalizer.
pub struct Equalizer {
    filters: Vec<BiquadFilter>,
    channels: usize,
    sample_rate: f64,
    gains: [f32; 10],
    active_bands: usize,
}

impl Equalizer {
    /// Create a new equalizer for the given sample rate and channel count.
    /// Starts with flat (0dB) gains.
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let sr = sample_rate as f64;
        let nyquist = sr * 0.45; // Guard: skip bands too close to Nyquist

        let active_bands = BAND_FREQUENCIES.iter().filter(|&&f| f < nyquist).count();

        let gains = [0.0_f32; 10];
        let filters = Self::build_filters(sr, channels, &gains, active_bands);

        Self {
            filters,
            channels,
            sample_rate: sr,
            gains,
            active_bands,
        }
    }

    /// Set all 10 band gains at once (e.g., from a preset).
    pub fn set_gains(&mut self, gains: [f32; 10]) {
        self.gains = gains;
        self.filters = Self::build_filters(
            self.sample_rate,
            self.channels,
            &self.gains,
            self.active_bands,
        );
    }

    /// Apply a named preset.
    pub fn apply_preset(&mut self, preset: &EqPreset) {
        self.set_gains(preset.gains);
    }

    /// Get current gain values.
    #[allow(dead_code)] // Used by future TUI EQ display
    pub fn gains(&self) -> &[f32; 10] {
        &self.gains
    }

    /// Reset all filter state (call between tracks to avoid click artifacts).
    #[allow(dead_code)] // Used by future streaming decode (reset between tracks)
    pub fn reset(&mut self) {
        for f in &mut self.filters {
            f.reset();
        }
    }

    /// Process interleaved f32 samples in-place.
    /// This is called on the producer/decode side, NOT in the audio callback.
    pub fn process(&mut self, samples: &mut [f32], channels: usize) {
        if self.is_flat() {
            return; // Skip processing if all gains are 0
        }

        for (i, sample) in samples.iter_mut().enumerate() {
            let ch = i % channels;
            for filter in &mut self.filters {
                *sample = filter.process(*sample, ch);
            }
        }
    }

    /// Returns true if all gains are 0dB (flat) — allows skipping processing.
    fn is_flat(&self) -> bool {
        self.gains.iter().all(|&g| g.abs() < 0.01)
    }

    fn build_filters(
        sample_rate: f64,
        channels: usize,
        gains: &[f32; 10],
        active_bands: usize,
    ) -> Vec<BiquadFilter> {
        BAND_FREQUENCIES
            .iter()
            .enumerate()
            .take(active_bands)
            .map(|(i, &freq)| {
                let gain_db = gains[i] as f64;

                // Skip bands with ~0dB gain
                if gain_db.abs() < 0.01 {
                    return BiquadFilter::new(BiquadCoeffs::bypass(), channels);
                }

                let coeffs = if i == 0 {
                    // Low shelf for the lowest band
                    BiquadCoeffs::low_shelf(sample_rate, freq, gain_db)
                } else if i == active_bands - 1 && BAND_FREQUENCIES[i] >= 8000.0 {
                    // High shelf for the highest active band (if it's 8kHz+)
                    BiquadCoeffs::high_shelf(sample_rate, freq, gain_db)
                } else {
                    // Peaking EQ for all middle bands
                    BiquadCoeffs::peaking(sample_rate, freq, gain_db, BANDWIDTH_OCTAVES)
                };

                BiquadFilter::new(coeffs, channels)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_does_not_modify_signal() {
        let mut eq = Equalizer::new(44100, 2);
        let original: Vec<f32> = (0..200).map(|i| (i as f32) * 0.005).collect();
        let mut samples = original.clone();
        eq.process(&mut samples, 2);
        // Flat EQ should skip processing entirely
        assert_eq!(original, samples);
    }

    #[test]
    fn preset_applies_correctly() {
        let mut eq = Equalizer::new(44100, 2);
        let preset = find_preset("Rock").unwrap();
        eq.apply_preset(preset);
        assert_eq!(eq.gains()[0], 4.0); // 31Hz = +4dB
        assert_eq!(eq.gains()[3], -2.0); // 250Hz = -2dB
    }

    #[test]
    fn all_presets_exist_and_have_valid_gains() {
        for preset in PRESETS {
            assert!(!preset.name.is_empty());
            for &gain in &preset.gains {
                assert!(
                    (-12.0..=12.0).contains(&gain),
                    "Preset '{}' has out-of-range gain: {gain}",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn find_preset_is_case_insensitive() {
        assert!(find_preset("rock").is_some());
        assert!(find_preset("ROCK").is_some());
        assert!(find_preset("Rock").is_some());
        assert!(find_preset("nonexistent").is_none());
    }

    #[test]
    fn eq_handles_low_sample_rate() {
        // At 22050 Hz, Nyquist is 11025 Hz — 16kHz band should be skipped
        let eq = Equalizer::new(22050, 1);
        assert!(
            eq.active_bands < 10,
            "Should skip high bands at low sample rates"
        );
    }

    #[test]
    fn eq_processes_mono_and_stereo() {
        for channels in [1, 2] {
            let mut eq = Equalizer::new(44100, channels);
            let preset = find_preset("Bass Boost").unwrap();
            eq.apply_preset(preset);

            let mut samples: Vec<f32> = (0..4410)
                .map(|i| {
                    let t = i as f32 / (44100.0 * channels as f32);
                    (2.0 * std::f32::consts::PI * 50.0 * t).sin() * 0.5
                })
                .collect();

            eq.process(&mut samples, channels);

            // Should not produce NaN or infinity
            assert!(
                samples.iter().all(|s| s.is_finite()),
                "EQ produced non-finite values for {channels}ch"
            );
        }
    }

    #[test]
    fn reset_clears_state_between_tracks() {
        let mut eq = Equalizer::new(44100, 2);
        let preset = find_preset("Rock").unwrap();
        eq.apply_preset(preset);

        // Process some samples
        let mut buf = vec![0.5_f32; 200];
        eq.process(&mut buf, 2);

        // Reset
        eq.reset();

        // Process again — should produce same result as fresh start
        let mut eq2 = Equalizer::new(44100, 2);
        eq2.apply_preset(preset);

        let mut buf1 = vec![0.3_f32; 200];
        let mut buf2 = buf1.clone();
        eq.process(&mut buf1, 2);
        eq2.process(&mut buf2, 2);

        for (a, b) in buf1.iter().zip(buf2.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "Reset EQ should match fresh EQ: {a} vs {b}"
            );
        }
    }

    #[test]
    fn preset_names_returns_all() {
        let names = preset_names();
        assert_eq!(names.len(), PRESETS.len());
        assert!(names.contains(&"Flat"));
        assert!(names.contains(&"Rock"));
    }
}
