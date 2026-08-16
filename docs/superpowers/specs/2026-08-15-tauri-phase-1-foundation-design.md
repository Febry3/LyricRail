# Taskbar Lyrics Player Phase 1 Foundation Design

## Goal

Create the initial Tauri 2 desktop application foundation for Taskbar Lyrics Player using the official React/TypeScript template. The foundation must launch a native desktop window and prove that React can invoke a Rust command through Tauri.

## Scope

This phase includes Tauri 2 scaffolding, React and TypeScript, a Rust `ping` command, frontend invocation and display of its response, a basic placeholder UI, dependency installation, and build verification.

This phase excludes Windows media sessions, Spotify integration, lyrics, taskbar positioning, playback controls, multiple windows, and production styling.

## Approach

Use `create-tauri-app` with the official React/TypeScript template in the current workspace. This follows Tauri-supported defaults and avoids manually reconstructing Vite, Rust, and Tauri configuration. If npm is unavailable, use the bundled pnpm runtime while retaining the official generator.

## Architecture

The frontend renders a minimal status screen and calls a Tauri command named `ping`. Rust returns a static success message, which React displays. The generated app starts with one native window; taskbar-specific behavior is deferred to a later phase.

## Verification

Verify dependency installation, the frontend build, Rust formatting and tests, Tauri environment information, Tauri development startup, and a visible Rust response in the native window. If a Windows prerequisite is missing, report the exact prerequisite failure.

## Constraints

- Use Tauri 2.x and the current official React/TypeScript template.
- Keep native and OS behavior in Rust.
- Keep presentation and interaction in React/TypeScript.
- Do not add unrequested product features or third-party services.
- Preserve the existing PRD.
