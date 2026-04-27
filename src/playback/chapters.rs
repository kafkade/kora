//! Podcast chapter support — parse and navigate chapter markers.

use serde::{Deserialize, Serialize};

/// A chapter marker in a podcast episode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub start_ms: u64,
    #[serde(default)]
    pub end_ms: Option<u64>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Parse a Podlove Simple Chapters (PSC) XML block.
///
/// Looks for `<psc:chapter>` or `<chapter>` elements with `start` and `title`
/// attributes. Handles both self-closing and paired tags.
pub fn parse_psc_chapters(xml: &str) -> Vec<Chapter> {
    let mut chapters = Vec::new();

    for line in xml.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("chapter") && !trimmed.contains("Chapter") {
            continue;
        }
        // Skip container tags like <psc:chapters> or <chapters>
        if trimmed.starts_with("<psc:chapters") || trimmed.starts_with("<chapters") {
            continue;
        }
        if trimmed.starts_with("</") {
            continue;
        }

        let start = extract_attr(trimmed, "start");
        let title = extract_attr(trimmed, "title");

        if let (Some(start_str), Some(title_str)) = (start, title)
            && let Some(start_ms) = parse_timestamp(&start_str)
        {
            chapters.push(Chapter {
                title: xml_unescape(&title_str),
                start_ms,
                end_ms: None,
                url: extract_attr(trimmed, "href").or_else(|| extract_attr(trimmed, "url")),
            });
        }
    }

    // Set end_ms from the next chapter's start
    for i in 0..chapters.len().saturating_sub(1) {
        chapters[i].end_ms = Some(chapters[i + 1].start_ms);
    }

    chapters.sort_by_key(|c| c.start_ms);
    chapters
}

/// Parse a timestamp like "HH:MM:SS", "MM:SS", or "HH:MM:SS.mmm" into milliseconds.
pub fn parse_timestamp(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        2 => {
            // MM:SS or MM:SS.mmm
            let mins: u64 = parts[0].parse().ok()?;
            let secs = parse_seconds(parts[1])?;
            Some(mins * 60_000 + secs)
        }
        3 => {
            // HH:MM:SS or HH:MM:SS.mmm
            let hours: u64 = parts[0].parse().ok()?;
            let mins: u64 = parts[1].parse().ok()?;
            let secs = parse_seconds(parts[2])?;
            Some(hours * 3_600_000 + mins * 60_000 + secs)
        }
        _ => None,
    }
}

fn parse_seconds(s: &str) -> Option<u64> {
    if let Some((whole, frac)) = s.split_once('.') {
        let secs: u64 = whole.parse().ok()?;
        let ms: u64 = match frac.len() {
            1 => frac.parse::<u64>().ok()? * 100,
            2 => frac.parse::<u64>().ok()? * 10,
            3 => frac.parse::<u64>().ok()?,
            _ => frac[..3].parse::<u64>().ok()?,
        };
        Some(secs * 1000 + ms)
    } else {
        let secs: u64 = s.parse().ok()?;
        Some(secs * 1000)
    }
}

fn extract_attr(s: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!("{name}={quote}");
        if let Some(start) = s.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = s[value_start..].find(quote) {
                return Some(s[value_start..value_start + end].to_string());
            }
        }
    }
    None
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Find the current chapter index for a given playback position.
pub fn current_chapter_index(chapters: &[Chapter], position_ms: u64) -> Option<usize> {
    if chapters.is_empty() {
        return None;
    }
    // Find the last chapter whose start_ms <= position
    let mut result = None;
    for (i, ch) in chapters.iter().enumerate() {
        if ch.start_ms <= position_ms {
            result = Some(i);
        } else {
            break;
        }
    }
    result
}

