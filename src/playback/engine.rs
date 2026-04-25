use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};

use crate::core::track::{Track, TrackSource};
use crate::core::types::Volume;
use crate::playback::eq::{self, Equalizer};

/// Play a list of tracks sequentially with optional EQ (non-TUI mode).
#[allow(dead_code)] // Used by headless/non-TUI mode (future)
pub fn play_tracks(tracks: &[Track], volume_db: f32, eq_preset: Option<&str>) -> Result<()> {
    let volume = Volume(volume_db);

    // Resolve EQ preset
    let preset = match eq_preset {
        Some(name) => {
            let p = eq::find_preset(name).with_context(|| {
                format!(
                    "Unknown EQ preset '{}'. Available: {}",
                    name,
                    eq::preset_names().join(", ")
                )
            })?;
            tracing::info!("EQ preset: {}", p.name);
            Some(p)
        }
        None => None,
    };

    for (i, track) in tracks.iter().enumerate() {
        let path = match &track.source {
            TrackSource::File(p) => p,
            TrackSource::Url(_) => {
                tracing::warn!("URL playback not yet supported, skipping");
                continue;
            }
        };

        println!("[{}/{}] {}", i + 1, tracks.len(), track.display_name());

        let decoded = super::decoder::decode_file(path)
            .with_context(|| format!("Failed to decode {}", path.display()))?;

        // Apply EQ to decoded samples
        let samples = if let Some(p) = preset {
            let mut eq = Equalizer::new(decoded.sample_rate, decoded.channels);
            eq.apply_preset(p);
            let mut buf = decoded.samples;
            eq.process(&mut buf, decoded.channels);
            buf
        } else {
            decoded.samples
        };

        let stop = Arc::new(AtomicBool::new(false));

        let stop_clone = stop.clone();
        let _ = ctrlc::set_handler(move || {
            stop_clone.store(true, Ordering::Relaxed);
        });

        crate::backend::cpal_backend::play_audio(
            &samples,
            decoded.sample_rate,
            decoded.channels,
            volume.as_linear(),
            &stop,
        )?;

        if stop.load(Ordering::Relaxed) {
            println!("\nStopped.");
            break;
        }
    }

    Ok(())
}
