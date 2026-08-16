use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use gsmtc::{ManagerEvent, SessionModel, SessionUpdateEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};

use crate::lyrics::{fetch_synced_lyrics, synchronize, LyricLine, LyricsState, LyricsStatus};

const WIDGET_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const VISIBILITY_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaStateErrorCode {
    SessionManagerUnavailable,
    SessionManagerDisconnected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStateError {
    pub code: MediaStateErrorCode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub track_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub source_app: Option<String>,
    pub playback_status: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub artwork_url: Option<String>,
    pub lyrics: LyricsState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MediaStateError>,
}

impl Default for MediaState {
    fn default() -> Self {
        Self {
            track_id: None,
            title: String::new(),
            artist: String::new(),
            album: None,
            source_app: None,
            playback_status: "NoMedia".into(),
            position_ms: 0,
            duration_ms: 0,
            artwork_url: None,
            lyrics: LyricsState::default(),
            error: None,
        }
    }
}

/// A platform-independent input used to normalize incomplete session data.
#[derive(Debug)]
pub struct SessionSnapshot {
    pub session_id: usize,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub source_app: String,
    pub playback_status: String,
    pub position_ms: i64,
    pub duration_ms: i64,
}

impl MediaState {
    fn with_error(code: MediaStateErrorCode) -> Self {
        Self {
            error: Some(MediaStateError { code }),
            ..Self::default()
        }
    }

    pub fn session_manager_unavailable() -> Self {
        Self::with_error(MediaStateErrorCode::SessionManagerUnavailable)
    }

    pub fn session_manager_disconnected() -> Self {
        Self::with_error(MediaStateErrorCode::SessionManagerDisconnected)
    }

    pub fn from_session_snapshot(snapshot: SessionSnapshot) -> Self {
        Self {
            track_id: Some(snapshot.session_id.to_string()),
            title: snapshot.title.trim().to_owned(),
            artist: snapshot.artist.trim().to_owned(),
            album: optional_text(snapshot.album),
            source_app: optional_text(Some(snapshot.source_app)),
            playback_status: non_empty_or(snapshot.playback_status, "Unknown"),
            position_ms: milliseconds(snapshot.position_ms),
            duration_ms: milliseconds(snapshot.duration_ms),
            artwork_url: None,
            lyrics: LyricsState::default(),
            error: None,
        }
    }
}

#[derive(Default)]
struct WorkerState {
    active_session_id: Option<usize>,
    supported_session_ids: HashSet<usize>,
    sessions: HashMap<usize, MediaState>,
    lyrics_lines: HashMap<usize, Vec<LyricLine>>,
    lyrics_generations: HashMap<usize, u64>,
    timeline_samples: HashMap<usize, TimelineSample>,
    no_active_track_since: Option<Instant>,
    widget_hidden: bool,
}

#[derive(Clone, Copy)]
struct TimelineSample {
    position_ms: u64,
    sampled_at: Instant,
}

struct LyricsRequest {
    session_id: usize,
    generation: u64,
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: u64,
}

pub fn spawn_media_session_worker(app: AppHandle, state: Arc<Mutex<MediaState>>) {
    tauri::async_runtime::spawn(async move {
        let mut manager_events = match gsmtc::SessionManager::create().await {
            Ok(events) => {
                info!("Windows media session manager started");
                events
            }
            Err(error) => {
                warn!(%error, "Windows media session manager is unavailable");
                publish_state(&app, &state, MediaState::session_manager_unavailable());
                return;
            }
        };

        let worker_state = Arc::new(Mutex::new(WorkerState::default()));
        let ticker_app = app.clone();
        let ticker_state = Arc::clone(&state);
        let ticker_worker_state = Arc::clone(&worker_state);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                tick_active_lyrics(&ticker_app, &ticker_state, &ticker_worker_state);
            }
        });

        let visibility_app = app.clone();
        let visibility_worker_state = Arc::clone(&worker_state);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(VISIBILITY_POLL_INTERVAL).await;
                let should_hide = with_worker_state(&visibility_worker_state, |worker| {
                    should_hide_idle_widget(worker, Instant::now())
                })
                .unwrap_or(false);

                if should_hide {
                    set_widget_visibility(&visibility_app, false);
                }
            }
        });

        while let Some(event) = manager_events.recv().await {
            match event {
                ManagerEvent::SessionCreated {
                    session_id,
                    mut rx,
                    source,
                } => {
                    if !is_supported_media_source(&source) {
                        info!(session_id, source = %source, "ignoring unsupported media session");
                        continue;
                    }

                    let _ = with_worker_state(&worker_state, |worker| {
                        worker.supported_session_ids.insert(session_id);
                    });
                    info!(session_id, source = %source, "media session detected");
                    let app = app.clone();
                    let state = Arc::clone(&state);
                    let worker_state = Arc::clone(&worker_state);

                    tauri::async_runtime::spawn(async move {
                        while let Some(event) = rx.recv().await {
                            let (model, artwork_url) = match event {
                                SessionUpdateEvent::Model(model) => (model, None),
                                SessionUpdateEvent::Media(model, image) => {
                                    (model, Some(image.map(|image| encode_artwork_url(&image))))
                                }
                            };
                            update_session(
                                &app,
                                &state,
                                &worker_state,
                                session_id,
                                model,
                                artwork_url,
                            );
                        }

                        debug!(session_id, "media session update stream ended");
                    });
                }
                ManagerEvent::SessionRemoved { session_id } => {
                    let should_clear = with_worker_state(&worker_state, |worker| {
                        worker.sessions.remove(&session_id);
                        worker.supported_session_ids.remove(&session_id);
                        worker.lyrics_lines.remove(&session_id);
                        worker.lyrics_generations.remove(&session_id);
                        worker.timeline_samples.remove(&session_id);
                        if worker.active_session_id == Some(session_id) {
                            worker.active_session_id = None;
                            worker
                                .no_active_track_since
                                .get_or_insert_with(Instant::now);
                            true
                        } else {
                            false
                        }
                    });

                    info!(session_id, "media session removed");
                    if should_clear.unwrap_or(false) {
                        publish_state(&app, &state, MediaState::default());
                    }
                }
                ManagerEvent::CurrentSessionChanged { session_id } => {
                    let activation = with_worker_state(&worker_state, |worker| {
                        let supported_session_id =
                            session_id.filter(|id| worker.supported_session_ids.contains(id));
                        worker.active_session_id = supported_session_id;
                        let Some(session_id) = supported_session_id else {
                            worker
                                .no_active_track_since
                                .get_or_insert_with(Instant::now);
                            return (None, None, false);
                        };

                        let mut current_state = worker
                            .sessions
                            .get(&session_id)
                            .cloned()
                            .unwrap_or_default();
                        let lyrics_request = if current_state.track_id.is_some()
                            && !current_state.title.is_empty()
                            && !current_state.artist.is_empty()
                            && !worker.lyrics_lines.contains_key(&session_id)
                        {
                            let generation = next_lyrics_generation(worker, session_id);
                            current_state.lyrics = LyricsState::loading();
                            worker.sessions.insert(session_id, current_state.clone());
                            Some(lyrics_request_from_state(
                                session_id,
                                generation,
                                &current_state,
                            ))
                        } else {
                            None
                        };

                        let should_show = if !current_state.title.is_empty() {
                            worker.no_active_track_since = None;
                            if worker.widget_hidden {
                                worker.widget_hidden = false;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        (Some(current_state), lyrics_request, should_show)
                    });

                    match session_id {
                        Some(session_id) => {
                            info!(session_id, "active media session changed");
                            let (current_state, lyrics_request) = activation
                                .ok()
                                .map(|(state, request, should_show)| {
                                    if should_show {
                                        set_widget_visibility(&app, true);
                                    }
                                    (state, request)
                                })
                                .unwrap_or((None, None));
                            let current_state = current_state.unwrap_or_default();
                            publish_state(&app, &state, current_state);
                            if let Some(request) = lyrics_request {
                                spawn_lyrics_fetch(
                                    app.clone(),
                                    Arc::clone(&state),
                                    Arc::clone(&worker_state),
                                    request,
                                );
                            }
                        }
                        None => {
                            info!("no active media session");
                            publish_state(&app, &state, MediaState::default());
                        }
                    }
                }
            }
        }

        warn!("Windows media session manager event stream terminated");
        publish_state(&app, &state, MediaState::session_manager_disconnected());
    });
}

