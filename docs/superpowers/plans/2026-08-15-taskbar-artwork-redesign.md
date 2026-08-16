# Taskbar Artwork Strip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Windows media-session album artwork to the authoritative media state and replace the compact diagnostic card with a restrained Mac-like taskbar artwork strip.

**Architecture:** Rust consumes `win-gsmtc` media thumbnail bytes, converts them to a local data URL, and carries that URL in the existing `MediaState` event stream. React remains a presentation layer: it renders the album sleeve, metadata, status, and progress rail with a stable fallback when artwork is unavailable. No external API or filesystem cache is introduced.

**Tech Stack:** Rust, Tauri 2, `win-gsmtc`, `base64`, React, TypeScript, CSS, Vite.

## Global Constraints

- The active album sleeve is the only expressive visual anchor.
- No decorative gradients, pills, dashboard cards, or marketing labels.
- The overlay remains legible at the existing compact taskbar dimensions and truncates long metadata without changing its height.
- Rust remains authoritative for media state; the frontend does not call Spotify or fetch remote images.
- Playback/timeline-only events preserve the last artwork for the same session.
- A new media-properties event replaces the artwork, including clearing it when Windows supplies no thumbnail.
- Artwork swaps crossfade in 180ms using opacity only, with an immediate swap for reduced motion.

---

### Task 1: Carry Windows media thumbnails through Rust state

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/media/session.rs`

**Interfaces:**
- Produce `MediaState.artwork_url: Option<String>` serialized as `artworkUrl`.
- Preserve artwork on `SessionUpdateEvent::Model` and replace/clear it on `SessionUpdateEvent::Media`.

- [ ] Add `base64 = "0.22"` as a direct Rust dependency.
- [ ] Extend `MediaState` and its default state with `artwork_url: Option<String>`.
- [ ] Change the event match so `SessionUpdateEvent::Media(model, image)` retains the optional `win_gsmtc::Image` while model-only events have no artwork update.
- [ ] Encode image bytes as `data:<content_type>;base64,<bytes>` using `base64::engine::general_purpose::STANDARD`.
- [ ] Update the worker state merge so model-only updates reuse the prior artwork for that session, while media updates use `Some(encoded_url)` or `Some(None)` to explicitly clear it.
- [ ] Add a unit assertion that the default state has no artwork and that metadata normalization still passes.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo test --manifest-path src-tauri/Cargo.toml`; expect all tests to pass.

### Task 2: Render the compact artwork strip

**Files:**
- Modify: `src/types/media.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.css`

**Interfaces:**
- Consume `MediaState.artworkUrl` from the existing `media-state-changed` event and `get_media_state` command.
- Keep the existing media-state listener, error handling, and no-media behavior.

- [ ] Add `artworkUrl: string | null` to the TypeScript media type and no-media constant.
- [ ] Replace the diagnostic heading/eyebrow treatment with a compact strip: artwork tile, title, artist, playback mark, and progress rail.
- [ ] Render a monochrome CSS placeholder when `artworkUrl` is null; do not show a broken image icon.
- [ ] Add an image `onError` fallback that swaps to the placeholder without destabilizing the media state.
- [ ] Use `object-fit: cover`, truncation with `min-width: 0`, and an accessible alt label based on title/artist.
- [ ] Add a 180ms opacity crossfade for artwork changes and disable the transition under `prefers-reduced-motion: reduce`.
- [ ] Use restrained charcoal/neutral tokens, one accent color, no gradients, no pills, no decorative labels, and no generic card mosaic.
- [ ] Keep the compact 420×52 taskbar layout intact and prevent horizontal overflow.
- [ ] Run `npm run build`; expect TypeScript and Vite to complete successfully.

### Task 3: Review the integrated surface

**Files:**
- Inspect: `src/App.tsx`, `src/App.css`, `src/types/media.ts`, `src-tauri/src/media/session.rs`
- Inspect: `docs/superpowers/specs/2026-08-15-taskbar-artwork-redesign-design.md`

- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `npm run build`.
- [ ] Run the Impeccable detector once over the changed UI files: `node C:\Users\Admin\.agents\skills\impeccable\scripts\detect.mjs --json src/App.tsx src/App.css`.
- [ ] Launch `npm run tauri dev` with a Windows media app playing and verify artwork, track changes, missing-art fallback, and no-media state manually.

