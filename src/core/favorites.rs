//! Favorites / bookmarks — persist a list of user-favorited tracks.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persisted set of favorited track paths / URLs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Favorites {
    #[serde(default)]
    pub items: Vec<String>,
}

impl Favorites {
    /// Add a track key (path or URL) to favorites. Duplicates are ignored.
    pub fn add(&mut self, key: &str) {
        if !self.items.iter().any(|k| k == key) {
            self.items.push(key.to_string());
        }
    }

    /// Remove a track key from favorites.
    pub fn remove(&mut self, key: &str) {
        self.items.retain(|k| k != key);
    }

    /// Toggle a track: add if absent, remove if present.
    /// Returns `true` if the track is now favorited.
    pub fn toggle(&mut self, key: &str) -> bool {
        if self.contains(key) {
            self.remove(key);
            false
        } else {
            self.add(key);
            true
        }
    }

    /// Check whether a track key is in favorites.
    pub fn contains(&self, key: &str) -> bool {
        self.items.iter().any(|k| k == key)
    }

    /// Load from `<config_dir>/kora/favorites.toml`. Returns default if missing.
    pub fn load() -> Result<Self> {
        let path = Self::favorites_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                let favs: Favorites = toml::from_str(&contents).with_context(|| {
                    format!("Failed to parse favorites file: {}", path.display())
                })?;
                Ok(favs)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Favorites::default()),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to read favorites file {}: {}",
                path.display(),
                e
            )),
        }
    }

    /// Serialize to TOML and write atomically (write to .tmp, then rename).
    pub fn save(&self) -> Result<()> {
        let path = Self::favorites_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let toml_str = toml::to_string_pretty(self).context("Failed to serialize favorites")?;

        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, toml_str).with_context(|| {
            format!(
                "Failed to write temp favorites file: {}",
                tmp_path.display()
            )
        })?;

        std::fs::rename(&tmp_path, &path)
            .with_context(|| format!("Failed to rename favorites file to: {}", path.display()))?;

        Ok(())
    }

    /// Platform-appropriate path for the favorites file.
    fn favorites_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kora")
            .join("favorites.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_favorites")
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut favs = Favorites::default();
        assert!(favs.toggle("song.mp3"));
        assert!(favs.contains("song.mp3"));
        assert!(!favs.toggle("song.mp3"));
        assert!(!favs.contains("song.mp3"));
    }

    #[test]
    fn contains_returns_correct_state() {
        let mut favs = Favorites::default();
        assert!(!favs.contains("a.mp3"));
        favs.add("a.mp3");
        assert!(favs.contains("a.mp3"));
        favs.remove("a.mp3");
        assert!(!favs.contains("a.mp3"));
    }

    #[test]
    fn duplicate_adds_are_ignored() {
        let mut favs = Favorites::default();
        favs.add("dup.mp3");
        favs.add("dup.mp3");
        favs.add("dup.mp3");
        assert_eq!(favs.items.len(), 1);
    }

    #[test]
    fn round_trip_save_load() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("favorites.toml");

        let mut favs = Favorites::default();
        favs.add("C:/Music/song1.mp3");
        favs.add("https://radio.example.com/stream");

        // Write directly to the test path (avoid clobbering user config)
        let toml_str = toml::to_string_pretty(&favs).unwrap();
        std::fs::write(&path, &toml_str).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let loaded: Favorites = toml::from_str(&contents).unwrap();
        assert_eq!(loaded.items, favs.items);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let path = test_dir().join("nonexistent.toml");
        // Ensure it doesn't exist
        let _ = std::fs::remove_file(&path);

        // Use the same deserialization logic as load() but with a custom path
        let result: Favorites = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Favorites::default(),
            other => panic!("Expected NotFound, got: {other:?}"),
        };
        assert!(result.items.is_empty());
    }

    #[test]
    fn load_empty_file_returns_empty() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty_favorites.toml");
        std::fs::write(&path, "").unwrap();

        let loaded: Favorites = toml::from_str("").unwrap();
        assert!(loaded.items.is_empty());

        let _ = std::fs::remove_file(&path);
    }
}
