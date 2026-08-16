import { useEffect, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import "./App.css";
import type { LyricsState, MediaErrorCode, MediaState } from "./types/media";

const ARTWORK_FADE_MS = 180;
const DISPLAY_PREFERENCES_KEY = "taskbar-lyrics-player:display-preferences";
const MIN_WIDGET_WIDTH = 360;
const MAX_WIDGET_WIDTH = 720;
const MIN_LYRICS_FONT_SIZE = 10;
const MAX_LYRICS_FONT_SIZE = 22;
type AppErrorCode = "bridgeUnavailable";

type DisplayPreferences = {
  albumArt: boolean;
  title: boolean;
  artist: boolean;
  lyrics: boolean;
  previous: boolean;
  playPause: boolean;
  next: boolean;
  progress: boolean;
  transparentBackground: boolean;
  width: number;
  lyricsFontSize: number;
};

const DEFAULT_DISPLAY_PREFERENCES: DisplayPreferences = {
  albumArt: true,
  title: true,
  artist: true,
  lyrics: true,
  previous: true,
  playPause: true,
  next: true,
  progress: true,
  transparentBackground: false,
  width: 560,
  lyricsFontSize: 14,
};

function loadDisplayPreferences(): DisplayPreferences {
  try {
    const stored = JSON.parse(localStorage.getItem(DISPLAY_PREFERENCES_KEY) ?? "null");
    if (!stored || typeof stored !== "object") {
      return DEFAULT_DISPLAY_PREFERENCES;
    }

    return Object.fromEntries(
      Object.keys(DEFAULT_DISPLAY_PREFERENCES).map((key) => {
        const preferenceKey = key as keyof DisplayPreferences;
        const defaultValue = DEFAULT_DISPLAY_PREFERENCES[preferenceKey];
        const storedValue = stored[preferenceKey];
        const value =
          typeof defaultValue === "boolean"
            ? typeof storedValue === "boolean"
              ? storedValue
              : defaultValue
            : typeof storedValue === "number" && Number.isFinite(storedValue)
              ? preferenceKey === "width"
                ? Math.min(Math.max(Math.round(storedValue), MIN_WIDGET_WIDTH), MAX_WIDGET_WIDTH)
                : Math.min(
                    Math.max(Math.round(storedValue), MIN_LYRICS_FONT_SIZE),
                    MAX_LYRICS_FONT_SIZE,
                  )
              : defaultValue;
        return [key, value];
      }),
    ) as DisplayPreferences;
  } catch {
    return DEFAULT_DISPLAY_PREFERENCES;
  }
}

const NO_MEDIA_STATE: MediaState = {
  trackId: null,
  title: "",
  artist: "",
  album: null,
  artworkUrl: null,
  sourceApp: null,
  playbackStatus: "NoMedia",
  positionMs: 0,
  durationMs: 0,
  error: null,
  lyrics: {
    status: "Idle",
    previousLine: null,
    currentLine: null,
    nextLine: null,
    currentLineIndex: null,
    error: null,
  },
};

function formatDuration(milliseconds: number) {
  const seconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

function getArtworkAltText(title: string, artist: string) {
  const safeTitle = title.trim() || "Untitled media";
  const safeArtist = artist.trim();

  return safeArtist
    ? `Album artwork for ${safeTitle} by ${safeArtist}`
    : `Album artwork for ${safeTitle}`;
}

function getProgressRatio(positionMs: number, durationMs: number) {
  if (durationMs <= 0) {
    return 0;
  }

  return Math.min(Math.max(positionMs / durationMs, 0), 1);
}

function getLyricsTeaser(lyrics: LyricsState) {
  switch (lyrics.status) {
    case "Loading":
      return "Fetching lyrics…";
    case "Unavailable":
      return "No synced lyrics available";
    case "Error":
      return "Lyrics unavailable";
    case "Ready":
      return lyrics.currentLine || lyrics.nextLine || "Lyrics ready — open context";
    default:
      return "";
  }
}

function getLyricsLine(line: string | null, fallback: string) {
  return line?.trim() || fallback;
}

function getErrorCopy(errorCode: MediaErrorCode | AppErrorCode) {
  switch (errorCode) {
    case "sessionManagerUnavailable":
      return {
        title: "Media connection unavailable",
        detail: "Restart a media app to retry.",
      };
    case "sessionManagerDisconnected":
      return {
        title: "Media connection lost",
        detail: "Reopen a media app to reconnect.",
      };
    case "bridgeUnavailable":
      return {
        title: "Media status unavailable",
        detail: "Reopen the widget to retry.",
      };
  }
}

function MediaIcon({ type }: { type: "previous" | "play" | "pause" | "next" }) {
  if (type === "play") {
    return (
      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
        <path d="m7 4.5 7 5.5-7 5.5z" />
      </svg>
    );
  }

  if (type === "pause") {
    return (
      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
        <path d="M6.5 4.5v11M13.5 4.5v11" />
      </svg>
    );
  }

  if (type === "previous") {
    return (
      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
        <path d="m13.5 4.5-6 5.5 6 5.5M5.5 4.5v11" />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
      <path d="m6.5 4.5 6 5.5-6 5.5M14.5 4.5v11" />
    </svg>
  );
}

function SettingsView() {
  const [preferences, setPreferences] = useState<DisplayPreferences>(loadDisplayPreferences);
  const [launchAtStartup, setLaunchAtStartup] = useState(false);
  const [autostartError, setAutostartError] = useState(false);
  const displayOptions: Array<{
    key: Exclude<keyof DisplayPreferences, "width" | "lyricsFontSize">;
    label: string;
    detail: string;
  }> = [
    { key: "albumArt", label: "Album artwork", detail: "Show the current track image." },
    { key: "title", label: "Track title", detail: "Show the song name." },
    { key: "artist", label: "Artist", detail: "Show the artist name." },
    { key: "lyrics", label: "Lyrics", detail: "Show the current lyric line." },
    { key: "previous", label: "Previous button", detail: "Show the previous-track control." },
    { key: "playPause", label: "Play / pause button", detail: "Show the main playback control." },
    { key: "next", label: "Next button", detail: "Show the next-track control." },
    { key: "progress", label: "Progress bar", detail: "Show playback progress below the content." },
    {
      key: "transparentBackground",
      label: "Transparent background",
      detail: "Remove the widget's outer panel background.",
    },
  ];

  const updatePreference = (key: keyof DisplayPreferences, value: boolean | number) => {
    const nextPreferences = { ...preferences, [key]: value };
    setPreferences(nextPreferences);
    localStorage.setItem(DISPLAY_PREFERENCES_KEY, JSON.stringify(nextPreferences));
    void emit("display-preferences-changed", nextPreferences);
  };

  useEffect(() => {
    let disposed = false;

    void isEnabled()
      .then((enabled) => {
        if (!disposed) {
          setLaunchAtStartup(enabled);
        }
      })
      .catch(() => {
        if (!disposed) {
          setAutostartError(true);
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  const updateAutostart = (enabled: boolean) => {
    const operation = enabled ? enable() : disable();

    setAutostartError(false);
    void operation.catch(() => {
      setLaunchAtStartup(!enabled);
      setAutostartError(true);
    });

    setLaunchAtStartup(enabled);
  };

  return (
    <main className="settings-page">
      <header className="settings-page__header">
        <div>
          <p className="settings-page__eyebrow">Taskbar Lyrics Player</p>
          <h1>Display settings</h1>
          <p>Choose what stays visible in the compact widget.</p>
        </div>
        <button
          className="settings-page__close"
          type="button"
          aria-label="Close settings"
          onClick={() => void getCurrentWindow().hide()}
        >
          ×
        </button>
      </header>

      <section className="settings-width" aria-label="Widget width">
        <div className="settings-width__header">
          <span>
            <span className="settings-option__label">Widget width</span>
            <span className="settings-option__detail">Compact overlay width</span>
          </span>
          <output>{preferences.width}px</output>
        </div>
        <input
          className="settings-width__slider"
          type="range"
          min={MIN_WIDGET_WIDTH}
          max={MAX_WIDGET_WIDTH}
          step={10}
          value={preferences.width}
          onChange={(event) => updatePreference("width", Number(event.target.value))}
        />
        <div className="settings-width__scale" aria-hidden="true">
          <span>{MIN_WIDGET_WIDTH}px</span>
          <span>{MAX_WIDGET_WIDTH}px</span>
        </div>
      </section>

      <section className="settings-width" aria-label="Lyrics font size">
        <div className="settings-width__header">
          <span>
            <span className="settings-option__label">Lyrics font size</span>
            <span className="settings-option__detail">Applies to compact and expanded lyrics.</span>
          </span>
          <output>{preferences.lyricsFontSize}px</output>
        </div>
        <input
          className="settings-width__slider"
          type="range"
          min={MIN_LYRICS_FONT_SIZE}
          max={MAX_LYRICS_FONT_SIZE}
          step={1}
          value={preferences.lyricsFontSize}
          onChange={(event) => updatePreference("lyricsFontSize", Number(event.target.value))}
        />
        <div className="settings-width__scale" aria-hidden="true">
          <span>{MIN_LYRICS_FONT_SIZE}px</span>
          <span>{MAX_LYRICS_FONT_SIZE}px</span>
        </div>
      </section>

      <section className="settings-list" aria-label="Widget display options">
        {displayOptions.map(({ key, label, detail }) => (
          <label className="settings-option" key={key}>
            <span>
              <span className="settings-option__label">{label}</span>
              <span className="settings-option__detail">{detail}</span>
            </span>
            <input
              type="checkbox"
              checked={preferences[key]}
              onChange={(event) => updatePreference(key, event.target.checked)}
            />
          </label>
        ))}
        <label className="settings-option">
          <span>
            <span className="settings-option__label">Launch at startup</span>
            <span className="settings-option__detail">
              {autostartError
                ? "Windows startup registration failed. Try again."
                : "Start the widget when you sign in to Windows."}
            </span>
          </span>
          <input
            type="checkbox"
            checked={launchAtStartup}
            onChange={(event) => updateAutostart(event.target.checked)}
          />
        </label>
      </section>
    </main>
  );
}

function WidgetView() {
  const [mediaState, setMediaState] = useState<MediaState>(NO_MEDIA_STATE);
  const [appError, setAppError] = useState<AppErrorCode | null>(null);
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);
  const [displayedArtworkUrl, setDisplayedArtworkUrl] = useState<string | null>(
    NO_MEDIA_STATE.artworkUrl,
  );
  const [incomingArtworkUrl, setIncomingArtworkUrl] = useState<string | null>(null);
  const [isArtworkTransitioning, setIsArtworkTransitioning] = useState(false);
  const [failedArtworkUrls, setFailedArtworkUrls] = useState<string[]>([]);
  const [isLyricsExpanded, setIsLyricsExpanded] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);
  const [displayPreferences, setDisplayPreferences] =
    useState<DisplayPreferences>(loadDisplayPreferences);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen<DisplayPreferences>("display-preferences-changed", (event) => {
      if (!disposed) {
        setDisplayPreferences(event.payload);
      }
    }).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        unlisten = stopListening;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    void invoke("set_compact_width", {
      width: displayPreferences.width,
      expanded: isLyricsExpanded,
    });
  }, [displayPreferences.width, isLyricsExpanded]);

  useEffect(() => {
    document.documentElement.style.setProperty(
      "--lyrics-font-size",
      `${displayPreferences.lyricsFontSize}px`,
    );
  }, [displayPreferences.lyricsFontSize]);

  useEffect(() => {
    const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotionPreference = () => {
      setPrefersReducedMotion(motionQuery.matches);
    };

    updateMotionPreference();

    if (typeof motionQuery.addEventListener === "function") {
      motionQuery.addEventListener("change", updateMotionPreference);

      return () => {
        motionQuery.removeEventListener("change", updateMotionPreference);
      };
    }

    motionQuery.addListener(updateMotionPreference);

    return () => {
      motionQuery.removeListener(updateMotionPreference);
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let receivedMediaEvent = false;
    let unlisten: (() => void) | undefined;

    void listen<MediaState>("media-state-changed", (event) => {
      if (!disposed) {
        receivedMediaEvent = true;
        setMediaState(event.payload);
        setFailedArtworkUrls((currentFailedArtworkUrls) =>
          event.payload.artworkUrl
            ? currentFailedArtworkUrls.filter(
                (failedArtworkUrl) => failedArtworkUrl !== event.payload.artworkUrl,
              )
            : currentFailedArtworkUrls,
        );
        setAppError(null);
      }
    }).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        unlisten = stopListening;
        void invoke<MediaState>("get_media_state")
          .then((snapshot) => {
            if (!disposed && !receivedMediaEvent) {
              setMediaState(snapshot);
            }
          })
          .catch(() => {
            if (!disposed) {
              setAppError("bridgeUnavailable");
            }
          });
      }
    }).catch(() => {
      if (!disposed) {
        setAppError("bridgeUnavailable");
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    setFailedArtworkUrls([]);
  }, [mediaState.sourceApp, mediaState.trackId]);

  useEffect(() => {
    if (mediaState.trackId === null && isLyricsExpanded) {
      setIsLyricsExpanded(false);
      void invoke("set_compact_expanded", { expanded: false });
    }
  }, [isLyricsExpanded, mediaState.trackId]);

  useEffect(() => {
    if (mediaState.artworkUrl === displayedArtworkUrl) {
      setIncomingArtworkUrl(null);
      setIsArtworkTransitioning(false);
      return;
    }

    if (prefersReducedMotion) {
      setDisplayedArtworkUrl(mediaState.artworkUrl);
      setIncomingArtworkUrl(null);
      setIsArtworkTransitioning(false);
      return;
    }

    setIncomingArtworkUrl(mediaState.artworkUrl);
    setIsArtworkTransitioning(true);

    const timeoutId = window.setTimeout(() => {
      setDisplayedArtworkUrl(mediaState.artworkUrl);
      setIncomingArtworkUrl(null);
      setIsArtworkTransitioning(false);
    }, ARTWORK_FADE_MS);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [displayedArtworkUrl, mediaState.artworkUrl, prefersReducedMotion]);

  const hasActiveSession = mediaState.trackId !== null;
  const errorCode = mediaState.error?.code ?? appError;
  const errorCopy = errorCode ? getErrorCopy(errorCode) : null;
  const handleArtworkError = (artworkUrl: string | null) => {
    if (!artworkUrl) {
      return;
    }

    setFailedArtworkUrls((currentFailedArtworkUrls) => {
      if (currentFailedArtworkUrls.includes(artworkUrl)) {
        return currentFailedArtworkUrls;
      }

      return [...currentFailedArtworkUrls, artworkUrl];
    });
  };
  const currentArtworkUrl =
    displayedArtworkUrl && !failedArtworkUrls.includes(displayedArtworkUrl)
      ? displayedArtworkUrl
      : null;
  const nextArtworkUrl =
    incomingArtworkUrl && !failedArtworkUrls.includes(incomingArtworkUrl)
      ? incomingArtworkUrl
      : null;
  const lyricsTeaser = getLyricsTeaser(mediaState.lyrics);
  const lyricsTone = mediaState.lyrics.status.toLowerCase();
  const lyricsFocusText = lyricsTeaser || "Current lyrics";
  const lyricsFocusKey = [
    mediaState.trackId ?? "no-track",
    mediaState.lyrics.status,
    mediaState.lyrics.currentLineIndex ?? "no-line",
    lyricsFocusText,
  ].join(":");
  const currentLyricsKey = [
    mediaState.trackId ?? "no-track",
    mediaState.lyrics.currentLineIndex ?? "no-line",
    mediaState.lyrics.currentLine ?? lyricsTeaser,
  ].join(":");
  const progressRatio = getProgressRatio(mediaState.positionMs, mediaState.durationMs);
  const progressMax = Math.max(mediaState.durationMs, 1);
  const progressNow = Math.min(Math.max(mediaState.positionMs, 0), progressMax);
  const progressValueText = `${formatDuration(mediaState.positionMs)} of ${formatDuration(mediaState.durationMs)}`;
  const artworkAltText = getArtworkAltText(mediaState.title, mediaState.artist);
  const toggleLyricsContext = () => {
    const nextExpanded = !isLyricsExpanded;
    setIsLyricsExpanded(nextExpanded);
    void invoke("set_compact_expanded", {
      expanded: nextExpanded,
      width: displayPreferences.width,
    }).catch(() => {
      setIsLyricsExpanded(false);
    });
  };
  const handleStripKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleLyricsContext();
    }
  };
  const handleMediaControl = (
    command: "previous_track" | "play_pause" | "next_track",
    label: string,
  ) => {
    setControlError(null);
    void invoke(command).catch(() => {
      setControlError(`${label} control unavailable`);
    });
  };
  const isPlaying = mediaState.playbackStatus === "Playing";
  const hasVisibleControls =
    displayPreferences.previous || displayPreferences.playPause || displayPreferences.next;
  const showMediaCopy = displayPreferences.title || displayPreferences.artist;
  const mediaStripClassName = [
    "media-strip",
    hasActiveSession ? "media-strip--interactive" : "",
    isLyricsExpanded ? "media-strip--expanded" : "",
    !displayPreferences.albumArt ? "media-strip--no-artwork" : "",
    !hasVisibleControls ? "media-strip--no-controls" : "",
    displayPreferences.transparentBackground ? "media-strip--transparent" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <main className="container">
      <section
        className={mediaStripClassName}
        role={hasActiveSession ? "group" : undefined}
        tabIndex={hasActiveSession ? 0 : -1}
        aria-expanded={hasActiveSession ? isLyricsExpanded : undefined}
        aria-controls={hasActiveSession ? "lyrics-context" : undefined}
        aria-label={hasActiveSession ? "Toggle lyrics context" : undefined}
        aria-live="polite"
        onClick={hasActiveSession ? toggleLyricsContext : undefined}
        onKeyDown={hasActiveSession ? handleStripKeyDown : undefined}
      >
        {displayPreferences.albumArt && (
          <div className="artwork-tile" aria-hidden={!currentArtworkUrl && !nextArtworkUrl}>
            <div className="artwork-placeholder" />
            {currentArtworkUrl && (
              <img
                className={`artwork-layer ${
                  isArtworkTransitioning
                    ? "artwork-layer--fading-out"
                    : "artwork-layer--visible"
                }`}
                src={currentArtworkUrl}
                alt={nextArtworkUrl ? "" : artworkAltText}
                aria-hidden={nextArtworkUrl ? true : undefined}
                onError={() => handleArtworkError(currentArtworkUrl)}
              />
            )}
            {nextArtworkUrl && (
              <img
                className={`artwork-layer artwork-layer--incoming ${
                  isArtworkTransitioning ? "artwork-layer--visible" : ""
                }`}
                src={nextArtworkUrl}
                alt={artworkAltText}
                onError={() => handleArtworkError(nextArtworkUrl)}
              />
            )}
          </div>
        )}
        {errorCopy ? (
          <div className="status-copy status-copy--error">
            <p className="status-title">{errorCopy.title}</p>
            <p className="status-detail">{errorCopy.detail}</p>
          </div>
        ) : hasActiveSession ? (
          <div className="media-strip__body">
            <div
              className={`media-strip__summary ${
                !showMediaCopy || !displayPreferences.lyrics
                  ? "media-strip__summary--single"
                  : ""
              }`}
            >
              {showMediaCopy && (
                <div className="media-copy">
                  {displayPreferences.title && (
                    <p className="track-title">{mediaState.title || "Untitled media"}</p>
                  )}
                  {displayPreferences.artist && (
                    <p className="artist">{mediaState.artist || "Unknown artist"}</p>
                  )}
                </div>
              )}
              {displayPreferences.lyrics && (
                <div className="lyric-focus">
                  <p
                    key={lyricsFocusKey}
                    className={`lyric-focus__text lyric-focus__text--${lyricsTone}`}
                  >
                    {lyricsFocusText}
                  </p>
                </div>
              )}
            </div>
            {displayPreferences.lyrics && isLyricsExpanded && (
              <section
                id="lyrics-context"
                className="lyrics-context"
                aria-label="Lyrics context"
                onClick={(event) => event.stopPropagation()}
              >
                <div className="lyrics-context__header">
                  <p className="lyrics-context__title">Lyrics</p>
                  <button
                    className="lyrics-context__toggle"
                    type="button"
                    onClick={toggleLyricsContext}
                    onKeyDown={(event) => event.stopPropagation()}
                    aria-label="Hide lyrics context"
                  >
                    Hide
                  </button>
                </div>
                <div className="lyrics-context__lines">
                  <p className="lyrics-line lyrics-line--previous">
                    <span className="lyrics-line__label">Previous</span>
                    <span>{getLyricsLine(mediaState.lyrics.previousLine, "—")}</span>
                  </p>
                  <p key={currentLyricsKey} className="lyrics-line lyrics-line--current">
                    <span className="lyrics-line__label">Current</span>
                    <span>
                      {getLyricsLine(mediaState.lyrics.currentLine, lyricsTeaser || "—")}
                    </span>
                  </p>
                  <p className="lyrics-line lyrics-line--next">
                    <span className="lyrics-line__label">Next</span>
                    <span>{getLyricsLine(mediaState.lyrics.nextLine, "—")}</span>
                  </p>
                </div>
              </section>
            )}
            {displayPreferences.progress && (
              <div
                className="progress-rail"
                role="progressbar"
                aria-label="Playback progress"
                aria-valuemin={0}
                aria-valuemax={progressMax}
                aria-valuenow={progressNow}
                aria-valuetext={progressValueText}
              >
                <span
                  className="progress-rail__value"
                  style={{ width: `${progressRatio * 100}%` }}
                />
              </div>
            )}
          </div>
        ) : (
          <div className="status-copy">
            <p className="status-title">No active media session</p>
            <p className="status-detail">
              Start audio or video in a Windows media app to inspect its state here.
            </p>
          </div>
        )}
        {hasActiveSession && !errorCopy && hasVisibleControls && (
          <div
            className="media-controls"
            role="group"
            aria-label="Playback controls"
            onClick={(event) => event.stopPropagation()}
            onKeyDown={(event) => event.stopPropagation()}
          >
            {displayPreferences.previous && (
              <button
                className="media-control"
                type="button"
                aria-label="Previous track"
                title="Previous track"
                onClick={() => handleMediaControl("previous_track", "Previous track")}
              >
                <MediaIcon type="previous" />
              </button>
            )}
            {displayPreferences.playPause && (
              <button
                className="media-control media-control--primary"
                type="button"
                aria-label={isPlaying ? "Pause" : "Play"}
                title={isPlaying ? "Pause" : "Play"}
                aria-pressed={isPlaying}
                onClick={() => handleMediaControl("play_pause", isPlaying ? "Pause" : "Play")}
              >
                <MediaIcon type={isPlaying ? "pause" : "play"} />
              </button>
            )}
            {displayPreferences.next && (
              <button
                className="media-control"
                type="button"
                aria-label="Next track"
                title="Next track"
                onClick={() => handleMediaControl("next_track", "Next track")}
              >
                <MediaIcon type="next" />
              </button>
            )}
          </div>
        )}
        {controlError && (
          <span className="sr-only" aria-live="polite">
            {controlError}
          </span>
        )}
      </section>
    </main>
  );
}

function App() {
  const isSettingsView = new URLSearchParams(window.location.search).get("view") === "settings";

  return isSettingsView ? <SettingsView /> : <WidgetView />;
}

export default App;
