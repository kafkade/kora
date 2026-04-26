//! OPML import/export for podcast subscriptions.
//!
//! Parses and generates OPML 2.0 XML using simple string matching —
//! no external XML crate needed.

use anyhow::Result;

use super::podcast::PodcastFeed;

/// A single entry parsed from an OPML file.
#[derive(Debug, Clone, PartialEq)]
pub struct OpmlEntry {
    pub title: String,
    pub url: String,
}

/// Parse an OPML file and extract feed URLs.
///
/// Lenient: skips malformed outlines rather than failing the whole import.
/// Handles both single and double quotes, nested outline groups, and
/// outlines missing `xmlUrl` (which are treated as category folders).
pub fn import_opml(content: &str) -> Result<Vec<OpmlEntry>> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("<outline") && !trimmed.starts_with("<Outline") {
            // Also handle case-insensitive match
            let lower = trimmed.to_lowercase();
            if !lower.starts_with("<outline") {
                continue;
            }
        }

        let url = match extract_attr(trimmed, "xmlUrl")
            .or_else(|| extract_attr(trimmed, "xmlurl"))
            .or_else(|| extract_attr(trimmed, "XMLURL"))
        {
            Some(u) => u,
            None => continue, // category folder or missing URL — skip
        };

        let title = extract_attr(trimmed, "text")
            .or_else(|| extract_attr(trimmed, "title"))
            .or_else(|| extract_attr(trimmed, "Text"))
            .or_else(|| extract_attr(trimmed, "Title"))
            .unwrap_or_default();

        entries.push(OpmlEntry {
            title: unescape_xml(&title),
            url: unescape_xml(&url),
        });
    }

    Ok(entries)
}

/// Extract the value of an XML attribute from a tag string.
///
/// Handles both `attr="value"` and `attr='value'` forms.
fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    // Build search pattern: `attrName="`  or  `attrName='`
    for quote in ['"', '\''] {
        let pattern = format!("{attr_name}={quote}");
        if let Some(start) = tag.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = tag[value_start..].find(quote) {
                return Some(tag[value_start..value_start + end].to_string());
            }
        }
    }
    None
}

/// Unescape basic XML entities.
fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Escape text for use in XML attribute values.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Generate OPML 2.0 XML from a list of podcast feeds.
pub fn export_opml(feeds: &[PodcastFeed]) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<opml version=\"2.0\">\n");
    xml.push_str("  <head><title>kora Podcast Subscriptions</title></head>\n");
    xml.push_str("  <body>\n");

    for feed in feeds {
        let text = escape_xml(&feed.title);
        let url = escape_xml(&feed.url);
        xml.push_str(&format!(
            "    <outline text=\"{text}\" type=\"rss\" xmlUrl=\"{url}\" />\n"
        ));
    }

    xml.push_str("  </body>\n");
    xml.push_str("</opml>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_opml_multiple_outlines() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>My Podcasts</title></head>
  <body>
    <outline text="Podcast One" type="rss" xmlUrl="https://one.example.com/feed" />
    <outline text="Podcast Two" type="rss" xmlUrl="https://two.example.com/rss.xml" />
    <outline text="Podcast Three" type="rss" xmlUrl="https://three.example.com/feed.xml" />
  </body>
</opml>"#;

        let entries = import_opml(opml).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].title, "Podcast One");
        assert_eq!(entries[0].url, "https://one.example.com/feed");
        assert_eq!(entries[1].title, "Podcast Two");
        assert_eq!(entries[2].url, "https://three.example.com/feed.xml");
    }

    #[test]
    fn parse_nested_outline_groups() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Nested</title></head>
  <body>
    <outline text="Tech">
      <outline text="Tech Pod" type="rss" xmlUrl="https://tech.example.com/rss" />
    </outline>
    <outline text="Comedy">
      <outline text="Funny Show" type="rss" xmlUrl="https://funny.example.com/rss" />
      <outline text="Laughs" type="rss" xmlUrl="https://laughs.example.com/rss" />
    </outline>
  </body>
