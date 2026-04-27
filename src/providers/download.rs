//! Podcast episode download management — download, cleanup, and storage limits.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::providers::podcast::PodcastState;

/// Download status for an episode.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Public API — used by future download progress UI
pub enum DownloadStatus {
    NotDownloaded,
    Downloading(f32),
    Downloaded(PathBuf),
    Failed(String),
}

/// Default download directory: `<cache_dir>/kora/podcasts`.
pub fn download_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kora")
        .join("podcasts")
}

/// Replace filesystem-unsafe characters with `_` and trim whitespace.
fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "_".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Download an episode to `download_dir/feed_title/episode_title.ext`.
///
/// If the file already exists (partial or complete), the download is skipped.
/// Returns the local file path on success.
pub fn download_episode(
    url: &str,
    feed_title: &str,
    episode_title: &str,
    download_dir: &Path,
) -> Result<PathBuf> {
    let feed_dir = download_dir.join(sanitize_filename(feed_title));
    std::fs::create_dir_all(&feed_dir)
        .with_context(|| format!("Failed to create directory: {}", feed_dir.display()))?;

    // Derive extension from URL (fall back to .mp3)
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|segment| {
            // Strip query string
            let name = segment.split('?').next().unwrap_or(segment);
            let dot_pos = name.rfind('.')?;
            let ext = &name[dot_pos..];
            if ext.len() > 1 && ext.len() <= 5 {
                Some(ext.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| ".mp3".to_string());

    let filename = format!("{}{}", sanitize_filename(episode_title), ext);
    let dest = feed_dir.join(&filename);

    // Skip if file already exists (simple resume: don't re-download)
    if dest.exists()
        && std::fs::metadata(&dest)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        return Ok(dest);
    }

    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download episode from: {url}"))?;

    let bytes = response
        .bytes()
        .with_context(|| "Failed to read episode response body")?;

    std::fs::write(&dest, &bytes)
        .with_context(|| format!("Failed to write episode file: {}", dest.display()))?;

    Ok(dest)
}

/// Check if an episode is already downloaded. Returns the path if it exists.
pub fn is_downloaded(
    url: &str,
    download_dir: &Path,
    feed_title: &str,
    episode_title: &str,
) -> Option<PathBuf> {
    let feed_dir = download_dir.join(sanitize_filename(feed_title));

    // Derive extension from URL (fall back to .mp3)
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|segment| {
            let name = segment.split('?').next().unwrap_or(segment);
            let dot_pos = name.rfind('.')?;
            let ext = &name[dot_pos..];
            if ext.len() > 1 && ext.len() <= 5 {
                Some(ext.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| ".mp3".to_string());

    let filename = format!("{}{}", sanitize_filename(episode_title), ext);
    let dest = feed_dir.join(filename);

    if dest.exists()
        && std::fs::metadata(&dest)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        Some(dest)
    } else {
        None
    }
}

/// Delete a downloaded episode file.
#[allow(dead_code)] // Public API — used by future cleanup commands and tests
pub fn delete_episode(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to delete episode: {}", path.display()))?;
    }
    Ok(())
}

/// Delete downloaded files for episodes marked as played.
/// Returns the number of files deleted.
pub fn cleanup_played(state: &PodcastState, download_dir: &Path) -> Result<usize> {
    let mut deleted = 0;

    // Walk all files under download_dir and check if corresponding episode is played
    if !download_dir.exists() {
        return Ok(0);
    }

    // Collect all episode positions that indicate played (position > 0)
    let played_urls: std::collections::HashSet<&String> = state
        .episode_positions
        .iter()
        .filter(|(_, pos)| **pos > 0)
        .map(|(url, _)| url)
        .collect();

    // Check each feed's downloaded episodes
    for feed in &state.feeds {
        let feed_dir = download_dir.join(sanitize_filename(&feed.title));
        if !feed_dir.exists() {
            continue;
        }

        let entries = std::fs::read_dir(&feed_dir)
            .with_context(|| format!("Failed to read directory: {}", feed_dir.display()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                // Check if any played URL maps to this file
                // We delete files in played feeds' directories when the URL is marked played
                let should_delete = played_urls.iter().any(|url| {
                    is_downloaded(url, download_dir, &feed.title, "").is_none()
                        || is_downloaded_path_matches(url, download_dir, &feed.title, &path)
                });

                if should_delete {
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::warn!("Failed to delete {}: {e}", path.display());
                    } else {
                        deleted += 1;
                    }
                }
            }
        }
    }

    Ok(deleted)
}

