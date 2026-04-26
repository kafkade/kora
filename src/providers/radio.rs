//! Radio Browser API client for discovering internet radio stations.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::core::track::{Track, TrackMetadata};

const API_BASE: &str = "https://de1.api.radio-browser.info";

/// A radio station from the Radio Browser API.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Fields used by tests and future TUI browsing
pub struct RadioStation {
    pub stationuuid: String,
    pub name: String,
    pub url_resolved: String,
    pub codec: String,
    pub bitrate: u32,
    pub country: String,
    pub countrycode: String,
    pub tags: String,
    #[serde(default)]
    pub favicon: String,
    #[serde(default)]
    pub language: String,
}

impl RadioStation {
    /// Convert this station into a playable `Track` with metadata.
    pub fn to_track(&self) -> Track {
        let genre = self
            .tags
            .split(',')
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut track = Track::from_url(self.url_resolved.clone());
        track.metadata = Some(TrackMetadata {
            title: Some(self.name.clone()),
            artist: genre,
            album: Some(format!("Radio — {}", self.country)),
            duration: None,
        });
        track
    }
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("kora/0.1.0")
        .build()
        .expect("failed to build HTTP client")
}

/// Search for stations by name.
pub fn search_by_name(query: &str, limit: usize) -> Result<Vec<RadioStation>> {
    let url = format!(
        "{API_BASE}/json/stations/byname/{query}?limit={limit}",
        query = urlencoded(query),
    );
    fetch_stations(&url)
}

/// Search for stations by genre/tag.
#[allow(dead_code)] // Public API — wired in TUI browsing (future)
pub fn search_by_tag(tag: &str, limit: usize) -> Result<Vec<RadioStation>> {
    let url = format!(
        "{API_BASE}/json/stations/bytag/{tag}?limit={limit}",
        tag = urlencoded(tag),
    );
    fetch_stations(&url)
}

/// Search for stations by country code (e.g. "US", "DE").
#[allow(dead_code)] // Public API — wired in TUI browsing (future)
pub fn search_by_country(code: &str, limit: usize) -> Result<Vec<RadioStation>> {
    let url = format!(
        "{API_BASE}/json/stations/bycountry/{code}?limit={limit}",
        code = urlencoded(code),
    );
    fetch_stations(&url)
}

fn fetch_stations(url: &str) -> Result<Vec<RadioStation>> {
    let response = client()
        .get(url)
        .send()
        .with_context(|| format!("Failed to reach Radio Browser API: {url}"))?;

    let stations: Vec<RadioStation> = response
        .json()
        .with_context(|| "Failed to parse Radio Browser response")?;

    Ok(stations)
}

/// Minimal percent-encoding for path segments (spaces → %20, etc.).
fn urlencoded(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"[
        {
            "stationuuid": "abc-123",
            "name": "Test FM",
            "url_resolved": "https://stream.example.com/test.mp3",
            "codec": "MP3",
            "bitrate": 128,
            "country": "Germany",
            "countrycode": "DE",
            "tags": "pop,rock,indie",
            "favicon": "https://example.com/icon.png",
            "language": "german"
        }
    ]"#;

    #[test]
    fn deserialize_radio_station() {
        let stations: Vec<RadioStation> = serde_json::from_str(SAMPLE_JSON).unwrap();
        assert_eq!(stations.len(), 1);
        let s = &stations[0];
        assert_eq!(s.stationuuid, "abc-123");
        assert_eq!(s.name, "Test FM");
        assert_eq!(s.url_resolved, "https://stream.example.com/test.mp3");
        assert_eq!(s.codec, "MP3");
        assert_eq!(s.bitrate, 128);
        assert_eq!(s.country, "Germany");
        assert_eq!(s.countrycode, "DE");
        assert_eq!(s.tags, "pop,rock,indie");
        assert_eq!(s.favicon, "https://example.com/icon.png");
        assert_eq!(s.language, "german");
    }

    #[test]
    fn deserialize_missing_optional_fields() {
        let json = r#"[{
            "stationuuid": "xyz",
            "name": "Minimal Station",
            "url_resolved": "https://s.example.com/stream",
            "codec": "AAC",
            "bitrate": 64,
            "country": "US",
            "countrycode": "US",
            "tags": ""
        }]"#;
        let stations: Vec<RadioStation> = serde_json::from_str(json).unwrap();
        assert_eq!(stations[0].favicon, "");
        assert_eq!(stations[0].language, "");
    }

    #[test]
    fn radio_station_to_track() {
        let stations: Vec<RadioStation> = serde_json::from_str(SAMPLE_JSON).unwrap();
        let track = stations[0].to_track();

        assert_eq!(track.path_string(), "https://stream.example.com/test.mp3");
        assert_eq!(track.display_name(), "pop — Test FM");
        let meta = track.metadata.as_ref().unwrap();
        assert_eq!(meta.title.as_deref(), Some("Test FM"));
        assert_eq!(meta.artist.as_deref(), Some("pop"));
        assert_eq!(meta.album.as_deref(), Some("Radio — Germany"));
        assert!(meta.duration.is_none());
    }

    #[test]
    fn radio_station_to_track_empty_tags() {
        let json = r#"[{
            "stationuuid": "xyz",
            "name": "No Tags FM",
            "url_resolved": "https://s.example.com/stream",
            "codec": "MP3",
            "bitrate": 128,
            "country": "US",
            "countrycode": "US",
            "tags": ""
        }]"#;
        let stations: Vec<RadioStation> = serde_json::from_str(json).unwrap();
        let track = stations[0].to_track();

        let meta = track.metadata.as_ref().unwrap();
        assert!(meta.artist.is_none());
    }

    #[test]
    fn urlencoded_encodes_spaces() {
        assert_eq!(urlencoded("lofi hip hop"), "lofi%20hip%20hop");
    }

    #[test]
    fn urlencoded_preserves_safe_chars() {
        assert_eq!(urlencoded("jazz-rock_fusion"), "jazz-rock_fusion");
    }
}
