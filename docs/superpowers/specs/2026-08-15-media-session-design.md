# Windows Media Session Design

## Goal

Detect the current Windows Global System Media Transport Controls session, normalize its metadata and timeline into Rust-owned state, and expose that state to the existing React window.

## Scope

This slice includes title, artist, optional album, source app identifier, playback status, current position, duration, no-session handling, and Tauri state events/commands. It excludes playback commands, lyrics, taskbar positioning, and expanded-player windows.

## Approach

Use the Windows GSMTC wrapper `win-gsmtc` in Rust. It provides an asynchronous session manager and session update events while keeping the Windows API boundary inside Rust. A Tauri background task owns the manager receiver, updates a shared `MediaState`, logs transitions, and emits `media-state-changed` to the frontend. The frontend requests an initial snapshot and subscribes to the event.

## State Contract

```rust
struct MediaState {
    track_id: Option<String>,
    title: String,
    artist: String,
    album: Option<String>,
    source_app: Option<String>,
    playback_status: String,
    position_ms: u64,
    duration_ms: u64,
}
```

No active session is represented by `track_id: null` and empty metadata. The frontend hides the future compact player in that state but displays a diagnostic media-session status during this phase.

## Error Handling

Manager creation, event stream termination, and per-session conversion errors are logged and converted to an empty safe state. The app must continue running if no session exists or the provider reports incomplete metadata.

## Verification

Run focused Rust tests for empty/default state and conversion helpers, `cargo check`/`cargo test`, the frontend build, and a Tauri development launch. With Spotify or another media source playing, confirm the native window shows title, artist, playback status, position, and duration.
