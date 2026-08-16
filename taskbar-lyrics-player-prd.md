# Taskbar Lyrics Player — Product & Program Requirements

## 1. Document Purpose

This document defines the product requirements, technical architecture, implementation boundaries, and acceptance criteria for an MVP Windows desktop application that displays the currently playing media and synchronized lyrics in the unused left-side area of the Windows taskbar.

Intended stack:

- Rust for native Windows integration, media-session access, playback control, window positioning, application state, and lyrics synchronization.
- Tauri 2 as the desktop runtime and Rust-to-frontend bridge.
- TypeScript, React, and CSS for presentation.

## 2. Product Summary

Working name: **Taskbar Lyrics Player**.

The product should detect the active media session, display the track title and artist in a compact taskbar overlay, display synchronized lyrics, open an expanded player on click, and provide basic playback controls. It must visually behave like part of the taskbar without modifying or injecting code into Windows Explorer.

Primary use case: a user listening to Spotify on Windows can see the current track and lyrics without opening Spotify or a separate lyrics application.

## 3. MVP Scope

### Included

- Detect the active Windows media session.
- Read track title, artist, optional album, playback state, current position, and duration.
- Display a compact taskbar overlay with song information.
- Fetch and synchronize lyrics for the active song.
- Open an expanded player.
- Show previous, current, and next lyric lines.
- Support previous track, play, pause, next track, and seek when supported.
- Hide the widget when no media session is active.
- Keep the widget visible while media is paused.
- Gracefully handle missing lyrics.

### Explicitly out of scope

Do not implement without an explicit scope change:

- Spotify OAuth or Spotify Web API dependency.
- Spotify account authentication, users, or cloud backend.
- Playlist or queue management.
- Volume mixer.
- Custom themes, full settings, startup configuration UI, or multi-monitor support.
- Apple Music or YouTube Music-specific integration.
- Taskbar injection, Explorer modification, shell-extension injection, DLL injection, or direct taskbar-internal modification.

## 4. Product Principles

### Native integration lives in Rust

Rust owns Windows media-session APIs, taskbar detection, screen coordinates, playback control, window positioning, media state, lyrics synchronization, Tauri commands, and Tauri events. The frontend must not directly call Windows APIs and must not become authoritative for playback state.

### TypeScript owns presentation

React/TypeScript owns rendering, state received from Rust, user interaction, commands sent to Rust, animations, visual transitions, and progress-bar interaction. Rust remains the authoritative source of playback state.

### No Windows Explorer modification

The app must use borderless transparent windows positioned over the unused taskbar area. It must not become a native child of the taskbar or inject into Explorer.

## 5. User Experience

### Compact mode

The default state is a small widget positioned in the unused area to the left of centered Windows 11 taskbar icons. It displays title and artist, preferably as `♪ Title — Artist`, falling back to a two-line layout when space is limited.

The widget has no normal border or title bar, is not resizable, does not appear as another taskbar application icon, remains lightweight, and aligns with taskbar height.

### Compact interaction

Clicking the compact widget opens the expanded player directly above it.

### Expanded player

The expanded player is a borderless, non-resizable panel above the taskbar containing title, artist, current time, total duration, progress bar, previous, play/pause, next, and synchronized lyrics.

### Lyrics display

Show at most three primary lines: previous, current, and next. The current lyric is visually dominant; adjacent lines have lower emphasis. Transitions should be smooth.

## 6. Application States

- **No active media:** hide compact and expanded players; process may remain running.
- **Playing:** show widget and update metadata, position, lyric, and state.
- **Paused:** keep widget visible and indicate paused state.
- **Lyrics loading:** show `Fetching lyrics…`; controls remain usable.
- **Lyrics unavailable:** show `No synced lyrics available`; controls remain usable.
- **Lyrics provider error:** show a non-blocking `Lyrics unavailable` fallback; do not crash.
- **Media session lost:** clear state and hide both windows.

## 7. Technical Architecture

```text
Windows Media Session
        │
        ▼
┌──────────────────────┐
│ Rust Core             │
│ MediaSessionService   │
│ PlaybackController    │
│ LyricsService         │
│ LyricsSynchronizer    │
│ TaskbarLocator        │
│ WindowManager         │
│ AppState              │
└──────────┬───────────┘
           │ Tauri commands/events
           ▼
┌──────────────────────┐
│ TypeScript + React    │
│ CompactPlayer         │
│ ExpandedPlayer        │
│ LyricsView            │
│ PlaybackControls      │
│ ProgressBar           │
└──────────────────────┘
```

## 8. Rust Components

