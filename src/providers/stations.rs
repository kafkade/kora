//! Custom radio stations loaded from a user-managed TOML file.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::core::track::{Track, TrackMetadata};

/// A user-defined radio station from `stations.toml`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Public API — wired in TUI browsing (future)
pub struct CustomStation {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

/// Root structure of `stations.toml`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Used by load_custom_stations
pub struct StationsFile {
    #[serde(default)]
    pub station: Vec<CustomStation>,
}

impl CustomStation {
    /// Convert this custom station into a playable `Track`.
    #[allow(dead_code)] // Public API — wired in TUI browsing (future)
    pub fn to_track(&self) -> Track {
        let album = self.country.as_ref().map(|c| format!("Radio — {c}"));

        let mut track = Track::from_url(self.url.clone());
        track.metadata = Some(TrackMetadata {
            title: Some(self.name.clone()),
            artist: self.genre.clone(),
            album,
            duration: None,
        });
        track
    }
}

/// Load custom stations from `<config_dir>/kora/stations.toml`.
///
/// Returns an empty vec if the file does not exist.
#[allow(dead_code)] // Public API — wired in TUI browsing (future)
pub fn load_custom_stations() -> Result<Vec<CustomStation>> {
    let path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("kora")
        .join("stations.toml");

    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let file: StationsFile = toml::from_str(&contents)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            Ok(file.station)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(anyhow::anyhow!("Failed to read {}: {}", path.display(), e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_stations_toml() {
        let toml_str = r#"
            [[station]]
            name = "KEXP"
            url = "https://kexp-mp3-128.streamguys1.com/kexp128.mp3"
            genre = "Indie"
            country = "US"

            [[station]]
            name = "SomaFM"
            url = "https://ice1.somafm.com/groovesalad-256-mp3"
        "#;

        let file: StationsFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.station.len(), 2);

        let kexp = &file.station[0];
        assert_eq!(kexp.name, "KEXP");
        assert_eq!(kexp.url, "https://kexp-mp3-128.streamguys1.com/kexp128.mp3");
        assert_eq!(kexp.genre.as_deref(), Some("Indie"));
        assert_eq!(kexp.country.as_deref(), Some("US"));

        let soma = &file.station[1];
        assert_eq!(soma.name, "SomaFM");
        assert!(soma.genre.is_none());
        assert!(soma.country.is_none());
    }

    #[test]
    fn custom_station_to_track() {
        let station = CustomStation {
            name: "KEXP".to_string(),
            url: "https://kexp.example.com/stream.mp3".to_string(),
            genre: Some("Indie".to_string()),
            country: Some("US".to_string()),
        };

        let track = station.to_track();
        assert_eq!(track.path_string(), "https://kexp.example.com/stream.mp3");
        assert_eq!(track.display_name(), "Indie — KEXP");

        let meta = track.metadata.as_ref().unwrap();
        assert_eq!(meta.title.as_deref(), Some("KEXP"));
        assert_eq!(meta.artist.as_deref(), Some("Indie"));
        assert_eq!(meta.album.as_deref(), Some("Radio — US"));
    }

    #[test]
    fn custom_station_to_track_minimal() {
        let station = CustomStation {
            name: "Mystery FM".to_string(),
            url: "https://mystery.example.com/stream".to_string(),
            genre: None,
            country: None,
        };

        let track = station.to_track();
        assert_eq!(track.display_name(), "Mystery FM");

        let meta = track.metadata.as_ref().unwrap();
        assert!(meta.artist.is_none());
        assert!(meta.album.is_none());
    }

    #[test]
    fn load_custom_stations_missing_file_returns_empty() {
        // dirs::config_dir()/kora/stations.toml is unlikely to exist in CI,
        // so this exercises the NotFound path.
        let stations = load_custom_stations().unwrap();
        // May be empty (no file) or non-empty (if file exists on dev machine).
        // The important thing is it doesn't error out.
        let _ = stations;
    }

    #[test]
    fn empty_stations_file() {
        let toml_str = "# empty stations file\n";
        let file: StationsFile = toml::from_str(toml_str).unwrap();
        assert!(file.station.is_empty());
    }
}
