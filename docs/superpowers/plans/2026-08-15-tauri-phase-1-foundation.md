# Taskbar Lyrics Player Phase 1 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold a Tauri 2 desktop application with a React/TypeScript frontend and verify a React-to-Rust `ping` command.

**Architecture:** Use the official `create-tauri-app` React/TypeScript template. Keep the generated single-window Tauri shell, replace the template greeting command with `ping`, and invoke it from React through `@tauri-apps/api/core`.

**Tech Stack:** Tauri 2.x, Rust, Cargo, React, TypeScript, Vite, npm or pnpm, Windows WebView2.

## Global Constraints

- Use Tauri 2.x and the current official React/TypeScript template.
- Keep native and OS behavior in Rust.
- Keep presentation and interaction in React/TypeScript.
- Do not add Spotify integration, media sessions, lyrics, taskbar positioning, playback controls, or multiple windows in this phase.
- Preserve `taskbar-lyrics-player-prd.md`.
- Do not initialize Git or create commits unless explicitly requested.

---

### Task 1: Scaffold the official Tauri React/TypeScript application

**Files:** Create the official generator output: `package.json`, lockfile, `index.html`, `src/`, `vite.config.ts`, TypeScript config, and `src-tauri/`. Preserve the PRD.

**Steps:**

- [ ] Check the workspace contents and record the PRD hash.
- [ ] Run `npm create tauri-app@latest .`, or use the bundled pnpm executable if npm is unavailable: `pnpm create tauri-app@latest .`.
- [ ] Choose project name `taskbar-lyrics-player`, identifier `com.taskbarlyricsplayer.app`, TypeScript/JavaScript, the selected package manager, React, and TypeScript.
- [ ] Install dependencies with `npm install` or `pnpm install`.
- [ ] Confirm `package.json` exposes the `tauri` script, `@tauri-apps/api` is 2.x, and the Tauri CLI reports 2.x.
- [ ] Do not initialize Git or commit.

### Task 2: Replace the template greeting with the Rust-to-React smoke test

**Files:** Modify `src-tauri/src/lib.rs`, `src/App.tsx`, and the generated stylesheet.

**Interfaces:** Produce `ping() -> &'static str`, register it with `tauri::generate_handler![ping]`, and display the response or a visible error in React.

**Steps:**

- [ ] Define `#[tauri::command] fn ping() -> &'static str { "pong from Rust" }`.
- [ ] Add a focused Rust test asserting that exact return value.
- [ ] Register `ping` in the generated Tauri builder.
- [ ] Invoke `invoke<string>("ping")` from React on mount and display `Connecting to Rust…`, the returned value, or an error.
- [ ] Keep the UI minimal and remove unused template imports/assets.
- [ ] Run the focused Rust test.

### Task 3: Verify frontend, Rust, and Tauri builds

**Steps:**

- [ ] Run the frontend production build.
- [ ] Run Rust formatting and tests.
- [ ] Run the Tauri environment information command.
- [ ] Run the Tauri development app and confirm the native window displays `pong from Rust`.
- [ ] Run a release compilation check; if installer bundling is blocked, report the exact Windows prerequisite.
- [ ] Confirm the PRD hash is unchanged and list the resulting project files.
