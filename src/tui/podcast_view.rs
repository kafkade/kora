//! Podcast browser overlay — browse subscribed feeds and their episodes.

use crate::core::track::Track;
use crate::providers::podcast::{
    PodcastEpisode, PodcastFeed, PodcastState, episode_to_track, fetch_feed, save_state,
};

/// A feed bundled with its fetched episodes.
pub struct PodcastFeedWithEpisodes {
    pub feed: PodcastFeed,
    pub episodes: Vec<PodcastEpisode>,
    pub episode_count: usize,
}

/// Current browsing mode inside the podcast view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodcastViewMode {
    /// Browsing the list of subscribed feeds.
    FeedList,
    /// Browsing episodes within a selected feed.
    EpisodeList,
}

/// Input mode for text entry (adding a feed URL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    /// Typing a feed URL.
    AddingFeed,
}

/// Podcast browser component for the TUI overlay.
pub struct PodcastView {
    feeds: Vec<PodcastFeedWithEpisodes>,
    selected_feed: usize,
    selected_episode: usize,
    mode: PodcastViewMode,
    status_message: Option<String>,
    input_mode: InputMode,
    input_buffer: String,
    scroll_offset: usize,
    visible_height: usize,
    /// The underlying podcast state for persistence.
    state: PodcastState,
    /// Whether feeds have been refreshed at least once since opening.
    refreshed: bool,
}

impl PodcastView {
    /// Initialize from saved podcast state.
    pub fn new(state: &PodcastState) -> Self {
        let feeds: Vec<PodcastFeedWithEpisodes> = state
            .feeds
            .iter()
            .map(|f| PodcastFeedWithEpisodes {
                feed: f.clone(),
                episodes: Vec::new(),
                episode_count: 0,
            })
            .collect();

        Self {
            feeds,
            selected_feed: 0,
            selected_episode: 0,
            mode: PodcastViewMode::FeedList,
            status_message: None,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            scroll_offset: 0,
            visible_height: 20,
            state: state.clone(),
            refreshed: false,
        }
    }

    /// Lazy-refresh all feeds on first open.
    pub fn ensure_refreshed(&mut self) {
        if !self.refreshed {
            self.refreshed = true;
            self.refresh_all();
        }
    }

    /// Re-fetch the RSS feed at the given index.
    pub fn refresh_feed(&mut self, index: usize) {
        if let Some(entry) = self.feeds.get_mut(index) {
            match fetch_feed(&entry.feed.url) {
                Ok((feed, episodes)) => {
                    entry.episode_count = episodes.len();
                    entry.episodes = episodes;
                    entry.feed.title = feed.title;
                    entry.feed.description = feed.description;
                    // Restore played/position state from persisted data
                    for ep in &mut entry.episodes {
                        if let Some(&pos) = self.state.episode_positions.get(&ep.audio_url) {
                            ep.position_ms = pos;
                            if pos > 0 {
                                ep.played = true;
                            }
                        }
                    }
                    self.status_message = Some(format!("Refreshed: {}", entry.feed.title));
                    // Sync feed title back to state
                    if let Some(sf) = self.state.feeds.get_mut(index) {
                        sf.title = entry.feed.title.clone();
                        sf.description = entry.feed.description.clone();
                    }
                }
                Err(e) => {
                    self.status_message = Some(format!("Error: {e}"));
                }
            }
        }
    }

    /// Refresh all subscribed feeds.
    pub fn refresh_all(&mut self) {
        let count = self.feeds.len();
        if count == 0 {
            self.status_message = Some("No feeds to refresh".to_string());
            return;
        }
        self.status_message = Some("Refreshing all feeds...".to_string());
        for i in 0..count {
            self.refresh_feed(i);
        }
        self.status_message = Some(format!("Refreshed {count} feed(s)"));
    }

    /// Add a new feed by URL — fetches, parses, and appends.
    pub fn add_feed(&mut self, url: &str) {
        // Check for duplicate
        if self.feeds.iter().any(|f| f.feed.url == url) {
            self.status_message = Some("Feed already subscribed".to_string());
            return;
        }

        self.status_message = Some("Fetching feed...".to_string());
        match fetch_feed(url) {
            Ok((feed, episodes)) => {
                let episode_count = episodes.len();
                let title = feed.title.clone();
                self.feeds.push(PodcastFeedWithEpisodes {
                    feed: feed.clone(),
                    episodes,
                    episode_count,
                });
                self.state.feeds.push(feed);
                if let Err(e) = save_state(&self.state) {
                    tracing::warn!("Failed to save podcast state: {e}");
                }
                self.status_message = Some(format!("Added: {title}"));
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {e}"));
            }
        }
    }