fn update_session(
    app: &AppHandle,
    state: &Arc<Mutex<MediaState>>,
    worker_state: &Arc<Mutex<WorkerState>>,
    session_id: usize,
    model: SessionModel,
    artwork_url: Option<Option<String>>,
) {
    let updated_session = with_worker_state(worker_state, |worker| {
        let previous_artwork_url = worker
            .sessions
            .get(&session_id)
            .and_then(|state| state.artwork_url.clone());

        let media_state = merge_media_state(
            SessionSnapshot {
                session_id,
                title: model
                    .media
                    .as_ref()
                    .map_or_else(String::new, |media| media.title.clone()),
                artist: model
                    .media
                    .as_ref()
                    .map_or_else(String::new, |media| media.artist.clone()),
                album: model
                    .media
                    .as_ref()
                    .and_then(|media| media.album.as_ref().map(|album| album.title.clone())),
                source_app: model.source,
                playback_status: model.playback.as_ref().map_or_else(
                    || "Unknown".into(),
                    |playback| format!("{:?}", playback.status),
                ),
                position_ms: model.timeline.as_ref().map_or(0, |timeline| {
                    windows_ticks_to_milliseconds(timeline.position)
                }),
                duration_ms: model.timeline.as_ref().map_or(0, |timeline| {
                    windows_ticks_to_milliseconds(timeline.end.saturating_sub(timeline.start))
                }),
            },
            previous_artwork_url,
            artwork_url,
        );
        worker.timeline_samples.insert(
            session_id,
            TimelineSample {
                position_ms: media_state.position_ms,
                sampled_at: Instant::now(),
            },
        );

        let track_changed = worker
            .sessions
            .get(&session_id)
            .is_none_or(|previous| track_identity_changed(previous, &media_state));
        let is_active = worker.active_session_id == Some(session_id);
        let mut media_state = media_state;
        let lyrics_request = if track_changed {
            worker.lyrics_lines.remove(&session_id);
            let generation = next_lyrics_generation(worker, session_id);
            if is_active && !media_state.title.is_empty() && !media_state.artist.is_empty() {
                media_state.lyrics = LyricsState::loading();
                Some(lyrics_request_from_state(
                    session_id,
                    generation,
                    &media_state,
                ))
            } else if media_state.title.is_empty() || media_state.artist.is_empty() {
                media_state.lyrics = LyricsState::unavailable();
                None
            } else {
                None
            }
        } else {
            if let Some(lines) = worker.lyrics_lines.get(&session_id) {
                if !lines.is_empty() {
                    media_state.lyrics =
                        LyricsState::ready(synchronize(lines, media_state.position_ms));
                } else {
                    media_state.lyrics = worker
                        .sessions
                        .get(&session_id)
                        .map(|previous| previous.lyrics.clone())
                        .filter(|lyrics| lyrics.status != LyricsStatus::Loading)
                        .unwrap_or_else(LyricsState::unavailable);
                }
            } else if let Some(previous) = worker.sessions.get(&session_id) {
                media_state.lyrics = previous.lyrics.clone();
            }
            None
        };

        let should_show = if is_active && !media_state.title.is_empty() {
            worker.no_active_track_since = None;
            if worker.widget_hidden {
                worker.widget_hidden = false;
                true
            } else {
                false
            }
        } else if is_active {
            worker
                .no_active_track_since
                .get_or_insert_with(Instant::now);
            false
        } else {
            false
        };

        worker.sessions.insert(session_id, media_state.clone());
        (is_active, media_state, lyrics_request, should_show)
    });

    match updated_session {
        Ok((true, media_state, lyrics_request, should_show)) => {
            debug!(session_id, title = %media_state.title, "active media state updated");
            if should_show {
                set_widget_visibility(app, true);
            }
            publish_state(app, state, media_state);
            if let Some(request) = lyrics_request {
                spawn_lyrics_fetch(
                    app.clone(),
                    Arc::clone(state),
                    Arc::clone(worker_state),
                    request,
                );
            }
        }
        Ok((false, _, _, _)) => debug!(session_id, "inactive media session updated"),
        Err(()) => publish_state(app, state, MediaState::default()),
    }
}

