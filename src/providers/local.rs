use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::core::track::Track;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "wav", "opus", "aac", "m4a", "wma", "aiff",
];

/// Resolve CLI inputs (files and directories) into playable tracks.
pub fn resolve_inputs(inputs: &[PathBuf]) -> Result<Vec<Track>> {
    let mut tracks = Vec::new();

    for input in inputs {
        if input.is_dir() {
            resolve_directory(input, &mut tracks)?;
        } else if input.is_file() && is_audio_file(input) {
            tracks.push(Track::from_file(input.clone()));
        } else if input.is_file() {
            tracing::warn!("Skipping unsupported file: {}", input.display());
        } else {
            tracing::warn!("Not found: {}", input.display());
        }
    }

    // Sort files within directories for predictable playback order
    tracks.sort_by(|a, b| {
        let path_a = match &a.source {
            crate::core::track::TrackSource::File(p) => p.to_string_lossy().into_owned(),
            _ => String::new(),
        };
        let path_b = match &b.source {
            crate::core::track::TrackSource::File(p) => p.to_string_lossy().into_owned(),
            _ => String::new(),
        };
        path_a.cmp(&path_b)
    });

    Ok(tracks)
}

fn resolve_directory(dir: &Path, tracks: &mut Vec<Track>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    entries.sort();

    for path in entries {
        if path.is_file() && is_audio_file(&path) {
            tracks.push(Track::from_file(path));
        }
    }

    Ok(())
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}
