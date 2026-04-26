//! File browser overlay for selecting audio files from the filesystem.

use std::path::{Path, PathBuf};

/// Audio file extensions supported by kora.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "wav", "opus", "aac", "m4a", "wma", "aiff",
];

/// A single entry in the file browser.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_audio: bool,
}

/// A navigable file browser for selecting audio files.
pub struct FileBrowser {
    current_dir: PathBuf,
    entries: Vec<DirEntry>,
    selected: usize,
    scroll_offset: usize,
    visible_height: usize,
}

impl FileBrowser {
    /// Create a new file browser starting at the given directory.
    pub fn new(start_dir: PathBuf) -> Self {
        let mut browser = Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible_height: 20,
        };
        browser.refresh();
        browser
    }

    /// Re-read the current directory and update entries.
    pub fn refresh(&mut self) {
        self.entries = read_directory(&self.current_dir);
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Go to the parent directory.
    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh();
        }
    }

    /// Move selection down.
    pub fn navigate_down(&mut self) {
        if !self.entries.is_empty() && self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.adjust_scroll();
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.adjust_scroll();
        }
    }

    /// Enter the selected directory, or return the selected audio file path.
    pub fn navigate_into(&mut self) -> Option<PathBuf> {
        let entry = self.entries.get(self.selected)?.clone();
        if entry.is_dir {
            self.current_dir = entry.path;
            self.refresh();
            None
        } else if entry.is_audio {
            Some(entry.path)
        } else {
            None
        }
    }

    /// Get the currently selected entry.
    #[allow(dead_code)] // Used in tests and future integration
    pub fn selected_entry(&self) -> Option<&DirEntry> {
        self.entries.get(self.selected)
    }

    /// Get all entries for display.
    pub fn entries_for_display(&self) -> &[DirEntry] {
        &self.entries
    }

    /// Get the current directory path.
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Get the scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the visible height (called before input handling for scroll accuracy).
    #[allow(dead_code)] // Used in tests and future resize handling
    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height;
        self.adjust_scroll();
    }

    fn adjust_scroll(&mut self) {
        if self.visible_height == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.visible_height {
            self.scroll_offset = self.selected - self.visible_height + 1;
        }
    }
}