fn next_lyrics_generation(worker: &mut WorkerState, session_id: usize) -> u64 {
    let generation = worker.lyrics_generations.entry(session_id).or_default();
    *generation = generation.saturating_add(1);
    *generation
}

fn lyrics_request_from_state(
    session_id: usize,
    generation: u64,
    state: &MediaState,
) -> LyricsRequest {
    LyricsRequest {
        session_id,
        generation,
        title: state.title.clone(),
        artist: state.artist.clone(),
        album: state.album.clone(),
        duration_ms: state.duration_ms,
    }
}

fn spawn_lyrics_fetch(
    app: AppHandle,
    state: Arc<Mutex<MediaState>>,
    worker_state: Arc<Mutex<WorkerState>>,
    request: LyricsRequest,
) {
    tauri::async_runtime::spawn(async move {
        let result = fetch_synced_lyrics(
            &request.title,
            &request.artist,
            request.album.as_deref(),
            request.duration_ms,
        )
        .await;

        let updated_state = with_worker_state(&worker_state, |worker| {
            if worker.active_session_id != Some(request.session_id)
                || worker.lyrics_generations.get(&request.session_id) != Some(&request.generation)
            {
                return None;
            }

            let media_state = worker.sessions.get_mut(&request.session_id)?;
            match result {
                Ok(Some(lines)) if !lines.is_empty() => {
                    media_state.lyrics = LyricsState::ready(synchronize(&lines, media_state.position_ms));
                    worker.lyrics_lines.insert(request.session_id, lines);
                }
                Ok(_) => {
                    worker.lyrics_lines.insert(request.session_id, Vec::new());
                    media_state.lyrics = LyricsState::unavailable();
                }
                Err(error) => {
                    warn!(session_id = request.session_id, %error, "lyrics provider request failed");
                    worker.lyrics_lines.remove(&request.session_id);
                    media_state.lyrics = LyricsState::error("providerUnavailable");
                }
            }
            Some(media_state.clone())
        })
        .ok()
        .flatten();

        if let Some(updated_state) = updated_state {
            publish_state(&app, &state, updated_state);
        }
    });
}

