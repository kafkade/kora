//! ReplayGain volume normalization — read RG tags and apply gain to decoded audio.
//!
//! ReplayGain tags are stored in ID3v2 TXXX frames, Vorbis Comments, or MP4
//! freeform atoms. This module reads them with `lofty`, parses the gain strings
//! (e.g. "−3.45 dB"), and applies the resulting linear gain to f32 sample buffers.

use std::path::Path;

use lofty::prelude::*;
use lofty::tag::ItemKey;

/// ReplayGain metadata extracted from an audio file.
#[derive(Debug, Clone, Default)]
pub struct ReplayGainInfo {
    pub track_gain_db: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Which ReplayGain value to apply during playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayGainMode {
    Off,
    #[default]
    Track,
    Album,
}

impl ReplayGainMode {
    /// Parse a config string ("off", "track", "album") into a mode.
    pub fn from_str_config(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Self::Off,
            "album" => Self::Album,
            _ => Self::Track,
        }
    }
}

impl std::fmt::Display for ReplayGainMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::Track => write!(f, "Track"),
            Self::Album => write!(f, "Album"),
        }
    }
}

/// Read ReplayGain tags from an audio file. Returns default (all `None`) on any error.
pub fn read_replaygain(path: &Path) -> ReplayGainInfo {
    let tagged_file = match lofty::read_from_path(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!("Could not read tags from {}: {e}", path.display());
            return ReplayGainInfo::default();
        }
    };

    let mut info = ReplayGainInfo::default();

    for tag in tagged_file.tags() {
        if info.track_gain_db.is_none()
            && let Some(s) = tag.get_string(ItemKey::ReplayGainTrackGain)
        {
            info.track_gain_db = parse_gain_value(s);
        }
        if info.album_gain_db.is_none()
            && let Some(s) = tag.get_string(ItemKey::ReplayGainAlbumGain)
        {
            info.album_gain_db = parse_gain_value(s);
        }
        if info.track_peak.is_none()
            && let Some(s) = tag.get_string(ItemKey::ReplayGainTrackPeak)
        {
            info.track_peak = parse_peak_value(s);
        }
        if info.album_peak.is_none()
            && let Some(s) = tag.get_string(ItemKey::ReplayGainAlbumPeak)
        {
            info.album_peak = parse_peak_value(s);
        }
    }

    info
}

/// Determine the dB gain to apply based on mode and available RG info.
///
/// - `Track` prefers `track_gain`, falls back to `album_gain`.
/// - `Album` prefers `album_gain`, falls back to `track_gain`.
/// - `Off` always returns `None`.
pub fn gain_to_apply(info: &ReplayGainInfo, mode: ReplayGainMode) -> Option<f32> {
    match mode {
        ReplayGainMode::Off => None,
        ReplayGainMode::Track => info.track_gain_db.or(info.album_gain_db),
        ReplayGainMode::Album => info.album_gain_db.or(info.track_gain_db),
    }
}

/// Apply a dB gain to all samples in-place, clamping to \[-1.0, 1.0\].
pub fn apply_replaygain(samples: &mut [f32], gain_db: f32) {
    let linear = 10.0_f32.powf(gain_db / 20.0);
    for s in samples.iter_mut() {
        *s = (*s * linear).clamp(-1.0, 1.0);
    }
}

/// Parse a ReplayGain gain string like "-3.45 dB" or "−3.45 dB" into f32.
///
/// Handles both ASCII minus (`-`, U+002D) and Unicode minus sign (`−`, U+2212).
fn parse_gain_value(s: &str) -> Option<f32> {
    // Strip optional " dB" / " db" suffix (case-insensitive)
    let s = s.trim();
    let s = s
        .strip_suffix("dB")
        .or_else(|| s.strip_suffix("db"))
        .or_else(|| s.strip_suffix("DB"))
        .unwrap_or(s)
        .trim();

    // Replace Unicode minus sign (U+2212) with ASCII minus
    let s = s.replace('\u{2212}', "-");

    s.parse::<f32>().ok()
}

