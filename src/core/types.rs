//! Shared audio types used across layers.

/// Volume represented in decibels.
#[derive(Debug, Clone, Copy)]
pub struct Volume(pub f32);

impl Volume {
    /// Convert dB to linear amplitude multiplier.
    pub fn as_linear(&self) -> f32 {
        10.0_f32.powf(self.0 / 20.0)
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self(0.0) // 0 dB = unity gain
    }
}
