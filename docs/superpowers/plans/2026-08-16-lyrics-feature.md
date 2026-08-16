# Lyrics Feature Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Rust-owned LRCLIB lyrics fetching and timestamp synchronization, expose previous/current/next context in media events, and make that context reachable from the compact taskbar widget.

**Architecture:** A focused Rust lyrics module owns LRC parsing, synchronization, LRCLIB requests, and serializable state. The existing media-session worker preserves lyrics across timeline updates, starts a generation-scoped fetch on track changes, and drops stale responses. React keeps the compact strip visually unchanged while toggling a Rust-positioned expanded window that exposes the three-line lyrics context.

**Tech Stack:** Rust, Tauri 2, win-gsmtc, reqwest/rustls, serde, React, TypeScript, CSS, npm.

## Global Constraints

- Rust owns lyric types/state, LRC parsing, synchronization, fetching, and taskbar-window positioning.
- Use LRCLIB public API directly from Rust; do not add Spotify OAuth or Spotify Web API.
- Query LRCLIB with title, artist, optional album, and duration, and send a clear User-Agent.
- Track changes set lyrics Loading, fetch asynchronously, ignore stale results, and timeline updates reselect context.
- Emitted media state includes nested `lyrics` with `status`, `previousLine`, `currentLine`, `nextLine`, and `error`.
- Keep the compact taskbar strip compact; expanded context must remain reachable without opening Spotify.
- Do not use pnpm or commit changes.
- Use focused TDD: tests for parser and synchronization must fail before implementation.

---

### Task 1: Lyrics domain, tests, and provider

**Files:**
- Create: `src-tauri/src/lyrics.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Produces `LyricLine`, `LyricsState`, `LyricsStatus`, `parse_lrc(&str) -> Vec<LyricLine>`, `synchronize(&[LyricLine], u64) -> LyricsContext`, and async LRCLIB fetch helpers for the media worker.
- `LyricsState` serializes with camelCase fields and PascalCase status values.

- [x] **Step 1: Write failing Rust tests** for LRC timestamps, sorted output, empty/invalid lines, and previous/current/next selection before the first line, at exact timestamps, between lines, and after the last line.
- [x] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml`** and confirm the new tests fail because the lyrics implementation is not present.
- [x] **Step 3: Add the minimal lyrics types, parser, synchronizer, LRCLIB response parsing, User-Agent request, and safe provider errors.**
- [x] **Step 4: Run the focused Rust tests and the full Rust suite.**

### Task 2: Media worker integration and expanded-window command

**Files:**
- Modify: `src-tauri/src/media/session.rs`
- Modify: `src-tauri/src/media/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/taskbar.rs`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Existing artwork, metadata, playback, and media-manager error behavior remains intact.
- `MediaState.lyrics` is nested and serializable.
- Add a Tauri command that resizes/repositions the current window between compact and expanded taskbar modes.

- [x] **Step 1: Extend MediaState with nested lyrics defaults and serialization coverage.**
- [x] **Step 2: Integrate generation-scoped async fetches on metadata/track changes and apply synchronized context on timeline updates.**
- [x] **Step 3: Add the expanded taskbar positioning helper and Tauri command.**
- [x] **Step 4: Run cargo fmt and cargo tests.**

### Task 3: TypeScript state and compact/expanded lyrics UI

**Files:**
- Modify: `src/types/media.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.css`

**Interfaces:**
- TypeScript mirrors the nested Rust lyrics object and PascalCase status values.
- Compact mode shows the current lyric or safe loading/unavailable/error copy.
- An accessible button/toggle expands the existing Tauri window to show previous/current/next lines and collapses it back to the compact strip.

- [x] **Step 1: Extend the no-media state and TypeScript types.**
- [x] **Step 2: Add the accessible toggle and lyrics context section without changing unrelated artwork/playback layout.**
- [x] **Step 3: Add only the compact dark-surface styles needed for the expanded context, including overflow and reduced-motion handling.**
- [x] **Step 4: Run npm build and the Impeccable detector once over changed UI files.**

### Task 4: Whole-feature verification

**Files:**
- No additional source files.

- [x] **Step 1: Run cargo fmt check.**
- [x] **Step 2: Run cargo test.**
- [x] **Step 3: Run npm build with the configured fnm Node path.**
- [x] **Step 4: Run a bounded Tauri dev smoke launch if feasible and report the remaining manual Spotify/Windows limitation.**
