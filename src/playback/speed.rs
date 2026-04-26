//! Playback speed adjustment via linear interpolation resampling.
//!
//! For speed > 1.0, frames are decimated (fewer output frames).
//! For speed < 1.0, frames are interpolated (more output frames).
//! Speed 1.0 passes through unchanged.

/// Minimum allowed playback speed.
pub const MIN_SPEED: f32 = 0.25;
/// Maximum allowed playback speed.
pub const MAX_SPEED: f32 = 3.0;
/// Step size for speed adjustments.
pub const SPEED_STEP: f32 = 0.25;
/// Default playback speed (normal).
pub const DEFAULT_SPEED: f32 = 1.0;

/// Resample `samples` (interleaved, `channels` channels per frame) to change
/// playback speed. Returns a new buffer with the adjusted number of frames.
///
/// Uses linear interpolation between adjacent frames for smooth output.
pub fn apply_speed(samples: &[f32], channels: usize, speed: f32) -> Vec<f32> {
    if channels == 0 || samples.is_empty() {
        return samples.to_vec();
    }
    if (speed - 1.0).abs() < 0.01 {
        return samples.to_vec();
    }

    let num_frames = samples.len() / channels;
    let new_num_frames = (num_frames as f64 / speed as f64) as usize;
    if new_num_frames == 0 {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(new_num_frames * channels);

    for i in 0..new_num_frames {
        let src_pos = i as f64 * speed as f64;
        let src_idx = src_pos as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        for ch in 0..channels {
            let idx0 = src_idx * channels + ch;
            let idx1 = ((src_idx + 1).min(num_frames - 1)) * channels + ch;
            let s0 = samples.get(idx0).copied().unwrap_or(0.0);
            let s1 = samples.get(idx1).copied().unwrap_or(0.0);
            output.push(s0 + (s1 - s0) * frac);
        }
    }

    output
}

/// Clamp a speed value to the allowed range.
pub fn clamp_speed(speed: f32) -> f32 {
    speed.clamp(MIN_SPEED, MAX_SPEED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_1_0_returns_same_length() {
        let samples: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let result = apply_speed(&samples, 2, 1.0);
        assert_eq!(result.len(), samples.len());
        assert_eq!(result, samples);
    }

    #[test]
    fn speed_2_0_returns_approximately_half_frames() {
        let channels = 2;
        let num_frames = 100;
        let samples: Vec<f32> = (0..(num_frames * channels))
            .map(|i| i as f32 * 0.001)
            .collect();
        let result = apply_speed(&samples, channels, 2.0);
        let result_frames = result.len() / channels;
        assert_eq!(result_frames, num_frames / 2);
    }

    #[test]
    fn speed_0_5_returns_approximately_double_frames() {
        let channels = 2;
        let num_frames = 100;
        let samples: Vec<f32> = (0..(num_frames * channels))
            .map(|i| i as f32 * 0.001)
            .collect();
        let result = apply_speed(&samples, channels, 0.5);
        let result_frames = result.len() / channels;
        assert_eq!(result_frames, num_frames * 2);
    }

    #[test]
    fn linear_interpolation_produces_smooth_values() {
        // Mono signal: 0.0, 1.0, 2.0, 3.0
        let samples = vec![0.0_f32, 1.0, 2.0, 3.0];
        let result = apply_speed(&samples, 1, 0.5);
        // At 0.5x speed we get ~8 frames from 4. Check monotonically increasing.
        for i in 1..result.len() {
            assert!(
                result[i] >= result[i - 1],
                "Non-monotonic at index {i}: {} < {}",
                result[i],
                result[i - 1]
            );
        }
    }

    #[test]
    fn stereo_channels_stay_aligned() {
        // 4 stereo frames: (L0,R0), (L1,R1), (L2,R2), (L3,R3)
        let samples = vec![
            0.0, 100.0, // frame 0
            1.0, 101.0, // frame 1
            2.0, 102.0, // frame 2
            3.0, 103.0, // frame 3
        ];
        let result = apply_speed(&samples, 2, 2.0);
        // 2x speed: ~2 output frames
        assert_eq!(
            result.len() % 2,
            0,
            "output must have even number of samples"
        );
        // Each frame's R channel should be ~100 more than L channel
        for frame in result.chunks(2) {
            let diff = frame[1] - frame[0];
            assert!(
                (diff - 100.0).abs() < 1.0,
                "Channel misalignment: L={}, R={}",
                frame[0],
                frame[1]
            );
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = apply_speed(&[], 2, 1.5);
        assert!(result.is_empty());
    }

    #[test]
    fn clamp_speed_enforces_bounds() {
        assert_eq!(clamp_speed(0.1), MIN_SPEED);
        assert_eq!(clamp_speed(5.0), MAX_SPEED);
        assert_eq!(clamp_speed(1.5), 1.5);
    }

    #[test]
    fn extreme_speed_produces_valid_output() {
        let samples: Vec<f32> = (0..200).map(|i| i as f32 * 0.005).collect();
        let fast = apply_speed(&samples, 2, 3.0);
        assert!(!fast.is_empty());
        let slow = apply_speed(&samples, 2, 0.25);
        assert!(slow.len() > samples.len());
    }
}