### MediaSessionService

Own Global System Media Transport Controls access. Provide the current active session, metadata, playback state, timeline properties, media-change notifications, and timeline-change notifications. Normalize the result into internal media state.

### PlaybackController

Provide `play()`, `pause()`, `toggle_play_pause()`, `next()`, `previous()`, and `seek(position_ms)`. Return meaningful errors when the active session does not support an operation.

### LyricsService

Resolve synchronized lyrics using title and artist, with optional album and duration matching. Normalize provider data to:

```rust
struct LyricLine {
    timestamp_ms: u64,
    text: String,
}
```

Sort lines by timestamp and hide provider-specific JSON behind a replaceable `LyricsProvider` interface.

### LyricsSynchronizer

Given playback position and sorted timestamps, return previous, current, next, and current index. Use binary search or a cached index with forward/backward correction; do not scan from the beginning on every update.

### TaskbarLocator

Determine actual taskbar bounds and overlay coordinates instead of hardcoding resolution or taskbar dimensions. MVP assumptions: Windows 11, primary monitor, normal bottom taskbar, centered icons.

### WindowManager

Manage `compact-player` and `expanded-player` windows. Compact behavior targets decorations false, transparent true, resizable false, skip taskbar true, and always-on-top true. The expanded window is borderless, above the compact widget, hidden with compact mode, non-resizable for MVP, and closes/hides on outside click where reasonably possible.

### AppState

Rust owns the authoritative serializable player state:

```rust
struct PlayerState {
    track_id: Option<String>,
    title: String,
    artist: String,
    album: Option<String>,
    is_playing: bool,
    position_ms: u64,
    duration_ms: u64,
    lyrics_status: LyricsStatus,
    current_lyric_index: Option<usize>,
}
```

Lyrics status values: `Idle`, `Loading`, `Ready`, `Unavailable`, and `Error`.

## 9. Frontend Architecture

Recommended structure:

```text
src/
├── components/
│   ├── CompactPlayer.tsx
│   ├── ExpandedPlayer.tsx
│   ├── LyricsView.tsx
│   ├── PlaybackControls.tsx
│   └── ProgressBar.tsx
├── hooks/usePlayerState.ts
├── lib/tauri.ts
├── lib/formatTime.ts
├── types/player.ts
└── styles/
    ├── tokens.css
    ├── compact-player.css
    └── expanded-player.css
```

Equivalent clean organization is acceptable if component boundaries remain clear.

Frontend state shape:

```ts
export type PlayerState = {
  trackId: string | null;
  title: string;
  artist: string;
  album: string | null;
  playback: {
    isPlaying: boolean;
    positionMs: number;
    durationMs: number;
  };
  lyrics: {
    status: "idle" | "loading" | "ready" | "unavailable" | "error";
    currentLineIndex: number | null;
    previousLine: string | null;
    currentLine: string | null;
    nextLine: string | null;
  };
};
```

The frontend may hold only temporary UI state such as expanded/open, hover, dragging, and animation state.

## 10. Rust ↔ TypeScript Communication

Commands may include `get_player_state`, `toggle_play_pause`, `next_track`, `previous_track`, `seek`, `open_expanded_player`, and `close_expanded_player`.

Rust should emit a `player-state-changed` event with the serialized `PlayerState`. Subscribe once in React and update render state from the event. Avoid expensive high-frequency polling; a small timing update is acceptable for smooth progress.

## 11. Media, Playback, and Seek Flows

Track changes must update song metadata immediately, set lyrics to loading, load lyrics independently, synchronize the result, and emit state updates throughout. Playback commands flow React → Tauri command → PlaybackController → Windows session → Windows-reported state → AppState → event → React. Do not treat frontend clicks as authoritative.

For seek, show temporary frontend position during drag, commit `seek(position_ms)` on release, then use the resulting timeline update to resynchronize lyrics. Do not send dozens of native seek calls per pixel for MVP.

## 12. Lyrics Matching and Caching

Normalize title and artist for matching by trimming whitespace, normalizing case, and removing only safe metadata noise such as `(Remastered)`, `- Live`, and `(feat. Artist)`. Preserve original metadata for UI and avoid aggressive transformations. Use duration when available to improve confidence.

A lightweight in-memory or small persistent cache may be added if complexity remains reasonable. Suggested key: normalized artist + normalized title + optional duration. The provider abstraction must make later caching easy even if cache persistence is deferred.

## 13. Styling and Performance

Use rounded corners, subtle transparency, restrained animation, compact spacing, readable typography, and feasible dark/light compatibility. Avoid oversized controls, web-dashboard styling, large shadows, unnecessary gradients, and excessive animation.

