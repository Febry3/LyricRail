# LyricRail

<p align="center">
  <img src="src-tauri/icons/lyricrail-logo-generated.png" alt="LyricRail logo" width="128" />
</p>

<p align="center"><strong>Your music, one line at a time.</strong></p>

LyricRail is a lightweight Windows desktop utility that keeps the currently playing song and synchronized lyrics visible in a compact overlay attached to the Windows taskbar.

It is designed for listening without repeatedly switching back to Spotify or another media player. The overlay shows album artwork, title, artist, lyric status, and playback controls, while an expanded view provides previous, current, and next lyric lines.

## Features

- Detects the active Windows media session, including Spotify.
- Displays title, artist, playback state, and album artwork.
- Positions a compact, always-on-top overlay inside the taskbar band.
- Expands into a lyric context view when clicked.
- Shows previous, current, and next synchronized lyric lines.
- Provides previous track, play/pause, and next track controls.
- Keeps the overlay visible while playback is paused.
- Handles loading, unavailable, and provider-error lyric states gracefully.
- Uses Rust for Windows integration and React/TypeScript for presentation.
- Does not inject into or modify Windows Explorer.

## How it works

```text
Windows Media Session
        │
        ▼
Rust + Tauri 2
  media state · controls · taskbar placement · lyric sync
        │
        ▼
React + TypeScript
  taskbar strip · artwork · controls · lyric context
```

LyricRail requests synchronized lyrics from [LRCLIB](https://lrclib.net/). Some songs do not have synchronized lyrics available; in that case the app displays a non-blocking fallback message and keeps the media controls usable.

## Requirements

- Windows 10 or Windows 11
- Node.js and npm
- Rust stable toolchain
- Microsoft Edge WebView2 Runtime

The project uses npm. pnpm is not required.

## Development

Install the JavaScript dependencies:

```powershell
npm install
```

Start the Tauri development app:

```powershell
npm run tauri dev
```

Create a production frontend build:

```powershell
npm run build
```

Run the Rust test suite:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

## Project structure

```text
src/
├── App.tsx             # React UI and Tauri event bindings
├── App.css             # Compact and expanded overlay styling
└── types/              # Frontend media-state types

src-tauri/
├── src/media/          # Windows media session and playback controls
├── src/lyrics.rs       # LRCLIB provider and lyric synchronization
├── src/taskbar.rs      # Taskbar detection and native window positioning
├── src/lib.rs          # Tauri commands and application setup
└── icons/              # Application branding and bundled icon assets
```

## Product boundaries

LyricRail uses a borderless, transparent, always-on-top Tauri window positioned over the taskbar area. It intentionally does not become a child window of Explorer and does not use taskbar injection, DLL injection, or shell modification.

Spotify authentication, playlists, queue management, volume mixing, custom themes, and multi-monitor configuration are outside the current MVP scope.

## Branding

The current product name is **LyricRail**. The logo is a charcoal rounded square with three lyric lines and a muted mint highlight on the current line.

Logo asset: [`src-tauri/icons/lyricrail-logo-generated.png`](src-tauri/icons/lyricrail-logo-generated.png)

## License

This project is currently an unlicensed private project.