/// Check if a URL's expected download path matches the given file path.
fn is_downloaded_path_matches(
    url: &str,
    download_dir: &Path,
    feed_title: &str,
    path: &Path,
) -> bool {
    let feed_dir = download_dir.join(sanitize_filename(feed_title));
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|segment| {
            let name = segment.split('?').next().unwrap_or(segment);
            let dot_pos = name.rfind('.')?;
            let ext = &name[dot_pos..];
            if ext.len() > 1 && ext.len() <= 5 {
                Some(ext.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| ".mp3".to_string());

    // We can't reconstruct the episode title from the URL alone, so match by directory
    path.starts_with(&feed_dir)
        && path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            == Some(ext)
}

/// Delete oldest files until total size is under `limit_mb`.
/// Returns the number of files deleted.
#[allow(dead_code)] // Public API — wired via config `podcast_storage_limit_mb`
pub fn enforce_storage_limit(download_dir: &Path, limit_mb: u64) -> Result<usize> {
    if !download_dir.exists() {
        return Ok(0);
    }

    let mut files = collect_files_with_metadata(download_dir)?;

    let limit_bytes = limit_mb * 1024 * 1024;
    let mut total_bytes: u64 = files.iter().map(|(_, size, _)| size).sum();

    if total_bytes <= limit_bytes {
        return Ok(0);
    }

    // Sort by modification time, oldest first
    files.sort_by_key(|(_, _, modified)| *modified);

    let mut deleted = 0;
    for (path, size, _) in &files {
        if total_bytes <= limit_bytes {
            break;
        }
        if let Err(e) = std::fs::remove_file(path) {
            tracing::warn!("Failed to delete {}: {e}", path.display());
        } else {
            total_bytes -= size;
            deleted += 1;
        }
    }

    Ok(deleted)
}

/// Total size of all downloaded episodes in MB.
#[allow(dead_code)] // Public API — used by storage limit enforcement and UI display
pub fn total_size_mb(download_dir: &Path) -> Result<f64> {
    if !download_dir.exists() {
        return Ok(0.0);
    }

    let files = collect_files_with_metadata(download_dir)?;
    let total_bytes: u64 = files.iter().map(|(_, size, _)| size).sum();
    Ok(total_bytes as f64 / (1024.0 * 1024.0))
}

/// Recursively collect all files with their size and modification time.
fn collect_files_with_metadata(dir: &Path) -> Result<Vec<(PathBuf, u64, std::time::SystemTime)>> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    dir: &Path,
    files: &mut Vec<(PathBuf, u64, std::time::SystemTime)>,
) -> Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("Failed to read dir: {}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files)?;
        } else if path.is_file()
            && let Ok(meta) = std::fs::metadata(&path)
        {
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            files.push((path, meta.len(), modified));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_special_characters() {
        assert_eq!(sanitize_filename("hello/world"), "hello_world");
        assert_eq!(sanitize_filename("a:b*c?d"), "a_b_c_d");
        assert_eq!(sanitize_filename("file<name>test"), "file_name_test");
        assert_eq!(sanitize_filename("pipe|char"), "pipe_char");
        assert_eq!(sanitize_filename("back\\slash"), "back_slash");
        assert_eq!(sanitize_filename(r#"quote"test"#), "quote_test");
    }

    #[test]
    fn sanitize_handles_empty_string() {
        assert_eq!(sanitize_filename(""), "_");
        assert_eq!(sanitize_filename("   "), "_");
    }

    #[test]
    fn sanitize_preserves_normal_characters() {
        assert_eq!(sanitize_filename("hello world"), "hello world");
        assert_eq!(sanitize_filename("episode 42"), "episode 42");
        assert_eq!(
            sanitize_filename("My Podcast - Episode 1"),
            "My Podcast - Episode 1"
        );
    }

    #[test]
    fn download_dir_returns_valid_path() {
        let dir = download_dir();
        assert!(dir.to_string_lossy().contains("kora"));
        assert!(dir.to_string_lossy().contains("podcasts"));
    }

    #[test]
    fn total_size_mb_on_empty_dir() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_download_empty");
        std::fs::create_dir_all(&dir).unwrap();

        // Remove any leftover files
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }

        let size = total_size_mb(&dir).unwrap();
        assert_eq!(size, 0.0);

        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn total_size_mb_nonexistent_dir() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("nonexistent_download_dir_xyz");
        let size = total_size_mb(&dir).unwrap();
        assert_eq!(size, 0.0);
    }

    #[test]
    fn is_downloaded_returns_none_when_missing() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_download_missing");
        std::fs::create_dir_all(&dir).unwrap();

        let result = is_downloaded(
            "https://example.com/episode.mp3",
            &dir,
            "Some Podcast",
            "Some Episode",
        );
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_downloaded_returns_path_when_exists() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_download_exists");
        let feed_dir = dir.join("Test Feed");
        std::fs::create_dir_all(&feed_dir).unwrap();

        let file_path = feed_dir.join("My Episode.mp3");
        std::fs::write(&file_path, b"fake audio data").unwrap();

        let result = is_downloaded(
            "https://example.com/ep.mp3",
            &dir,
            "Test Feed",
            "My Episode",
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), file_path);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn enforce_storage_limit_deletes_when_over_limit() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_storage_limit");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create some test files
        for i in 0..5 {
            let path = dir.join(format!("file{i}.mp3"));
            std::fs::write(&path, vec![0u8; 1024]).unwrap();
        }

        // 0 limit should delete everything
        let deleted = enforce_storage_limit(&dir, 0).unwrap();
        assert_eq!(deleted, 5);

        // Verify dir is now empty of files
        let size = total_size_mb(&dir).unwrap();
        assert_eq!(size, 0.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn enforce_storage_limit_noop_under_limit() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_storage_under_limit");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a tiny file (1KB)
        std::fs::write(dir.join("tiny.mp3"), vec![0u8; 1024]).unwrap();

        // 100 MB limit — nothing should be deleted
        let deleted = enforce_storage_limit(&dir, 100).unwrap();
        assert_eq!(deleted, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_episode_removes_file() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_delete_ep");
        std::fs::create_dir_all(&dir).unwrap();

        let file_path = dir.join("test_ep.mp3");
        std::fs::write(&file_path, b"data").unwrap();
        assert!(file_path.exists());

        delete_episode(&file_path).unwrap();
        assert!(!file_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_episode_nonexistent_is_ok() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("nonexistent_file.mp3");
        assert!(delete_episode(&path).is_ok());
    }
}
