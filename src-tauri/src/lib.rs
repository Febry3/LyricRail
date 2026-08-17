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

    #[test]
    fn settings_window_close_is_reusable() {
        let source = include_str!("../../src/App.tsx");

        assert!(source.contains("getCurrentWindow().hide()"));
        assert!(!source.contains("getCurrentWindow().close()"));
    }

    #[test]
    fn transparent_background_preference_is_wired_through_widget_and_styles() {
        let app_source = include_str!("../../src/App.tsx");
        let style_source = include_str!("../../src/App.css");

        assert!(app_source.contains("transparentBackground: boolean"));
        assert!(app_source.contains("transparentBackground: false"));
        assert!(app_source.contains("Transparent background"));
        assert!(app_source.contains("media-strip--transparent"));
        assert!(style_source.contains(".media-strip--transparent"));
    }

    #[test]
    fn taskbar_visibility_detection_is_wired_into_visibility_polling() {
        let taskbar_source = include_str!("taskbar.rs");
        let media_source = include_str!("media/session.rs");

        assert!(taskbar_source.contains("taskbar_is_visible"));
        assert!(taskbar_source.contains("IsWindowVisible"));
        assert!(media_source.contains("taskbar_is_visible"));
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
fn set_compact_expanded(
    window: tauri::WebviewWindow,
    expanded: bool,
    width: u32,
) -> Result<(), String> {
    if expanded {
        taskbar::position_expanded_window(&window)
    } else {
        taskbar::position_compact_window_with_width(&window, width)
    }
}

#[tauri::command]
fn set_compact_width(app: tauri::AppHandle, width: u32, expanded: bool) -> Result<(), String> {
    if expanded {
        return Ok(());
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_owned())?;
    taskbar::position_compact_window_with_width(&window, width)
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::clone(&media_state))
        .invoke_handler(tauri::generate_handler![
            ping,
            get_media_state,
            set_compact_expanded,
            set_compact_width,
            previous_track,
            play_pause,
            next_track
        ])
        .setup(move |app| {
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let exit_item = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
            let tray_menu = MenuBuilder::new(app)
                .items(&[&settings_item, &exit_item])
                .build()?;
            let tray_icon = Image::from_bytes(TRAY_ICON_BYTES)?;

            if let Some(settings_window) = app.get_webview_window("settings") {
                let reusable_settings_window = settings_window.clone();
                settings_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = reusable_settings_window.hide();
                    }
                });
            }

            let tray_builder = TrayIconBuilder::new()
                .menu(&tray_menu)
                .icon(tray_icon)
                .tooltip("LyricRail")
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "settings" => {
                        if let Some(window) = app.get_webview_window("settings") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "exit" => app.exit(0),
                    _ => {}
                });
            tray_builder.build(app)?;

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
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");
