//! Cross-platform media key integration via souvlaki (MPRIS/SMTC/MediaRemote).
//!
//! This module bridges the OS media controls to kora's playback commands.
//! It is gated behind the `media-controls` feature flag so the player
//! works even on systems without D-Bus or other platform requirements.

use std::sync::mpsc;
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::playback::player::PlayerCommand;

/// On Windows, retrieve the console window handle for SMTC integration.
#[cfg(target_os = "windows")]
fn get_console_hwnd() -> Option<*mut std::ffi::c_void> {
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }
    let hwnd = unsafe { GetConsoleWindow() };
    if hwnd.is_null() { None } else { Some(hwnd) }
}

/// Initialize platform media controls and return a receiver for media key events.
///
/// Returns `None` if initialization fails (e.g. no D-Bus on Linux, headless server).
pub fn init_media_controls() -> Option<(MediaControls, mpsc::Receiver<MediaControlEvent>)> {
    let (tx, rx) = mpsc::channel();

    #[cfg(target_os = "windows")]
    let hwnd = get_console_hwnd();
    #[cfg(not(target_os = "windows"))]
    let hwnd: Option<*mut std::ffi::c_void> = None;

    let config = PlatformConfig {
        dbus_name: "kora",
        display_name: "Kora",
        hwnd,
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Media controls unavailable: {e:?}");
            return None;
        }
    };

    if let Err(e) = controls.attach(move |event: MediaControlEvent| {
        let _ = tx.send(event);
    }) {
        tracing::warn!("Failed to attach media controls handler: {e:?}");
        return None;
    }

    Some((controls, rx))
}

/// Update the OS now-playing metadata display.
pub fn update_metadata(
    controls: &mut MediaControls,
    title: Option<&str>,
    artist: Option<&str>,
    album: Option<&str>,
    duration: Option<Duration>,
) {
    let metadata = MediaMetadata {
        title,
        artist,
        album,
        cover_url: None,
        duration,
    };
    if let Err(e) = controls.set_metadata(metadata) {
        tracing::debug!("Failed to update media metadata: {e:?}");
    }
}

/// Update the OS playback state indicator.
pub fn update_playback(controls: &mut MediaControls, playing: bool, position: Option<Duration>) {
    let progress = position.map(MediaPosition);
    let playback = if playing {
        MediaPlayback::Playing { progress }
    } else {
        MediaPlayback::Paused { progress }
    };
    if let Err(e) = controls.set_playback(playback) {
        tracing::debug!("Failed to update media playback state: {e:?}");
    }
}

/// Map a souvlaki `MediaControlEvent` to a kora `PlayerCommand`.
///
/// Returns `None` for events we don't handle (Raise, OpenUri, etc.).
pub fn map_media_event(event: &MediaControlEvent) -> Option<PlayerCommand> {
    match event {
        MediaControlEvent::Play | MediaControlEvent::Pause | MediaControlEvent::Toggle => {
            Some(PlayerCommand::PlayPause)
        }
        MediaControlEvent::Next => Some(PlayerCommand::NextTrack),
        MediaControlEvent::Previous => Some(PlayerCommand::PrevTrack),
        MediaControlEvent::Stop => Some(PlayerCommand::Stop),
        MediaControlEvent::Quit => Some(PlayerCommand::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use souvlaki::MediaControlEvent;

    #[test]
    fn test_map_play_events() {
        assert!(matches!(
            map_media_event(&MediaControlEvent::Play),
            Some(PlayerCommand::PlayPause)
        ));
        assert!(matches!(
            map_media_event(&MediaControlEvent::Pause),
            Some(PlayerCommand::PlayPause)
        ));
        assert!(matches!(
            map_media_event(&MediaControlEvent::Toggle),
            Some(PlayerCommand::PlayPause)
        ));
    }

    #[test]
    fn test_map_navigation_events() {
        assert!(matches!(
            map_media_event(&MediaControlEvent::Next),
            Some(PlayerCommand::NextTrack)
        ));
        assert!(matches!(
            map_media_event(&MediaControlEvent::Previous),
            Some(PlayerCommand::PrevTrack)
        ));
    }

    #[test]
    fn test_map_stop_and_quit() {
        assert!(matches!(
            map_media_event(&MediaControlEvent::Stop),
            Some(PlayerCommand::Stop)
        ));
        assert!(matches!(
            map_media_event(&MediaControlEvent::Quit),
            Some(PlayerCommand::Quit)
        ));
    }

    #[test]
    fn test_map_unhandled_events() {
        assert!(map_media_event(&MediaControlEvent::Raise).is_none());
    }
}
