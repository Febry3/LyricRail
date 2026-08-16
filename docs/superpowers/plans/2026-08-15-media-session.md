# Windows Media Session Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect the active Windows media session and display normalized media state in the Tauri React window.

**Architecture:** Add a Rust media-session module backed by `win-gsmtc`. A Tauri-managed shared state is updated by an event-driven async worker and emitted to React through `media-state-changed`; React also invokes `get_media_state` on mount.

**Tech Stack:** Rust, Tauri 2, `win-gsmtc`, Tokio through Tauri async runtime, React, TypeScript.

## Global Constraints

- Rust owns Windows APIs, authoritative media state, normalization, logging, and event emission.
- React owns presentation only and must not poll Windows or call native APIs.
- This phase does not add playback controls, lyrics, taskbar geometry, or multiple windows.
- Missing sessions and incomplete metadata are safe runtime states, not fatal errors.
- Keep verification practical; do not add a strict full-suite gate.

---

### Task 1: Add normalized media state and the session worker

**Files:** Create `src-tauri/src/media/mod.rs`, `src-tauri/src/media/session.rs`; modify `src-tauri/Cargo.toml` and `src-tauri/src/lib.rs`.

- [ ] Add the `win-gsmtc` dependency and any required async/serialization dependencies.
- [ ] Define serializable `MediaState` with track ID, title, artist, album, source app, playback status, position, and duration.
- [ ] Define safe empty-state and conversion helpers.
- [ ] Start an event-driven GSMTC worker with Tauri’s async runtime, update `tauri::State<Mutex<MediaState>>`, log changes, and emit `media-state-changed`.
- [ ] Register `get_media_state` and initialize the worker during `run()`.
- [ ] Add focused tests for default state and conversion/normalization helpers.

### Task 2: Display state in React

**Files:** Modify `src/App.tsx` and `src/App.css`; create `src/types/media.ts` if useful.

- [ ] Define the serializable frontend media-state type.
- [ ] Invoke `get_media_state` on mount.
- [ ] Subscribe to `media-state-changed` once and clean up the listener.
- [ ] Display no-session, metadata, playback status, position, duration, and source app states.
- [ ] Keep the UI diagnostic and compact; do not add controls or lyrics.

### Task 3: Verify the slice

- [ ] Run focused Rust tests and `cargo check`.
- [ ] Run the frontend build using the available package manager/runtime.
- [ ] Launch `tauri dev` and confirm the window starts.
- [ ] With a media source active, confirm metadata/state is visible; with no active source, confirm the safe empty state.