fn tick_active_lyrics(
    app: &AppHandle,
    state: &Arc<Mutex<MediaState>>,
    worker_state: &Arc<Mutex<WorkerState>>,
) {
    let updated_state = with_worker_state(worker_state, |worker| {
        let session_id = worker.active_session_id?;
        let sample = *worker.timeline_samples.get(&session_id)?;
        let lines = worker.lyrics_lines.get(&session_id)?;
        if lines.is_empty() {
            return None;
        }

        let media_state = worker.sessions.get_mut(&session_id)?;
        let elapsed_ms = sample
            .sampled_at
            .elapsed()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        let position_ms = projected_position_ms(
            sample.position_ms,
            media_state.duration_ms,
            &media_state.playback_status,
            elapsed_ms,
        );
        let next_lyrics = LyricsState::ready(synchronize(lines, position_ms));
        if media_state.lyrics == next_lyrics {
            return None;
        }

        media_state.lyrics = next_lyrics;
        Some(media_state.clone())
    })
    .ok()
    .flatten();

    if let Some(updated_state) = updated_state {
        publish_state(app, state, updated_state);
    }
}

fn track_identity_changed(previous: &MediaState, next: &MediaState) -> bool {
    previous.title != next.title || previous.artist != next.artist || previous.album != next.album
}

fn publish_state(app: &AppHandle, shared_state: &Arc<Mutex<MediaState>>, next_state: MediaState) {
    let changed = match shared_state.lock() {
        Ok(mut state) => {
            if *state == next_state {
                false
            } else {
                *state = next_state.clone();
                true
            }
        }
        Err(error) => {
            error!(%error, "media state mutex was poisoned");
            false
        }
    };

    if changed {
        info!(
            track_id = ?next_state.track_id,
            playback_status = %next_state.playback_status,
            "emitting media state change"
        );
        if let Err(error) = app.emit("media-state-changed", next_state) {
            warn!(%error, "could not emit media state change");
        }
    }
}