/// Format a chapter display string: "Ch 2/5: Introduction"
pub fn format_chapter_display(chapters: &[Chapter], position_ms: u64) -> Option<String> {
    let idx = current_chapter_index(chapters, position_ms)?;
    let ch = &chapters[idx];
    Some(format!("Ch {}/{}: {}", idx + 1, chapters.len(), ch.title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timestamp_mmss() {
        assert_eq!(parse_timestamp("01:30"), Some(90_000));
        assert_eq!(parse_timestamp("00:00"), Some(0));
        assert_eq!(parse_timestamp("10:05"), Some(605_000));
    }

    #[test]
    fn parse_timestamp_hhmmss() {
        assert_eq!(parse_timestamp("00:01:30"), Some(90_000));
        assert_eq!(parse_timestamp("01:00:00"), Some(3_600_000));
        assert_eq!(parse_timestamp("02:30:15"), Some(9_015_000));
    }

    #[test]
    fn parse_timestamp_with_millis() {
        assert_eq!(parse_timestamp("00:01:30.500"), Some(90_500));
        assert_eq!(parse_timestamp("01:30.5"), Some(90_500));
        assert_eq!(parse_timestamp("00:00.123"), Some(123));
    }

    #[test]
    fn parse_timestamp_invalid() {
        assert_eq!(parse_timestamp(""), None);
        assert_eq!(parse_timestamp("abc"), None);
        assert_eq!(parse_timestamp("1:2:3:4"), None);
    }

    #[test]
    fn parse_psc_basic() {
        let xml = r#"
        <psc:chapters>
            <psc:chapter start="00:00:00" title="Intro"/>
            <psc:chapter start="00:05:30" title="Interview"/>
            <psc:chapter start="00:45:00" title="Outro"/>
        </psc:chapters>
        "#;
        let chapters = parse_psc_chapters(xml);
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].title, "Intro");
        assert_eq!(chapters[0].start_ms, 0);
        assert_eq!(chapters[1].title, "Interview");
        assert_eq!(chapters[1].start_ms, 330_000);
        assert_eq!(chapters[2].title, "Outro");
        assert_eq!(chapters[2].start_ms, 2_700_000);
    }

    #[test]
    fn parse_psc_with_href() {
        let xml = r#"<psc:chapter start="00:10:00" title="Topic" href="https://example.com"/>"#;
        let chapters = parse_psc_chapters(xml);
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].url, Some("https://example.com".to_string()));
    }

    #[test]
    fn parse_psc_with_ampersand() {
        let xml = r#"<psc:chapter start="00:00:00" title="Tom &amp; Jerry"/>"#;
        let chapters = parse_psc_chapters(xml);
        assert_eq!(chapters[0].title, "Tom & Jerry");
    }

    #[test]
    fn parse_psc_end_ms_set_from_next() {
        let xml = r#"
            <psc:chapter start="00:00:00" title="A"/>
            <psc:chapter start="00:10:00" title="B"/>
        "#;
        let chapters = parse_psc_chapters(xml);
        assert_eq!(chapters[0].end_ms, Some(600_000));
        assert_eq!(chapters[1].end_ms, None);
    }

    #[test]
    fn parse_psc_empty() {
        assert!(parse_psc_chapters("").is_empty());
        assert!(parse_psc_chapters("<psc:chapters></psc:chapters>").is_empty());
    }

    #[test]
    fn current_chapter_index_works() {
        let chapters = vec![
            Chapter {
                title: "A".into(),
                start_ms: 0,
                end_ms: Some(60_000),
                url: None,
            },
            Chapter {
                title: "B".into(),
                start_ms: 60_000,
                end_ms: Some(120_000),
                url: None,
            },
            Chapter {
                title: "C".into(),
                start_ms: 120_000,
                end_ms: None,
                url: None,
            },
        ];
        assert_eq!(current_chapter_index(&chapters, 0), Some(0));
        assert_eq!(current_chapter_index(&chapters, 30_000), Some(0));
        assert_eq!(current_chapter_index(&chapters, 60_000), Some(1));
        assert_eq!(current_chapter_index(&chapters, 90_000), Some(1));
        assert_eq!(current_chapter_index(&chapters, 120_000), Some(2));
        assert_eq!(current_chapter_index(&chapters, 999_999), Some(2));
    }

    #[test]
    fn current_chapter_index_empty() {
        assert_eq!(current_chapter_index(&[], 0), None);
    }

    #[test]
    fn format_chapter_display_works() {
        let chapters = vec![
            Chapter {
                title: "Intro".into(),
                start_ms: 0,
                end_ms: Some(60_000),
                url: None,
            },
            Chapter {
                title: "Main".into(),
                start_ms: 60_000,
                end_ms: None,
                url: None,
            },
        ];
        assert_eq!(
            format_chapter_display(&chapters, 30_000),
            Some("Ch 1/2: Intro".to_string())
        );
        assert_eq!(
            format_chapter_display(&chapters, 90_000),
            Some("Ch 2/2: Main".to_string())
        );
        assert_eq!(format_chapter_display(&[], 0), None);
    }
}