Keep long-running idle CPU use very low: prefer OS events, avoid busy loops and unnecessary rerenders, do not continuously refetch lyrics, cache active-track lyrics in memory, and hide/show windows rather than repeatedly creating/destroying them.

## 14. Reliability and Logging

The app must not crash on missing metadata, unavailable duration, lyrics failures, missing media sessions, unsupported playback actions, Spotify closure, or rapid track changes. Convert errors into safe UI states and log them.

Use structured Rust logging with `ERROR`, `WARN`, `INFO`, and `DEBUG`. Examples include media session detected, track changed, lyrics loaded, lyrics unavailable, playback action unsupported, and lyrics provider request failed. Never log sensitive provider tokens.

## 15. Testing Strategy

### Rust unit tests

Test lyrics synchronization before the first line, at exact timestamps, between lines, after the last line, after forward/backward seek, and with empty lyrics. Test metadata normalization with normal titles, whitespace, featured artists, and remaster notation. Test state transitions for no media, playing, paused, track changes, and lyrics loading/ready/unavailable.

### Frontend tests

Test compact rendering, paused state, lyric states, progress calculation, unavailable lyrics, and playback callback behavior.

### Manual integration checklist

1. Start with Spotify closed.
2. Open Spotify and play a song.
3. Verify the compact widget appears.
4. Pause and verify it remains visible.
5. Resume and skip tracks.
6. Verify metadata and lyrics change.
7. Open expanded player.
8. Test play/pause, previous, next, forward seek, and backward seek.
9. Verify lyric resynchronization.
10. Close Spotify and verify the widget disappears.

## 16. Acceptance Criteria

- **AC-01 Media detection:** Spotify playback exposes title and artist.
- **AC-02 Compact widget:** active media produces a compact borderless widget in the left taskbar area.
- **AC-03 No injection:** appearance is achieved without Explorer modification or taskbar-process injection.
- **AC-04 Hidden without media:** no active session hides the widget.
- **AC-05 Paused state:** paused playback keeps the widget visible and indicates pause.
- **AC-06 Expanded player:** clicking compact mode opens the player above the taskbar.
- **AC-07 Play/pause:** expanded mode can play and pause.
- **AC-08 Previous/next:** expanded mode can request previous and next when supported.
- **AC-09 Progress:** position and duration are displayed.
- **AC-10 Seeking:** supported sessions can seek using the progress bar.
- **AC-11 Lyrics:** synchronized lyrics follow playback position.
- **AC-12 Context:** previous, current, and next lines are shown.
- **AC-13 Seek resynchronization:** forward and backward seeks resolve the correct lyric.
- **AC-14 Missing lyrics:** unavailable lyrics do not break controls.
- **AC-15 Track changes:** metadata updates immediately and lyrics load for the new track.
- **AC-16 Media closure:** closing Spotify with no remaining media session hides the widget.

## 17. Implementation Order

1. Bootstrap Tauri 2, TypeScript, React, and a Rust command smoke test.
2. Validate Windows media-session discovery and metadata.
3. Add playback controls.
4. Add centralized Rust-owned `PlayerState` and Tauri events.
5. Add taskbar detection and compact overlay positioning.
6. Add expanded player and seek UI.
7. Add lyrics provider abstraction and loading states.
8. Add timestamp synchronization and seek resynchronization.
9. Polish visuals, transitions, loading, errors, and compact overflow.

Each phase must be verified before the next begins. Validate Windows media APIs and taskbar positioning early. Do not generate large numbers of placeholder modules before the core integration works.

## 18. Definition of Done

The MVP is complete when the project builds and launches on Windows; Spotify playback is detected; metadata, taskbar overlay, paused state, expanded player, controls, progress, seeking, synchronized lyrics, track changes, media closure, and lyrics failures work; no injection is used; Rust and frontend tests pass; and the manual integration checklist passes.

## 19. Future Enhancements

After MVP: startup with Windows, lyrics cache, auto-hide support, configurable compact width, keyboard shortcuts, multi-monitor support, taskbar placement detection, theme customization, album artwork, richer lyric animations, multiple media-source preferences, Spotify-specific enhancements, queue display, configurable providers, and a provider/plugin architecture.

## 20. Final Architecture Rule

> **Rust owns the operating system, media session, playback state, lyrics synchronization, and window behavior. TypeScript/React owns presentation and user interaction. Tauri is the typed boundary between them.**

The application should behave like a lightweight native Windows utility, not like a website placed inside a desktop wrapper.
