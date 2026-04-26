//! LRC lyrics parser and synced lyrics display support.
//!
//! Parses `.lrc` sidecar files and embedded lyrics tags to provide
//! time-synced lyrics for the TUI display.

use std::path::Path;

use lofty::prelude::*;
use lofty::tag::ItemKey;

use crate::core::track::{Track, TrackSource};

/// A single timed lyric line.
#[derive(Debug, Clone)]
pub struct LyricLine {
    pub timestamp_ms: u64,
    pub text: String,
}

/// Parsed lyrics for a track.
#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    pub source: LyricsSource,
}

/// Where the lyrics were loaded from.
#[derive(Debug, Clone, Default)]
pub enum LyricsSource {
    #[default]
    None,
    #[allow(dead_code)]
    LrcFile(String),
    EmbeddedTag,
}

/// Parse LRC format content into [`Lyrics`].
///
/// Handles `[mm:ss.xx]` and `[mm:ss]` timestamps, multiple timestamps per line,
/// and skips metadata lines like `[ar:Artist]`.
pub fn parse_lrc(content: &str) -> Lyrics {
    let mut lines = Vec::new();

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut timestamps = Vec::new();
        let mut rest = trimmed;

        // Extract all [mm:ss.xx] timestamps from the beginning of the line
        while rest.starts_with('[') {
            let Some(close) = rest.find(']') else {
                break;
            };
            let tag = &rest[1..close];
            if let Some(ts) = parse_timestamp(tag) {
                timestamps.push(ts);
                rest = &rest[close + 1..];
            } else {
                // Metadata tag like [ar:Artist] — skip entire line
                if is_metadata_tag(tag) {
                    timestamps.clear();
                    break;
                }
                // Unknown bracket content — stop parsing timestamps
                break;
            }
        }

        if timestamps.is_empty() {
            continue;
        }

        let text = rest.trim().to_string();
        for ts in timestamps {
            lines.push(LyricLine {
                timestamp_ms: ts,
                text: text.clone(),
            });
        }
    }

    lines.sort_by_key(|l| l.timestamp_ms);

    Lyrics {
        lines,
        source: LyricsSource::None,
    }
}

/// Parse a timestamp string like "01:30.50" or "01:30" into milliseconds.
fn parse_timestamp(tag: &str) -> Option<u64> {
    let parts: Vec<&str> = tag.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let minutes: u64 = parts[0].parse().ok()?;

    // Seconds may have a decimal component
    let sec_parts: Vec<&str> = parts[1].split('.').collect();
    let seconds: u64 = sec_parts[0].parse().ok()?;

    let centiseconds: u64 = if sec_parts.len() > 1 {
        let frac = sec_parts[1];
        match frac.len() {
            1 => frac.parse::<u64>().ok()? * 100,
            2 => frac.parse::<u64>().ok()? * 10,
            3 => frac.parse::<u64>().ok()?,
            _ => frac[..3].parse::<u64>().ok()?,
        }
    } else {
        0
    };

    Some(minutes * 60_000 + seconds * 1_000 + centiseconds)
}

/// Check if a bracket tag is an LRC metadata tag (e.g. `ar:`, `ti:`, `al:`).
fn is_metadata_tag(tag: &str) -> bool {
    let lower = tag.to_ascii_lowercase();
    lower.starts_with("ar:")
        || lower.starts_with("ti:")
        || lower.starts_with("al:")
        || lower.starts_with("au:")
        || lower.starts_with("by:")
        || lower.starts_with("offset:")
        || lower.starts_with("re:")
        || lower.starts_with("ve:")
        || lower.starts_with("length:")
}

/// Load lyrics for a track, trying sidecar `.lrc` file first, then embedded tags.
pub fn load_lyrics_for_track(track: &Track) -> Lyrics {
    match &track.source {
        TrackSource::File(path) => {
            // 1. Try sidecar .lrc file
            if let Some(lyrics) = try_load_lrc_file(path) {
                return lyrics;
            }
            // 2. Try embedded lyrics tag
            if let Some(lyrics) = try_load_embedded_lyrics(path) {
                return lyrics;
            }
            Lyrics::default()
        }
        TrackSource::Url(_) => Lyrics::default(),
    }
}

