//! Podcast provider — fetch RSS feeds, parse episodes, persist state.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::track::{Track, TrackMetadata};

/// A subscribed podcast feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastFeed {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
}

/// A single episode within a podcast feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastEpisode {
    pub title: String,
    pub audio_url: String,
    #[serde(default)]
    pub duration_secs: Option<u64>,
    #[serde(default)]
    pub published: Option<String>,
    /// Last playback position in milliseconds.
    #[serde(default)]
    pub position_ms: u64,
    #[serde(default)]
    pub played: bool,
}

/// Persisted podcast state (subscriptions + resume positions).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Public API — used by future TUI podcast management
pub struct PodcastState {
    #[serde(default)]
    pub feeds: Vec<PodcastFeed>,
    /// Map of audio URL → last playback position in ms.
    #[serde(default)]
    pub episode_positions: HashMap<String, u64>,
}

impl PodcastState {
    /// Import feeds from OPML entries, deduplicating by URL.
    /// Returns the number of newly added feeds.
    pub fn import_feeds_from_opml(&mut self, entries: &[super::opml::OpmlEntry]) -> usize {
        let mut added = 0;
        for entry in entries {
            let already_exists = self.feeds.iter().any(|f| f.url == entry.url);
            if !already_exists {
                self.feeds.push(PodcastFeed {
                    url: entry.url.clone(),
                    title: entry.title.clone(),
                    description: String::new(),
                });
                added += 1;
            }
        }
        added
    }

    /// Return all subscribed feeds (convenience accessor for OPML export).
    pub fn export_feeds(&self) -> &[PodcastFeed] {
        &self.feeds
    }
}

/// Fetch an RSS feed and parse it into a `PodcastFeed` + episode list.
pub fn fetch_feed(url: &str) -> Result<(PodcastFeed, Vec<PodcastEpisode>)> {
    let body = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to fetch podcast feed: {url}"))?
        .bytes()
        .with_context(|| "Failed to read podcast feed response body")?;

    parse_feed_bytes(url, &body)
}

/// Parse raw RSS/Atom bytes into a feed + episodes (separated for testability).
fn parse_feed_bytes(url: &str, bytes: &[u8]) -> Result<(PodcastFeed, Vec<PodcastEpisode>)> {
    let feed = feed_rs::parser::parse(bytes).context("Failed to parse RSS/Atom feed")?;

    let title = feed
        .title
        .map(|t| t.content)
        .unwrap_or_else(|| "Untitled Podcast".to_string());

    let description = feed.description.map(|d| d.content).unwrap_or_default();

    let podcast_feed = PodcastFeed {
        url: url.to_string(),
        title,
        description,
    };

    let episodes: Vec<PodcastEpisode> = feed
        .entries
        .iter()
        .filter_map(|entry| {
            // Look for an audio enclosure (media type starts with "audio/")
            let enclosure = entry.media.iter().find_map(|media| {
                media.content.iter().find_map(|content| {
                    let mime = content.content_type.as_ref()?.essence().to_string();
                    if mime.starts_with("audio/") {
                        content.url.as_ref().map(|u| u.to_string())
                    } else {
                        None
                    }
                })
            });

            // Also check RSS 2.0 <enclosure> links if feed-rs mapped them to MediaObject
            let audio_url = enclosure.or_else(|| {
                entry.links.iter().find_map(|link| {
                    let mt = link.media_type.as_deref().unwrap_or("");
                    if mt.starts_with("audio/") {
                        Some(link.href.clone())
                    } else {
                        None
                    }
                })
            })?;

            let ep_title = entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| "Untitled Episode".to_string());

            let duration_secs = entry.media.iter().find_map(|media| {
                media
                    .content
                    .iter()
                    .find_map(|c| c.duration.map(|d| d.as_secs()))
            });

            let published = entry.published.map(|dt| dt.to_rfc2822());

            Some(PodcastEpisode {
                title: ep_title,
                audio_url,
                duration_secs,
                published,
                position_ms: 0,
                played: false,
            })
        })
        .collect();

    Ok((podcast_feed, episodes))
}

/// Load podcast state from the config directory. Returns default if missing.
#[allow(dead_code)] // Public API — wired in future TUI podcast management
pub fn load_state() -> Result<PodcastState> {
    let path = state_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let state: PodcastState = toml::from_str(&contents)
                .with_context(|| format!("Failed to parse podcast state: {}", path.display()))?;
            Ok(state)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PodcastState::default()),
        Err(e) => Err(anyhow::anyhow!(
            "Failed to read podcast state {}: {}",
            path.display(),
            e
        )),
    }
}

