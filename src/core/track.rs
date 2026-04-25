use std::path::PathBuf;
use std::time::Duration;

/// A playable audio item — either a local file or a URL.
#[derive(Debug, Clone)]
pub struct Track {
    pub source: TrackSource,
    pub metadata: Option<TrackMetadata>,
}

#[derive(Debug, Clone)]
pub enum TrackSource {
    File(PathBuf),
    Url(String),
}

#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
}

impl Track {
    pub fn from_file(path: PathBuf) -> Self {
        Self {
            source: TrackSource::File(path),
            metadata: None,
        }
    }

    pub fn display_name(&self) -> String {
        if let Some(ref meta) = self.metadata {
            if let (Some(artist), Some(title)) = (&meta.artist, &meta.title) {
                return format!("{artist} — {title}");
            }
            if let Some(title) = &meta.title {
                return title.clone();
            }
        }
        match &self.source {
            TrackSource::File(path) => path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown".into()),
            TrackSource::Url(url) => url.clone(),
        }
    }
}