/// Try to load a `.lrc` sidecar file next to the audio file.
fn try_load_lrc_file(audio_path: &Path) -> Option<Lyrics> {
    let lrc_path = audio_path.with_extension("lrc");
    let content = std::fs::read_to_string(&lrc_path).ok()?;
    let mut lyrics = parse_lrc(&content);
    if lyrics.lines.is_empty() {
        return None;
    }
    lyrics.source = LyricsSource::LrcFile(
        lrc_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    Some(lyrics)
}

/// Try to read embedded lyrics from audio file tags via lofty.
fn try_load_embedded_lyrics(path: &Path) -> Option<Lyrics> {
    let tagged_file = lofty::read_from_path(path).ok()?;

    for tag in tagged_file.tags() {
        // Try LYRICS item key (covers USLT for ID3, LYRICS for Vorbis)
        if let Some(text) = tag.get_string(ItemKey::Lyrics)
            && !text.trim().is_empty()
        {
            let mut lyrics = parse_lrc(text);
            if !lyrics.lines.is_empty() {
                lyrics.source = LyricsSource::EmbeddedTag;
                return Some(lyrics);
            }
            // Plain text lyrics without timestamps — treat as single block
            let lines: Vec<LyricLine> = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
                .map(|(i, line)| LyricLine {
                    // Space lines 5 seconds apart for unsynced display
                    timestamp_ms: i as u64 * 5000,
                    text: line.trim().to_string(),
                })
                .collect();
            if !lines.is_empty() {
                return Some(Lyrics {
                    lines,
                    source: LyricsSource::EmbeddedTag,
                });
            }
        }
    }

    None
}

/// Find the index of the lyric line that should be highlighted at the given
/// playback position (the last line whose timestamp <= position).
pub fn current_line_index(lyrics: &Lyrics, position_ms: u64) -> Option<usize> {
    if lyrics.lines.is_empty() {
        return None;
    }

    // Binary search for the last line with timestamp <= position_ms
    match lyrics
        .lines
        .binary_search_by_key(&position_ms, |l| l.timestamp_ms)
    {
        Ok(i) => {
            // Exact match — find the last line with the same timestamp
            let mut idx = i;
            while idx + 1 < lyrics.lines.len() && lyrics.lines[idx + 1].timestamp_ms == position_ms
            {
                idx += 1;
            }
            Some(idx)
        }
        Err(0) => {
            // Position is before the first line
            if position_ms < lyrics.lines[0].timestamp_ms {
                None
            } else {
                Some(0)
            }
        }
        Err(i) => Some(i - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_lrc_line() {
        let lyrics = parse_lrc("[00:12.34]Hello");
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].timestamp_ms, 12340);
        assert_eq!(lyrics.lines[0].text, "Hello");
    }

    #[test]
    fn parse_lrc_seconds_only() {
        let lyrics = parse_lrc("[01:30]Text");
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].timestamp_ms, 90000);
        assert_eq!(lyrics.lines[0].text, "Text");
    }

    #[test]
    fn parse_multiple_timestamps() {
        let lyrics = parse_lrc("[00:12.00][00:24.00]Repeated line");
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[0].timestamp_ms, 12000);
        assert_eq!(lyrics.lines[0].text, "Repeated line");
        assert_eq!(lyrics.lines[1].timestamp_ms, 24000);
        assert_eq!(lyrics.lines[1].text, "Repeated line");
    }

    #[test]
    fn skip_metadata_lines() {
        let content = "[ar:Artist]\n[ti:Title]\n[al:Album]\n[00:05.00]First line";
        let lyrics = parse_lrc(content);
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].text, "First line");
    }

    #[test]
    fn empty_content_returns_empty() {
        let lyrics = parse_lrc("");
        assert!(lyrics.lines.is_empty());
    }

    #[test]
    fn current_line_index_basic() {
        let lyrics = Lyrics {
            lines: vec![
                LyricLine {
                    timestamp_ms: 5000,
                    text: "A".into(),
                },
                LyricLine {
                    timestamp_ms: 10000,
                    text: "B".into(),
                },
                LyricLine {
                    timestamp_ms: 15000,
                    text: "C".into(),
                },
            ],
            source: LyricsSource::None,
        };

        // Before any line
        assert_eq!(current_line_index(&lyrics, 0), None);
        assert_eq!(current_line_index(&lyrics, 4999), None);

        // On first line
        assert_eq!(current_line_index(&lyrics, 5000), Some(0));
        assert_eq!(current_line_index(&lyrics, 7000), Some(0));

        // On second line
        assert_eq!(current_line_index(&lyrics, 10000), Some(1));
        assert_eq!(current_line_index(&lyrics, 12000), Some(1));

        // On third line
        assert_eq!(current_line_index(&lyrics, 15000), Some(2));
        assert_eq!(current_line_index(&lyrics, 99999), Some(2));
    }

    #[test]
    fn current_line_index_empty_returns_none() {
        let lyrics = Lyrics::default();
        assert_eq!(current_line_index(&lyrics, 1000), None);
    }

    #[test]
    fn load_lyrics_nonexistent_file() {
        let track = Track::from_file(std::path::PathBuf::from("/nonexistent/path/song.mp3"));
        let lyrics = load_lyrics_for_track(&track);
        assert!(lyrics.lines.is_empty());
    }

    #[test]
    fn parse_lrc_with_three_digit_ms() {
        let lyrics = parse_lrc("[00:01.234]Precise");
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].timestamp_ms, 1234);
    }

    #[test]
    fn parse_lrc_sorted_output() {
        let content = "[00:30.00]Second\n[00:10.00]First\n[00:50.00]Third";
        let lyrics = parse_lrc(content);
        assert_eq!(lyrics.lines.len(), 3);
        assert_eq!(lyrics.lines[0].text, "First");
        assert_eq!(lyrics.lines[1].text, "Second");
        assert_eq!(lyrics.lines[2].text, "Third");
    }

    #[test]
    fn parse_lrc_empty_text_line() {
        let content = "[00:10.00]\n[00:20.00]With text";
        let lyrics = parse_lrc(content);
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[0].text, "");
        assert_eq!(lyrics.lines[1].text, "With text");
    }

    #[test]
    fn parse_lrc_single_digit_fraction() {
        let lyrics = parse_lrc("[00:05.5]Half");
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].timestamp_ms, 5500);
    }
}