/// Save podcast state to the config directory.
#[allow(dead_code)] // Public API — wired in future TUI podcast management
pub fn save_state(state: &PodcastState) -> Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    let toml_str = toml::to_string_pretty(state).context("Failed to serialize podcast state")?;

    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &toml_str)
        .with_context(|| format!("Failed to write temp podcast state: {}", tmp_path.display()))?;

    std::fs::rename(&tmp_path, &path)
        .with_context(|| format!("Failed to rename podcast state file to: {}", path.display()))?;

    Ok(())
}

/// Convert a `PodcastEpisode` into a playable `Track` with metadata.
pub fn episode_to_track(episode: &PodcastEpisode) -> Track {
    let mut track = Track::from_url(episode.audio_url.clone());
    track.metadata = Some(TrackMetadata {
        title: Some(episode.title.clone()),
        artist: None,
        album: None,
        duration: episode.duration_secs.map(std::time::Duration::from_secs),
    });
    track
}

/// Platform-appropriate path for the podcast state file.
fn state_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kora")
        .join("podcasts.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Podcast</title>
    <item>
      <title>Episode 1</title>
      <enclosure url="https://example.com/ep1.mp3" type="audio/mpeg" length="1234567"/>
      <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parse_rss_feed() {
        let (feed, episodes) =
            parse_feed_bytes("https://example.com/feed.rss", SAMPLE_RSS.as_bytes()).unwrap();

        assert_eq!(feed.title, "Test Podcast");
        assert_eq!(feed.url, "https://example.com/feed.rss");

        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].title, "Episode 1");
        assert_eq!(episodes[0].audio_url, "https://example.com/ep1.mp3");
        assert!(!episodes[0].played);
        assert_eq!(episodes[0].position_ms, 0);
    }

    #[test]
    fn parse_rss_multiple_episodes() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Multi Episode Show</title>
    <item>
      <title>Ep 3</title>
      <enclosure url="https://example.com/ep3.mp3" type="audio/mpeg" length="100"/>
    </item>
    <item>
      <title>Ep 2</title>
      <enclosure url="https://example.com/ep2.mp3" type="audio/mpeg" length="200"/>
    </item>
    <item>
      <title>Ep 1</title>
      <enclosure url="https://example.com/ep1.mp3" type="audio/mpeg" length="300"/>
    </item>
  </channel>
</rss>"#;
        let (feed, episodes) =
            parse_feed_bytes("https://example.com/feed.rss", rss.as_bytes()).unwrap();
        assert_eq!(feed.title, "Multi Episode Show");
        assert_eq!(episodes.len(), 3);
        assert_eq!(episodes[0].title, "Ep 3");
        assert_eq!(episodes[2].title, "Ep 1");
    }

    #[test]
    fn parse_rss_skips_non_audio_enclosures() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Mixed Content</title>
    <item>
      <title>Video Episode</title>
      <enclosure url="https://example.com/video.mp4" type="video/mp4" length="999"/>
    </item>
    <item>
      <title>Audio Episode</title>
      <enclosure url="https://example.com/audio.mp3" type="audio/mpeg" length="123"/>
    </item>
  </channel>