    /// Remove the feed at the given index.
    pub fn remove_feed(&mut self, index: usize) {
        if index < self.feeds.len() {
            let title = self.feeds[index].feed.title.clone();
            self.feeds.remove(index);
            self.state.feeds.remove(index);
            if let Err(e) = save_state(&self.state) {
                tracing::warn!("Failed to save podcast state: {e}");
            }
            // Clamp selection
            if self.selected_feed >= self.feeds.len() && !self.feeds.is_empty() {
                self.selected_feed = self.feeds.len() - 1;
            }
            self.status_message = Some(format!("Removed: {title}"));
        }
    }

    /// Convert the selected episode to a playable Track.
    pub fn selected_episode_track(&self) -> Option<Track> {
        if self.mode != PodcastViewMode::EpisodeList {
            return None;
        }
        let feed = self.feeds.get(self.selected_feed)?;
        let episode = feed.episodes.get(self.selected_episode)?;
        Some(episode_to_track(episode))
    }

    /// Move selection up.
    pub fn select_up(&mut self) {
        match self.mode {
            PodcastViewMode::FeedList => {
                if self.selected_feed > 0 {
                    self.selected_feed -= 1;
                    self.adjust_scroll();
                }
            }
            PodcastViewMode::EpisodeList => {
                if self.selected_episode > 0 {
                    self.selected_episode -= 1;
                    self.adjust_scroll();
                }
            }
        }
    }

    /// Move selection down.
    pub fn select_down(&mut self) {
        match self.mode {
            PodcastViewMode::FeedList => {
                if !self.feeds.is_empty() && self.selected_feed + 1 < self.feeds.len() {
                    self.selected_feed += 1;
                    self.adjust_scroll();
                }
            }
            PodcastViewMode::EpisodeList => {
                if let Some(feed) = self.feeds.get(self.selected_feed)
                    && !feed.episodes.is_empty()
                    && self.selected_episode + 1 < feed.episodes.len()
                {
                    self.selected_episode += 1;
                    self.adjust_scroll();
                }
            }
        }
    }

    /// Enter: drill into feed (show episodes) or select episode for playback.
    /// Returns `true` if an episode was selected (caller should play it).
    pub fn enter(&mut self) -> bool {
        match self.mode {
            PodcastViewMode::FeedList => {
                if !self.feeds.is_empty() {
                    self.mode = PodcastViewMode::EpisodeList;
                    self.selected_episode = 0;
                    self.scroll_offset = 0;
                }
                false
            }
            PodcastViewMode::EpisodeList => {
                // Signal that an episode was selected
                self.selected_episode_track().is_some()
            }
        }
    }

    /// Go back: from episodes to feed list, or signal close.
    /// Returns `true` if the view should be closed.
    pub fn back(&mut self) -> bool {
        match self.mode {
            PodcastViewMode::EpisodeList => {
                self.mode = PodcastViewMode::FeedList;
                self.scroll_offset = 0;
                false
            }
            PodcastViewMode::FeedList => true,
        }
    }

    /// Start the add-feed text input mode.
    pub fn start_add_feed(&mut self) {
        self.input_mode = InputMode::AddingFeed;
        self.input_buffer.clear();
    }

    /// Cancel text input.
    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    /// Submit the current input buffer as a feed URL.
    pub fn submit_input(&mut self) {
        if self.input_mode == InputMode::AddingFeed && !self.input_buffer.is_empty() {
            let url = self.input_buffer.clone();
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            self.add_feed(&url);
        } else {
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
        }
    }

    /// Append a character to the input buffer.
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// Delete the last character from the input buffer.
    pub fn input_backspace(&mut self) {
        self.input_buffer.pop();
    }

    // --- Accessors for rendering ---

    pub fn feeds(&self) -> &[PodcastFeedWithEpisodes] {
        &self.feeds
    }

    pub fn selected_feed_index(&self) -> usize {
        self.selected_feed
    }

    pub fn selected_episode_index(&self) -> usize {
        self.selected_episode
    }

    pub fn mode(&self) -> PodcastViewMode {
        self.mode
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    #[allow(dead_code)] // Used for future resize handling and dead_code lint
    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height;
    }

    fn selected_index(&self) -> usize {
        match self.mode {
            PodcastViewMode::FeedList => self.selected_feed,
            PodcastViewMode::EpisodeList => self.selected_episode,
        }
    }

