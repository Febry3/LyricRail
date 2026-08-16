#[cfg(test)]
mod tests {
    use super::{
        media::{MediaState, SessionSnapshot},
        ping,
    };

    #[test]
    fn ping_returns_expected_message() {
        assert_eq!(ping(), "pong from Rust");
    }

    #[test]
    fn default_media_state_represents_no_active_session() {
        let state = MediaState::default();

        assert_eq!(state.track_id, None);
        assert_eq!(state.title, "");
        assert_eq!(state.artist, "");
        assert_eq!(state.album, None);
        assert_eq!(state.source_app, None);
        assert_eq!(state.playback_status, "NoMedia");
        assert_eq!(state.position_ms, 0);
        assert_eq!(state.duration_ms, 0);
    }

    #[test]
    fn session_snapshot_normalizes_empty_optional_metadata() {
        let state = MediaState::from_session_snapshot(SessionSnapshot {
            session_id: 42,
            title: "  A Track  ".into(),
            artist: "".into(),
            album: Some("   ".into()),
            source_app: "Spotify.exe".into(),
            playback_status: "Playing".into(),
            position_ms: -50,
            duration_ms: 210_000,
        });

        assert_eq!(state.track_id.as_deref(), Some("42"));
        assert_eq!(state.title, "A Track");
        assert_eq!(state.artist, "");
        assert_eq!(state.album, None);
        assert_eq!(state.source_app.as_deref(), Some("Spotify.exe"));
        assert_eq!(state.playback_status, "Playing");
        assert_eq!(state.position_ms, 0);
        assert_eq!(state.duration_ms, 210_000);
    }
}

#[tauri::command]
fn ping() -> &'static str {
    "pong from Rust"
}

#[tauri::command]
fn get_media_state(state: tauri::State<'_, Arc<Mutex<MediaState>>>) -> MediaState {
    match state.lock() {
        Ok(state) => state.clone(),
        Err(error) => {
            tracing::error!(%error, "media state mutex was poisoned while serving a command");
            MediaState::default()
        }
    }
}

#[tauri::command]
fn set_compact_expanded(window: tauri::WebviewWindow, expanded: bool) -> Result<(), String> {
    if expanded {
        taskbar::position_expanded_window(&window)
    } else {
        taskbar::position_compact_window(&window)
    }
}

#[tauri::command]
fn previous_track() -> Result<(), String> {
    media::controls::previous()
}

#[tauri::command]
fn play_pause() -> Result<(), String> {
    media::controls::play_pause()
}

#[tauri::command]
fn next_track() -> Result<(), String> {
    media::controls::next()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let media_state = Arc::new(Mutex::new(MediaState::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::clone(&media_state))
        .invoke_handler(tauri::generate_handler![
            ping,
            get_media_state,
            set_compact_expanded,
            previous_track,
            play_pause,
            next_track
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = taskbar::position_compact_window(&window) {
                    tracing::warn!(%error, "could not position compact taskbar window");
                }
                taskbar::install_window_persistence(window);
            }
            media::spawn_media_session_worker(app.handle().clone(), Arc::clone(&media_state));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
mod lyrics;
pub mod media;
mod taskbar;

use std::sync::{Arc, Mutex};

use media::MediaState;
use tauri::Manager;
