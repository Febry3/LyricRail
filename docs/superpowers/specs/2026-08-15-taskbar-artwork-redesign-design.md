# Taskbar Artwork Strip — Design Spec

## Intent

Make the compact taskbar surface feel like a quiet, polished native music control: Mac-like in restraint and finish, but positioned as a Windows taskbar overlay rather than a copied macOS component.

## Visual thesis

The active album sleeve is the only expressive visual anchor. Everything around it is measured utility: dark charcoal surface, compact system typography, one thin progress rail, and no decorative gradients, pills, dashboard cards, or marketing labels.

## Compact composition

- Fixed compact overlay with a 42px square artwork tile on the left.
- Title is the primary line; artist is secondary and quieter.
- A 1px progress rail sits below the metadata and uses one restrained accent color.
- Playback state is conveyed by a small status mark and concise text only when useful.
- The overlay remains legible at the existing compact taskbar dimensions and truncates long metadata without changing its height.
- Artwork uses a subtle corner radius and a thin low-contrast edge; no faux device chrome.

## States

- Active media with artwork: show the thumbnail, title, artist, playback state, and progress.
- Active media without artwork: show a restrained monochrome placeholder, never a broken-image icon.
- No active media: preserve the existing empty state with a quiet placeholder and short instruction.
- Media-session error: keep the surface stable and show a concise inline error.

## Artwork data flow

`win-gsmtc` already exposes thumbnail bytes through `SessionUpdateEvent::Media(_, image)`. Rust will convert the image bytes to a `data:<content-type>;base64,...` URL, include it in the authoritative `MediaState`, and emit it with the existing `media-state-changed` event. The frontend only renders the URL; it does not call Spotify, fetch remote images, or own media state.

Playback/timeline-only events preserve the last artwork for the same session. A new media-properties event replaces the artwork, including clearing it when Windows supplies no thumbnail.

## Interaction and motion

- Artwork swaps crossfade in 180ms using opacity only.
- The progress rail updates without layout animation.
- No hover choreography is needed for the passive compact surface.
- Reduced-motion users receive an immediate artwork swap.

## Implementation boundary

Expected files:

- `src-tauri/Cargo.toml`: add a direct base64 encoding dependency.
- `src-tauri/src/media/session.rs`: carry thumbnail data through normalized media state.
- `src/types/media.ts`: add the artwork URL field.
- `src/App.tsx`: render the album sleeve, fallback, metadata, status, and progress rail.
- `src/App.css`: replace the generic diagnostic card treatment with the compact strip visual system.

No changes to taskbar positioning, Windows Explorer, Spotify authentication, or lyrics behavior are included.

## Verification

- Rust format/check/test commands pass.
- Frontend TypeScript/Vite build passes.
- Manual run with a Windows media app confirms artwork appears, changes with tracks, and falls back cleanly when unavailable.