    fn adjust_scroll(&mut self) {
        if self.visible_height == 0 {
            return;
        }
        let sel = self.selected_index();
        if sel < self.scroll_offset {
            self.scroll_offset = sel;
        } else if sel >= self.scroll_offset + self.visible_height {
            self.scroll_offset = sel - self.visible_height + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_state() -> PodcastState {
        PodcastState::default()
    }

    fn state_with_feeds() -> PodcastState {
        PodcastState {
            feeds: vec![
                PodcastFeed {
                    url: "https://example.com/feed1.rss".to_string(),
                    title: "Feed One".to_string(),
                    description: "First feed".to_string(),
                },
                PodcastFeed {
                    url: "https://example.com/feed2.rss".to_string(),
                    title: "Feed Two".to_string(),
                    description: "Second feed".to_string(),
                },
                PodcastFeed {
                    url: "https://example.com/feed3.rss".to_string(),
                    title: "Feed Three".to_string(),
                    description: "Third feed".to_string(),
                },
            ],
            episode_positions: HashMap::new(),
        }
    }

    #[test]
    fn new_with_empty_state() {
        let view = PodcastView::new(&empty_state());
        assert!(view.feeds().is_empty());
        assert_eq!(view.selected_feed_index(), 0);
        assert_eq!(view.selected_episode_index(), 0);
        assert_eq!(view.mode(), PodcastViewMode::FeedList);
        assert!(view.status_message().is_none());
        assert_eq!(view.input_mode(), InputMode::Normal);
    }

    #[test]
    fn new_loads_feeds_from_state() {
        let state = state_with_feeds();
        let view = PodcastView::new(&state);
        assert_eq!(view.feeds().len(), 3);
        assert_eq!(view.feeds()[0].feed.title, "Feed One");
        assert_eq!(view.feeds()[1].feed.title, "Feed Two");
        assert_eq!(view.feeds()[2].feed.title, "Feed Three");
        // Episodes are empty until refresh
        assert!(view.feeds()[0].episodes.is_empty());
    }

    #[test]
    fn navigation_clamps_at_boundaries() {
        let state = state_with_feeds();
        let mut view = PodcastView::new(&state);

        // Already at 0 — up should stay at 0
        view.select_up();
        assert_eq!(view.selected_feed_index(), 0);

        // Go to end
        view.select_down();
        view.select_down();
        assert_eq!(view.selected_feed_index(), 2);

        // Down past end should clamp
        view.select_down();
        assert_eq!(view.selected_feed_index(), 2);
    }

    #[test]
    fn navigation_up_down() {
        let state = state_with_feeds();
        let mut view = PodcastView::new(&state);

        assert_eq!(view.selected_feed_index(), 0);
        view.select_down();
        assert_eq!(view.selected_feed_index(), 1);
        view.select_down();
        assert_eq!(view.selected_feed_index(), 2);
        view.select_up();
        assert_eq!(view.selected_feed_index(), 1);
        view.select_up();
        assert_eq!(view.selected_feed_index(), 0);
    }

    #[test]
    fn mode_switches_on_enter_and_back() {
        let state = state_with_feeds();
        let mut view = PodcastView::new(&state);

        assert_eq!(view.mode(), PodcastViewMode::FeedList);

        // Enter drills into episodes
        let played = view.enter();
        assert!(!played);
        assert_eq!(view.mode(), PodcastViewMode::EpisodeList);

        // Back returns to feed list
        let should_close = view.back();
        assert!(!should_close);
        assert_eq!(view.mode(), PodcastViewMode::FeedList);

        // Back again in feed list means close
        let should_close = view.back();
        assert!(should_close);
    }

    #[test]
    fn enter_on_empty_feed_list_stays_in_feed_list() {
        let mut view = PodcastView::new(&empty_state());
        let played = view.enter();
        assert!(!played);
        assert_eq!(view.mode(), PodcastViewMode::FeedList);
    }

    #[test]
    fn input_mode_add_feed() {
        let mut view = PodcastView::new(&empty_state());

        view.start_add_feed();
        assert_eq!(view.input_mode(), InputMode::AddingFeed);
        assert!(view.input_buffer().is_empty());

        view.input_char('h');
        view.input_char('t');
        view.input_char('t');
        view.input_char('p');
        assert_eq!(view.input_buffer(), "http");

        view.input_backspace();
        assert_eq!(view.input_buffer(), "htt");

        view.cancel_input();
        assert_eq!(view.input_mode(), InputMode::Normal);
        assert!(view.input_buffer().is_empty());
    }

    #[test]
    fn selected_episode_track_returns_none_in_feed_list() {
        let state = state_with_feeds();
        let view = PodcastView::new(&state);
        assert!(view.selected_episode_track().is_none());
    }

    #[test]
    fn remove_feed_clamps_selection() {
        let state = state_with_feeds();
        let mut view = PodcastView::new(&state);

        // Select last feed
        view.select_down();
        view.select_down();
        assert_eq!(view.selected_feed_index(), 2);

        // Remove last — selection should clamp to new last
        view.remove_feed(2);
        assert_eq!(view.feeds().len(), 2);
        assert_eq!(view.selected_feed_index(), 1);
    }
}
