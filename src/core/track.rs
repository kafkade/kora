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
    #[allow(dead_code)] // Used in future phases (library browsing)
    pub album: Option<String>,
    #[allow(dead_code)] // Used in future phases (progress bar)
    pub duration: Option<Duration>,
}

impl Track {
    pub fn from_file(path: PathBuf) -> Self {
        Self {
            source: TrackSource::File(path),
            metadata: None,
        }
    }

    pub fn from_url(url: String) -> Self {
        Self {
            source: TrackSource::Url(url),
            metadata: None,
        }
    }

    /// Return the source path or URL as a string (for session persistence).
    pub fn path_string(&self) -> String {
        match &self.source {
            TrackSource::File(p) => p.to_string_lossy().into_owned(),
            TrackSource::Url(url) => url.clone(),
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
