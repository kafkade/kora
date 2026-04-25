use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};

use crate::core::track::{Track, TrackSource};
use crate::core::types::Volume;

/// Play a list of tracks sequentially.
/// Spike 1: decode fully into memory, then stream to CPAL via rtrb.
pub fn play_tracks(tracks: &[Track], volume_db: f32) -> Result<()> {
    let volume = Volume(volume_db);

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

        let stop = Arc::new(AtomicBool::new(false));

        // Set up Ctrl+C handler for this track
        let stop_clone = stop.clone();
        let _ = ctrlc::set_handler(move || {
            stop_clone.store(true, Ordering::Relaxed);
        });

        crate::backend::cpal_backend::play_audio(
            &decoded.samples,
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
