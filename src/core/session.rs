//! Session persistence — save and restore playback state across runs.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persisted playback state saved between sessions.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Session {
    pub track_path: Option<String>,
    pub position_ms: u64,
    pub queue: Vec<String>,
    pub queue_index: usize,
    pub volume_db: f32,
    pub eq_preset: Option<String>,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub repeat: String,
}

impl Session {
    /// Serialize to TOML and write atomically (write to .tmp, then rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let toml_str = toml::to_string_pretty(self).context("Failed to serialize session")?;

        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, toml_str).with_context(|| {
            format!("Failed to write temp session file: {}", tmp_path.display())
        })?;

        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("Failed to rename session file to: {}", path.display()))?;

        Ok(())
    }

    /// Deserialize from TOML. Returns default session if file is missing.
    pub fn load(path: &Path) -> Result<Session> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                let session: Session = toml::from_str(&contents)
                    .with_context(|| format!("Failed to parse session file: {}", path.display()))?;
                Ok(session)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Session::default()),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to read session file {}: {}",
                path.display(),
                e
            )),
        }
    }

    /// Platform-appropriate path for the session file.
    pub fn session_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kora")
            .join("session.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_session")
    }

    #[test]
    fn round_trip_save_load() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round_trip.toml");

        let session = Session {
            track_path: Some("C:/Music/song.mp3".to_string()),
            position_ms: 142000,
            queue: vec![
                "C:/Music/song1.mp3".to_string(),
                "C:/Music/song2.mp3".to_string(),
            ],
            queue_index: 1,
            volume_db: -3.0,
            eq_preset: Some("Rock".to_string()),
            shuffle: true,
            repeat: "All".to_string(),
        };

        session.save(&path).unwrap();
        let loaded = Session::load(&path).unwrap();
        assert_eq!(session, loaded);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = test_dir().join("nonexistent.toml");
        let session = Session::load(&path).unwrap();
        assert_eq!(session, Session::default());
    }

    #[test]
    fn load_corrupt_file_returns_error() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corrupt.toml");
        std::fs::write(&path, "this is not valid {{{{").unwrap();

        let result = Session::load(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn session_path_is_valid() {
        let path = Session::session_path();
        assert!(path.ends_with("session.toml"));
        assert!(path.to_string_lossy().contains("kora"));
    }
}