/// Parse a peak value (plain float, no "dB" suffix).
fn parse_peak_value(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // -- parse_gain_value --

    #[test]
    fn parse_negative_with_db_suffix() {
        let v = parse_gain_value("-3.45 dB");
        assert!((v.unwrap() - (-3.45)).abs() < 0.001);
    }

    #[test]
    fn parse_unicode_minus_with_db_suffix() {
        // U+2212 MINUS SIGN
        let v = parse_gain_value("\u{2212}3.45 dB");
        assert!((v.unwrap() - (-3.45)).abs() < 0.001);
    }

    #[test]
    fn parse_without_suffix() {
        let v = parse_gain_value("-3.45");
        assert!((v.unwrap() - (-3.45)).abs() < 0.001);
    }

    #[test]
    fn parse_positive_with_db_suffix() {
        let v = parse_gain_value("+2.0 dB");
        assert!((v.unwrap() - 2.0).abs() < 0.001);
    }

    #[test]
    fn parse_zero() {
        let v = parse_gain_value("0.0 dB");
        assert!((v.unwrap()).abs() < 0.001);
    }

    #[test]
    fn parse_garbage_returns_none() {
        assert!(parse_gain_value("not a number").is_none());
        assert!(parse_gain_value("").is_none());
        assert!(parse_gain_value("abc dB").is_none());
    }

    // -- apply_replaygain --

    #[test]
    fn zero_db_no_change() {
        let mut samples = vec![0.5, -0.5, 0.0, 1.0, -1.0];
        let original = samples.clone();
        apply_replaygain(&mut samples, 0.0);
        for (a, b) in samples.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-6, "0dB should not change samples");
        }
    }

    #[test]
    fn positive_6db_roughly_doubles() {
        // +6.02 dB ≈ 2× linear, but we use +6 dB for simplicity
        let mut samples = vec![0.25, -0.25];
        apply_replaygain(&mut samples, 6.0);
        // 10^(6/20) ≈ 1.9953
        let expected = 0.25 * 10.0_f32.powf(6.0 / 20.0);
        assert!(
            (samples[0] - expected).abs() < 0.01,
            "expected ~{expected}, got {}",
            samples[0]
        );
        assert!(
            (samples[1] - (-expected)).abs() < 0.01,
            "expected ~{}, got {}",
            -expected,
            samples[1]
        );
    }

    #[test]
    fn clamps_to_valid_range() {
        let mut samples = vec![0.9, -0.9];
        // +20 dB = 10× gain → 9.0, should clamp to 1.0
        apply_replaygain(&mut samples, 20.0);
        assert_eq!(samples[0], 1.0);
        assert_eq!(samples[1], -1.0);
    }

    // -- gain_to_apply --

    #[test]
    fn track_mode_prefers_track() {
        let info = ReplayGainInfo {
            track_gain_db: Some(-3.0),
            album_gain_db: Some(-5.0),
            ..Default::default()
        };
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Track), Some(-3.0));
    }

    #[test]
    fn track_mode_falls_back_to_album() {
        let info = ReplayGainInfo {
            track_gain_db: None,
            album_gain_db: Some(-5.0),
            ..Default::default()
        };
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Track), Some(-5.0));
    }

    #[test]
    fn album_mode_prefers_album() {
        let info = ReplayGainInfo {
            track_gain_db: Some(-3.0),
            album_gain_db: Some(-5.0),
            ..Default::default()
        };
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Album), Some(-5.0));
    }

    #[test]
    fn album_mode_falls_back_to_track() {
        let info = ReplayGainInfo {
            track_gain_db: Some(-3.0),
            album_gain_db: None,
            ..Default::default()
        };
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Album), Some(-3.0));
    }

    #[test]
    fn off_mode_returns_none() {
        let info = ReplayGainInfo {
            track_gain_db: Some(-3.0),
            album_gain_db: Some(-5.0),
            ..Default::default()
        };
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Off), None);
    }

    #[test]
    fn no_tags_returns_none() {
        let info = ReplayGainInfo::default();
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Track), None);
        assert_eq!(gain_to_apply(&info, ReplayGainMode::Album), None);
    }

    // -- read_replaygain --

    #[test]
    fn read_nonexistent_file_returns_default() {
        let info = read_replaygain(Path::new("nonexistent_file_that_does_not_exist.mp3"));
        assert!(info.track_gain_db.is_none());
        assert!(info.album_gain_db.is_none());
        assert!(info.track_peak.is_none());
        assert!(info.album_peak.is_none());
    }

    // -- ReplayGainMode --

    #[test]
    fn mode_from_str_config() {
        assert_eq!(ReplayGainMode::from_str_config("off"), ReplayGainMode::Off);
        assert_eq!(
            ReplayGainMode::from_str_config("track"),
            ReplayGainMode::Track
        );
        assert_eq!(
            ReplayGainMode::from_str_config("album"),
            ReplayGainMode::Album
        );
        assert_eq!(
            ReplayGainMode::from_str_config("TRACK"),
            ReplayGainMode::Track
        );
        assert_eq!(ReplayGainMode::from_str_config("OFF"), ReplayGainMode::Off);
        // unknown defaults to Track
        assert_eq!(
            ReplayGainMode::from_str_config("garbage"),
            ReplayGainMode::Track
        );
    }

    #[test]
    fn mode_display() {
        assert_eq!(ReplayGainMode::Off.to_string(), "Off");
        assert_eq!(ReplayGainMode::Track.to_string(), "Track");
        assert_eq!(ReplayGainMode::Album.to_string(), "Album");
    }
}