</opml>"#;

        let entries = import_opml(opml).unwrap();
        // Category folders (no xmlUrl) are skipped, only feeds are returned
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].title, "Tech Pod");
        assert_eq!(entries[1].title, "Funny Show");
        assert_eq!(entries[2].title, "Laughs");
    }

    #[test]
    fn parse_single_quotes() {
        let opml = r#"<?xml version='1.0'?>
<opml version='2.0'>
  <body>
    <outline text='Single Quoted' type='rss' xmlUrl='https://single.example.com/feed' />
  </body>
</opml>"#;

        let entries = import_opml(opml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Single Quoted");
        assert_eq!(entries[0].url, "https://single.example.com/feed");
    }

    #[test]
    fn parse_empty_opml_returns_empty() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Empty</title></head>
  <body>
  </body>
</opml>"#;

        let entries = import_opml(opml).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_non_xml_returns_empty() {
        let garbage = "this is not xml at all\njust random text\n{\"json\": true}";
        let entries = import_opml(garbage).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_empty_string_returns_empty() {
        let entries = import_opml("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn export_produces_valid_xml() {
        let feeds = vec![
            PodcastFeed {
                url: "https://example.com/feed1".to_string(),
                title: "Feed One".to_string(),
                description: String::new(),
            },
            PodcastFeed {
                url: "https://example.com/feed2".to_string(),
                title: "Feed Two".to_string(),
                description: "A great podcast".to_string(),
            },
        ];

        let xml = export_opml(&feeds);
        assert!(xml.contains("<?xml version=\"1.0\""));
        assert!(xml.contains("<opml version=\"2.0\">"));
        assert!(xml.contains("xmlUrl=\"https://example.com/feed1\""));
        assert!(xml.contains("text=\"Feed One\""));
        assert!(xml.contains("xmlUrl=\"https://example.com/feed2\""));
        assert!(xml.contains("text=\"Feed Two\""));
        assert!(xml.contains("type=\"rss\""));
    }

    #[test]
    fn export_escapes_xml_entities() {
        let feeds = vec![PodcastFeed {
            url: "https://example.com/feed?a=1&b=2".to_string(),
            title: "Tom & Jerry's <Show>".to_string(),
            description: String::new(),
        }];

        let xml = export_opml(&feeds);
        assert!(xml.contains("Tom &amp; Jerry's &lt;Show&gt;"));
        assert!(xml.contains("a=1&amp;b=2"));
    }

    #[test]
    fn round_trip_export_import_preserves_feeds() {
        let feeds = vec![
            PodcastFeed {
                url: "https://alpha.example.com/rss".to_string(),
                title: "Alpha Pod".to_string(),
                description: String::new(),
            },
            PodcastFeed {
                url: "https://beta.example.com/feed.xml".to_string(),
                title: "Beta Show".to_string(),
                description: "desc".to_string(),
            },
        ];

        let xml = export_opml(&feeds);
        let entries = import_opml(&xml).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Alpha Pod");
        assert_eq!(entries[0].url, "https://alpha.example.com/rss");
        assert_eq!(entries[1].title, "Beta Show");
        assert_eq!(entries[1].url, "https://beta.example.com/feed.xml");
    }

    #[test]
    fn round_trip_with_special_chars() {
        let feeds = vec![PodcastFeed {
            url: "https://example.com/rss?format=xml&lang=en".to_string(),
            title: "A & B \"Podcast\" <Special>".to_string(),
            description: String::new(),
        }];

        let xml = export_opml(&feeds);
        let entries = import_opml(&xml).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "A & B \"Podcast\" <Special>");
        assert_eq!(entries[0].url, "https://example.com/rss?format=xml&lang=en");
    }

    #[test]
    fn import_skips_outlines_without_xml_url() {
        let opml = r#"<opml version="2.0">
  <body>
    <outline text="Category Folder" />
    <outline text="Real Feed" type="rss" xmlUrl="https://real.example.com/rss" />
    <outline text="Another Category">
    </outline>
  </body>
</opml>"#;

        let entries = import_opml(opml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Real Feed");
    }

    #[test]
    fn import_handles_mixed_case_attributes() {
        let opml = r#"<opml version="2.0">
  <body>
    <outline text="Mixed Case" type="rss" xmlUrl="https://mixed.example.com/feed" />
  </body>
</opml>"#;

        let entries = import_opml(opml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://mixed.example.com/feed");
    }

    #[test]
    fn export_empty_feeds() {
        let xml = export_opml(&[]);
        assert!(xml.contains("<body>"));
        assert!(xml.contains("</body>"));
        // Should still be valid OPML, just with no outlines
        let entries = import_opml(&xml).unwrap();
        assert!(entries.is_empty());
    }
}