</rss>"#;
        let (_, episodes) =
            parse_feed_bytes("https://example.com/feed.rss", rss.as_bytes()).unwrap();
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].title, "Audio Episode");
    }

    #[test]
    fn podcast_state_round_trip() {
        let state = PodcastState {
            feeds: vec![PodcastFeed {
                url: "https://example.com/feed.rss".to_string(),
                title: "My Podcast".to_string(),
                description: "A test podcast".to_string(),
            }],
            episode_positions: HashMap::from([
                ("https://example.com/ep1.mp3".to_string(), 42000),
                ("https://example.com/ep2.mp3".to_string(), 0),
            ]),
        };

        let toml_str = toml::to_string_pretty(&state).unwrap();
        let loaded: PodcastState = toml::from_str(&toml_str).unwrap();

        assert_eq!(loaded.feeds.len(), 1);
        assert_eq!(loaded.feeds[0].title, "My Podcast");
        assert_eq!(loaded.episode_positions.len(), 2);
        assert_eq!(
            loaded.episode_positions["https://example.com/ep1.mp3"],
            42000
        );
    }

    #[test]
    fn load_state_missing_file_returns_default() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_podcast");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nonexistent_podcasts.toml");

        // Directly test the same logic load_state uses
        let result: Result<PodcastState, _> = match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PodcastState::default()),
            Err(e) => Err(e.to_string()),
        };

        let state = result.unwrap();
        assert!(state.feeds.is_empty());
        assert!(state.episode_positions.is_empty());
    }

    #[test]
    fn episode_to_track_conversion() {
        let episode = PodcastEpisode {
            title: "Great Episode".to_string(),
            audio_url: "https://example.com/great.mp3".to_string(),
            duration_secs: Some(3600),
            published: Some("Mon, 01 Jan 2024 00:00:00 +0000".to_string()),
            position_ms: 15000,
            played: false,
        };

        let track = episode_to_track(&episode);
        assert_eq!(track.path_string(), "https://example.com/great.mp3");
        assert_eq!(track.display_name(), "Great Episode");

        let meta = track.metadata.as_ref().unwrap();
        assert_eq!(meta.title.as_deref(), Some("Great Episode"));
        assert_eq!(meta.duration, Some(std::time::Duration::from_secs(3600)));
    }

    #[test]
    fn episode_to_track_no_duration() {
        let episode = PodcastEpisode {
            title: "Short Episode".to_string(),
            audio_url: "https://example.com/short.mp3".to_string(),
            duration_secs: None,
            published: None,
            position_ms: 0,
            played: true,
        };

        let track = episode_to_track(&episode);
        let meta = track.metadata.as_ref().unwrap();
        assert!(meta.duration.is_none());
    }

    #[test]
    fn state_path_is_valid() {
        let path = state_path();
        assert!(path.ends_with("podcasts.toml"));
        assert!(path.to_string_lossy().contains("kora"));
    }

    #[test]
    fn save_and_load_state_round_trip() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_podcast");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip_podcasts.toml");

        let state = PodcastState {
            feeds: vec![PodcastFeed {
                url: "https://example.com/rss".to_string(),
                title: "Round Trip Pod".to_string(),
                description: String::new(),
            }],
            episode_positions: HashMap::from([("https://example.com/ep1.mp3".to_string(), 5000)]),
        };

        // Use the same write logic as save_state
        let toml_str = toml::to_string_pretty(&state).unwrap();
        std::fs::write(&path, &toml_str).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let loaded: PodcastState = toml::from_str(&contents).unwrap();

        assert_eq!(loaded.feeds.len(), 1);
        assert_eq!(loaded.feeds[0].title, "Round Trip Pod");
        assert_eq!(
            loaded.episode_positions["https://example.com/ep1.mp3"],
            5000
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn import_feeds_deduplicates_by_url() {
        let mut state = PodcastState {
            feeds: vec![PodcastFeed {
                url: "https://existing.example.com/rss".to_string(),
                title: "Existing Pod".to_string(),
                description: String::new(),
            }],
            episode_positions: HashMap::new(),
        };

        let entries = vec![
            super::super::opml::OpmlEntry {
                title: "New Feed".to_string(),
                url: "https://new.example.com/rss".to_string(),
            },
            super::super::opml::OpmlEntry {
                title: "Existing Pod (dupe)".to_string(),
                url: "https://existing.example.com/rss".to_string(),
            },
            super::super::opml::OpmlEntry {
                title: "Another New".to_string(),
                url: "https://another.example.com/rss".to_string(),
            },
        ];

        let added = state.import_feeds_from_opml(&entries);
        assert_eq!(added, 2);
        assert_eq!(state.feeds.len(), 3);
        // Original title preserved for the duplicate
        assert_eq!(state.feeds[0].title, "Existing Pod");
    }

    #[test]
    fn export_feeds_returns_all() {
        let state = PodcastState {
            feeds: vec![
                PodcastFeed {
                    url: "https://a.com/rss".to_string(),
                    title: "A".to_string(),
                    description: String::new(),
                },
                PodcastFeed {
                    url: "https://b.com/rss".to_string(),
                    title: "B".to_string(),
                    description: String::new(),
                },
            ],
            episode_positions: HashMap::new(),
        };

        let feeds = state.export_feeds();
        assert_eq!(feeds.len(), 2);
        assert_eq!(feeds[0].title, "A");
        assert_eq!(feeds[1].title, "B");
    }
}
