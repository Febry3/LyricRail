# Taskbar Overlay Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Keep the lyrics overlay visibly above the Windows taskbar after taskbar clicks and reshape the compact UI to match the supplied wide album-art/current-lyric sketch.

**Architecture:** Rust will own the native window's topmost/visible reassertion and taskbar-relative positioning. React will keep the existing media and lyrics state, but render one calm horizontal strip: artwork at the left, track metadata as supporting text, and the synchronized current lyric as the visual focus. The existing expanded three-line lyrics context remains available without changing lyrics fetching.

**Tech Stack:** Tauri 2, Rust, `windows-sys`, React, TypeScript, CSS, existing `npm` scripts.

## Global Constraints

- Keep the overlay always visible and topmost when the taskbar receives focus.
- Do not hide or destroy the window on blur or taskbar interaction.
- Preserve Rust-owned media and lyrics state, including previous/current/next lines.
- Use the attached screenshots as visual references, not as literal copy or pixel assets.
- Do not use pnpm; verification uses `npm run build`, `cargo test`, and `npm run tauri dev`.

### Task 1: Native window persistence and taskbar positioning

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/taskbar.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Test: `src-tauri/src/taskbar.rs` (pure geometry/constants where practical)

**Interfaces:**
- Preserve `position_compact_window`, `position_expanded_window`, and `set_compact_expanded` command names.
- Add a small Rust helper that reapplies `HWND_TOPMOST` and `SWP_NOACTIVATE | SWP_SHOWWINDOW` to the main window without stealing focus.
- Start a low-frequency native-window keep-alive from Tauri setup; it must stop when the app runtime ends.

- [ ] Add the minimal Windows API feature flags needed for `SetWindowPos`, `ShowWindow`, and the Tauri window handle.
- [ ] Write/extend focused tests for compact/expanded dimensions and taskbar-edge placement calculations where they do not require a live HWND.
- [ ] Apply topmost/show/no-activate ordering after initial placement and from the keep-alive path.
- [ ] Keep taskbar-edge positioning above the taskbar for bottom/top/left/right taskbars, using the current inset and dimensions.
- [ ] Keep the window visible when it loses focus and ensure the expanded context is positioned fully on-screen above the taskbar.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo test --manifest-path src-tauri/Cargo.toml`.

### Task 2: Sketch-aligned compact lyric strip

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Modify: `src/types/media.ts` only if the existing state needs a compatibility-safe adjustment.

**Interfaces:**
- Consume the existing `MediaState.lyrics` fields without changing their Rust serialization names.
- Preserve keyboard activation, artwork fallback, loading/unavailable/error lyric copy, and the expanded previous/current/next context.

- [ ] Make the compact strip a single wide rounded surface with a square artwork tile on the left.
- [ ] Make the current lyric the strongest text, centered in the available strip space; keep title and artist as smaller supporting metadata.
- [ ] Remove competing compact chrome that is not present in the sketch while retaining a restrained playback/status cue.
- [ ] Keep the current lyric and safe fallback states readable with ellipsis and no layout overflow.
- [ ] Keep click and keyboard activation predictable; the expanded context must not close the app or place content behind the taskbar.
- [ ] Support reduced motion and visible keyboard focus.
- [ ] Run `npm run build`.

### Task 3: Integrated verification

**Files:**
- Verify: all files from Tasks 1–2.

- [ ] Inspect the final diff and confirm no unrelated media/provider behavior changed.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml` and confirm zero failures.
- [ ] Run `npm run build` with the active fnm Node path.
- [ ] Launch `npm run tauri dev`, confirm Vite, Rust, and the app process start, then stop cleanly.
- [ ] Manually verify with the user's desktop: overlay remains above the taskbar after clicking the taskbar, album art is visible, current lyrics are centered, and click-to-expand remains visible.