fn with_worker_state<T>(
    worker_state: &Arc<Mutex<WorkerState>>,
    operation: impl FnOnce(&mut WorkerState) -> T,
) -> Result<T, ()> {
    match worker_state.lock() {
        Ok(mut state) => Ok(operation(&mut state)),
        Err(error) => {
            error!(%error, "media worker state mutex was poisoned");
            Err(())
        }
    }
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn non_empty_or(value: String, fallback: &str) -> String {
    optional_text(Some(value)).unwrap_or_else(|| fallback.to_owned())
}

fn milliseconds(value: i64) -> u64 {
    value.max(0) as u64
}

fn windows_ticks_to_milliseconds(value: i64) -> i64 {
    value / 10_000
}

fn projected_position_ms(
    position_ms: u64,
    duration_ms: u64,
    playback_status: &str,
    elapsed_ms: u64,
) -> u64 {
    let projected = if playback_status.eq_ignore_ascii_case("playing") {
        position_ms.saturating_add(elapsed_ms)
    } else {
        position_ms
    };

    if duration_ms == 0 {
        projected
    } else {
        projected.min(duration_ms)
    }
}

fn should_hide_idle_widget(worker: &mut WorkerState, now: Instant) -> bool {
    let has_active_track = worker
        .active_session_id
        .and_then(|session_id| worker.sessions.get(&session_id))
        .is_some_and(|state| !state.title.is_empty());

    if has_active_track {
        worker.no_active_track_since = None;
        return false;
    }

    let idle_since = *worker.no_active_track_since.get_or_insert(now);
    if !worker.widget_hidden && now.saturating_duration_since(idle_since) >= WIDGET_IDLE_TIMEOUT {
        worker.widget_hidden = true;
        true
    } else {
        false
    }
}

fn set_widget_visibility(app: &AppHandle, visible: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let result = if visible {
        window.show()
    } else {
        window.hide()
    };

    if let Err(error) = result {
        warn!(%error, visible, "could not update taskbar widget visibility");
    }
}

fn is_supported_media_source(source: &str) -> bool {
    let source = source.trim().to_lowercase();

    source.contains("spotify")
}

fn merge_media_state(
    snapshot: SessionSnapshot,
    previous_artwork_url: Option<String>,
    artwork_url: Option<Option<String>>,
) -> MediaState {
    let mut state = MediaState::from_session_snapshot(snapshot);
    state.artwork_url = match artwork_url {
        Some(artwork_url) => artwork_url,
        None => previous_artwork_url,
    };
    state
}

fn encode_artwork_url(image: &gsmtc::Image) -> String {
    format!(
        "data:{};base64,{}",
        image.content_type,
        STANDARD.encode(&image.data)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_media_state_has_no_artwork_and_normalizes_metadata() {
        let state = MediaState::default();
        let serialized = serde_json::to_value(&state).expect("default state should serialize");

        assert_eq!(state.track_id, None);
        assert_eq!(state.title, "");
        assert_eq!(state.artist, "");
        assert_eq!(state.album, None);
        assert_eq!(state.source_app, None);
        assert_eq!(state.playback_status, "NoMedia");
        assert_eq!(state.position_ms, 0);
        assert_eq!(state.duration_ms, 0);
        assert_eq!(state.artwork_url, None);
        assert_eq!(state.lyrics, LyricsState::default());
        assert_eq!(state.error, None);
        assert!(serialized.get("error").is_none());

        let normalized = MediaState::from_session_snapshot(SessionSnapshot {
            session_id: 7,
            title: "  Track Title  ".into(),
            artist: "  Artist Name  ".into(),
            album: Some("   ".into()),
            source_app: "  Spotify.exe  ".into(),
            playback_status: " Playing ".into(),
            position_ms: -10,
            duration_ms: 123_456,
        });

        assert_eq!(normalized.track_id.as_deref(), Some("7"));
        assert_eq!(normalized.title, "Track Title");
        assert_eq!(normalized.artist, "Artist Name");
        assert_eq!(normalized.album, None);
        assert_eq!(normalized.source_app.as_deref(), Some("Spotify.exe"));
        assert_eq!(normalized.playback_status, "Playing");
        assert_eq!(normalized.position_ms, 0);
        assert_eq!(normalized.duration_ms, 123_456);
        assert_eq!(normalized.artwork_url, None);
        assert_eq!(normalized.lyrics, LyricsState::default());
        assert_eq!(normalized.error, None);
    }

    #[test]
    fn session_manager_error_states_keep_no_media_defaults_and_serialize_typed_codes() {
        let unavailable = MediaState::session_manager_unavailable();
        let disconnected = MediaState::session_manager_disconnected();

        for (state, expected_code) in [
            (unavailable, "sessionManagerUnavailable"),
            (disconnected, "sessionManagerDisconnected"),
        ] {
            let serialized =
                serde_json::to_value(&state).expect("error states should serialize cleanly");

            assert_eq!(state.track_id, None);
            assert_eq!(state.title, "");
            assert_eq!(state.artist, "");
            assert_eq!(state.album, None);
            assert_eq!(state.source_app, None);
            assert_eq!(state.playback_status, "NoMedia");
            assert_eq!(state.position_ms, 0);
            assert_eq!(state.duration_ms, 0);
            assert_eq!(state.artwork_url, None);
            assert_eq!(
                serialized
                    .pointer("/error/code")
                    .and_then(serde_json::Value::as_str),
                Some(expected_code),
            );
        }
    }

    #[test]
    fn model_only_updates_preserve_artwork_while_media_updates_can_clear_it() {
        let snapshot = SessionSnapshot {
            session_id: 99,
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            source_app: "Spotify.exe".into(),
            playback_status: "Playing".into(),
            position_ms: 1_000,
            duration_ms: 2_000,
        };
        let previous_artwork_url = Some("data:image/png;base64,AAAA".into());

        let preserved = merge_media_state(snapshot, previous_artwork_url.clone(), None);
        assert_eq!(preserved.artwork_url, previous_artwork_url);
        assert_eq!(preserved.error, None);

        let cleared = merge_media_state(
            SessionSnapshot {
                session_id: 99,
                title: "Song".into(),
                artist: "Artist".into(),
                album: None,
                source_app: "Spotify.exe".into(),
                playback_status: "Playing".into(),
                position_ms: 1_000,
                duration_ms: 2_000,
            },
            previous_artwork_url,
            Some(None),
        );
        assert_eq!(cleared.artwork_url, None);
        assert_eq!(cleared.error, None);
    }

    #[test]
    fn image_bytes_are_encoded_as_a_data_url() {
        let image = gsmtc::Image {
            content_type: "image/jpeg".into(),
            data: vec![0x66, 0x6f, 0x6f],
        };

        assert_eq!(encode_artwork_url(&image), "data:image/jpeg;base64,Zm9v");
    }

    #[test]
    fn converts_windows_timeline_ticks_to_milliseconds() {
        assert_eq!(windows_ticks_to_milliseconds(1_880_000_000), 188_000);
        assert_eq!(windows_ticks_to_milliseconds(10_000), 1);
        assert_eq!(windows_ticks_to_milliseconds(-10_000), -1);
    }

    #[test]
    fn projects_playing_position_forward_for_lyric_sync() {
        assert_eq!(
            projected_position_ms(12_000, 180_000, "Playing", 750),
            12_750
        );
    }

    #[test]
    fn does_not_project_paused_position_forward() {
        assert_eq!(
            projected_position_ms(12_000, 180_000, "Paused", 750),
            12_000
        );
    }

    #[test]
    fn clamps_projected_position_to_track_duration() {
        assert_eq!(
            projected_position_ms(179_900, 180_000, "Playing", 750),
            180_000
        );
    }

    #[test]
    fn accepts_supported_media_sources() {
        assert!(is_supported_media_source(
            "SpotifyAB.SpotifyMusic_zpdnekdrzrea0"
        ));
    }

    #[test]
    fn rejects_unrelated_media_sources() {
        assert!(!is_supported_media_source("foobar.video.player"));
        assert!(!is_supported_media_source("Windows Media Player"));
        assert!(!is_supported_media_source("Google Chrome"));
        assert!(!is_supported_media_source("Microsoft Edge"));
        assert!(!is_supported_media_source("YouTube Music"));
        assert!(!is_supported_media_source("YouTube"));
    }

    #[test]
    fn idle_widget_hides_only_after_five_minutes_without_an_active_track() {
        let mut worker = WorkerState::default();
        let start = Instant::now();

        assert!(!should_hide_idle_widget(&mut worker, start));
        assert!(!should_hide_idle_widget(
            &mut worker,
            start + Duration::from_secs(299)
        ));
        assert!(should_hide_idle_widget(
            &mut worker,
            start + Duration::from_secs(300)
        ));
    }
}
