import { useEffect, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import type { LyricsState, MediaErrorCode, MediaState } from "./types/media";

const ARTWORK_FADE_MS = 180;
type AppErrorCode = "bridgeUnavailable";

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

function App() {
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
  const progressRatio = getProgressRatio(mediaState.positionMs, mediaState.durationMs);
  const progressMax = Math.max(mediaState.durationMs, 1);
  const progressNow = Math.min(Math.max(mediaState.positionMs, 0), progressMax);
  const progressValueText = `${formatDuration(mediaState.positionMs)} of ${formatDuration(mediaState.durationMs)}`;
  const artworkAltText = getArtworkAltText(mediaState.title, mediaState.artist);
  const toggleLyricsContext = () => {
    const nextExpanded = !isLyricsExpanded;
    setIsLyricsExpanded(nextExpanded);
    void invoke("set_compact_expanded", { expanded: nextExpanded }).catch(() => {
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

  return (
    <main className="container">
      <section
        className={`media-strip ${hasActiveSession ? "media-strip--interactive" : ""} ${
          isLyricsExpanded ? "media-strip--expanded" : ""
        }`}
        role={hasActiveSession ? "group" : undefined}
        tabIndex={hasActiveSession ? 0 : -1}
        aria-expanded={hasActiveSession ? isLyricsExpanded : undefined}
        aria-controls={hasActiveSession ? "lyrics-context" : undefined}
        aria-label={hasActiveSession ? "Toggle lyrics context" : undefined}
        aria-live="polite"
        onClick={hasActiveSession ? toggleLyricsContext : undefined}
        onKeyDown={hasActiveSession ? handleStripKeyDown : undefined}
      >
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
        {errorCopy ? (
          <div className="status-copy status-copy--error">
            <p className="status-title">{errorCopy.title}</p>
            <p className="status-detail">{errorCopy.detail}</p>
          </div>
        ) : hasActiveSession ? (
          <div className="media-strip__body">
            <div className="media-strip__summary">
              <div className="media-copy">
                <p className="track-title">{mediaState.title || "Untitled media"}</p>
                <p className="artist">{mediaState.artist || "Unknown artist"}</p>
              </div>
              <div className="lyric-focus">
                <p className={`lyric-focus__text lyric-focus__text--${lyricsTone}`}>
                  {lyricsFocusText}
                </p>
              </div>
            </div>
            {isLyricsExpanded && (
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
                  <p className="lyrics-line lyrics-line--current">
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
          </div>
        ) : (
          <div className="status-copy">
            <p className="status-title">No active media session</p>
            <p className="status-detail">
              Start audio or video in a Windows media app to inspect its state here.
            </p>
          </div>
        )}
        {hasActiveSession && !errorCopy && (
          <div
            className="media-controls"
            role="group"
            aria-label="Playback controls"
            onClick={(event) => event.stopPropagation()}
            onKeyDown={(event) => event.stopPropagation()}
          >
            <button
              className="media-control"
              type="button"
              aria-label="Previous track"
              title="Previous track"
              onClick={() => handleMediaControl("previous_track", "Previous track")}
            >
              <MediaIcon type="previous" />
            </button>
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
            <button
              className="media-control"
              type="button"
              aria-label="Next track"
              title="Next track"
              onClick={() => handleMediaControl("next_track", "Next track")}
            >
              <MediaIcon type="next" />
            </button>
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

export default App;