fn is_audio_extension(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

fn read_directory(path: &Path) -> Vec<DirEntry> {
    let read_dir = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in read_dir.filter_map(|e| e.ok()) {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        let is_dir = entry_path.is_dir();
        let is_audio = if is_dir {
            false
        } else {
            entry_path
                .extension()
                .and_then(|e| e.to_str())
                .map(is_audio_extension)
                .unwrap_or(false)
        };

        let dir_entry = DirEntry {
            name,
            path: entry_path,
            is_dir,
            is_audio,
        };

        if is_dir {
            dirs.push(dir_entry);
        } else if is_audio {
            files.push(dir_entry);
        }
        // Non-audio files are filtered out
    }

    // Sort alphabetically within each group (case-insensitive)
    dirs.sort_by_key(|a| a.name.to_lowercase());
    files.sort_by_key(|a| a.name.to_lowercase());

    // Directories first, then audio files
    dirs.extend(files);
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_file_browser")
            .join(name)
    }

    fn setup_test_dir(name: &str) -> PathBuf {
        let dir = test_dir(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Create subdirectories
        fs::create_dir_all(dir.join("Albums")).unwrap();
        fs::create_dir_all(dir.join("Playlists")).unwrap();

        // Create audio files
        fs::write(dir.join("song1.mp3"), b"fake mp3").unwrap();
        fs::write(dir.join("song2.flac"), b"fake flac").unwrap();
        fs::write(dir.join("track.ogg"), b"fake ogg").unwrap();
        fs::write(dir.join("audio.wav"), b"fake wav").unwrap();

        // Create non-audio files (should be hidden)
        fs::write(dir.join("readme.txt"), b"text file").unwrap();
        fs::write(dir.join("cover.jpg"), b"image file").unwrap();

        // Create hidden file (should be hidden)
        fs::write(dir.join(".hidden"), b"hidden").unwrap();

        // Create files in subdirectory
        fs::write(dir.join("Albums").join("album_track.mp3"), b"fake").unwrap();

        dir
    }

    #[test]
    fn entries_listing() {
        let dir = setup_test_dir("entries_listing");
        let browser = FileBrowser::new(dir.clone());

        // Should have 2 dirs + 4 audio files = 6 entries
        assert_eq!(browser.entries_for_display().len(), 6);

        // Non-audio files and hidden files should be excluded
        let names: Vec<&str> = browser
            .entries_for_display()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(!names.contains(&"readme.txt"));
        assert!(!names.contains(&"cover.jpg"));
        assert!(!names.contains(&".hidden"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn directory_first_sorting() {
        let dir = setup_test_dir("dir_first_sort");
        let browser = FileBrowser::new(dir.clone());
        let entries = browser.entries_for_display();

        // First entries should be directories
        assert!(entries[0].is_dir);
        assert!(entries[1].is_dir);

        // Then audio files
        assert!(entries[2].is_audio);
        assert!(!entries[2].is_dir);

        // Directories sorted alphabetically
        assert_eq!(entries[0].name, "Albums");
        assert_eq!(entries[1].name, "Playlists");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audio_file_filtering() {
        let dir = setup_test_dir("audio_filter");
        let browser = FileBrowser::new(dir.clone());

        let audio_entries: Vec<&DirEntry> = browser
            .entries_for_display()
            .iter()
            .filter(|e| e.is_audio)
            .collect();
        assert_eq!(audio_entries.len(), 4);

        let audio_names: Vec<&str> = audio_entries.iter().map(|e| e.name.as_str()).collect();
        assert!(audio_names.contains(&"song1.mp3"));
        assert!(audio_names.contains(&"song2.flac"));
        assert!(audio_names.contains(&"track.ogg"));
        assert!(audio_names.contains(&"audio.wav"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn navigation_down_up() {
        let dir = setup_test_dir("nav_down_up");
        let mut browser = FileBrowser::new(dir.clone());

        assert_eq!(browser.selected_index(), 0);

        browser.navigate_down();
        assert_eq!(browser.selected_index(), 1);

        browser.navigate_down();
        assert_eq!(browser.selected_index(), 2);

        browser.select_previous();
        assert_eq!(browser.selected_index(), 1);

        // Can't go above 0
        browser.select_previous();
        browser.select_previous();
        assert_eq!(browser.selected_index(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn navigate_into_directory() {
        let dir = setup_test_dir("nav_into_dir");
        let mut browser = FileBrowser::new(dir.clone());

        // First entry should be "Albums" directory
        assert_eq!(browser.entries_for_display()[0].name, "Albums");

        // Navigate into it — returns None (directory, not a file selection)
        let result = browser.navigate_into();
        assert!(result.is_none());
        assert!(browser.current_dir().ends_with("Albums"));

        // Should have 1 audio file
        assert_eq!(browser.entries_for_display().len(), 1);
        assert_eq!(browser.entries_for_display()[0].name, "album_track.mp3");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn navigate_back_to_parent() {
        let dir = setup_test_dir("nav_parent");
        let mut browser = FileBrowser::new(dir.join("Albums"));

        assert!(browser.current_dir().ends_with("Albums"));

        browser.navigate_up();
        assert_eq!(browser.current_dir(), dir.as_path());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn navigate_into_audio_file() {
        let dir = setup_test_dir("nav_audio");
        let mut browser = FileBrowser::new(dir.clone());

        // Navigate to first audio file (skip 2 dirs)
        browser.navigate_down();
        browser.navigate_down();

        let entry = browser.selected_entry().unwrap();
        assert!(entry.is_audio);

        let result = browser.navigate_into();
        assert!(result.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_directory() {
        let dir = test_dir("empty_dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let browser = FileBrowser::new(dir.clone());
        assert!(browser.entries_for_display().is_empty());
        assert!(browser.selected_entry().is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scroll_adjustment() {
        let dir = test_dir("scroll_adj");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Create many audio files to test scrolling
        for i in 0..30 {
            fs::write(dir.join(format!("track_{i:02}.mp3")), b"fake").unwrap();
        }

        let mut browser = FileBrowser::new(dir.clone());
        browser.set_visible_height(5);

        assert_eq!(browser.scroll_offset(), 0);

        // Navigate past visible area
        for _ in 0..10 {
            browser.navigate_down();
        }

        // Scroll should have adjusted
        assert!(browser.scroll_offset() > 0);
        assert!(browser.selected_index() < browser.scroll_offset() + 5);

        let _ = fs::remove_dir_all(&dir);
    }
}
