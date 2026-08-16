export type MediaErrorCode =
  | "sessionManagerUnavailable"
  | "sessionManagerDisconnected";

export type MediaError = {
  code: MediaErrorCode;
};

export type LyricsStatus = "Idle" | "Loading" | "Ready" | "Unavailable" | "Error";

export type LyricsState = {
  status: LyricsStatus;
  previousLine: string | null;
  currentLine: string | null;
  nextLine: string | null;
  currentLineIndex: number | null;
  error: string | null;
};

export type MediaState = {
  trackId: string | null;
  title: string;
  artist: string;
  album: string | null;
  artworkUrl: string | null;
  sourceApp: string | null;
  playbackStatus: string;
  positionMs: number;
  durationMs: number;
  error?: MediaError | null;
  lyrics: LyricsState;
};
