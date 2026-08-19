use std::{
    collections::HashMap,
    fmt,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const LRCLIB_SEARCH_URL: &str = "https://lrclib.net/api/search";
const LRCLIB_GET_URL: &str = "https://lrclib.net/api/get";
const LRCLIB_USER_AGENT: &str = "TaskbarLyricsPlayer/0.1.3";
const LRCLIB_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_DURATION_DRIFT_SECONDS: f64 = 15.0;
const LYRICS_CACHE_CAPACITY: usize = 32;
const LYRICS_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub timestamp_ms: u64,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum LyricsStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Unavailable,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsState {
    pub status: LyricsStatus,
    pub previous_line: Option<String>,
    pub current_line: Option<String>,
    pub next_line: Option<String>,
    pub current_line_index: Option<usize>,
    pub error: Option<String>,
}

impl LyricsState {
    pub fn loading() -> Self {
        Self {
            status: LyricsStatus::Loading,
            ..Self::default()
        }
    }

    pub fn unavailable() -> Self {
        Self {
            status: LyricsStatus::Unavailable,
            ..Self::default()
        }
    }

    pub fn error(code: impl Into<String>) -> Self {
        Self {
            status: LyricsStatus::Error,
            error: Some(code.into()),
            ..Self::default()
        }
    }

    pub fn ready(context: LyricsContext) -> Self {
        Self {
            status: LyricsStatus::Ready,
            previous_line: context.previous_line,
            current_line: context.current_line,
            next_line: context.next_line,
            current_line_index: context.current_line_index,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricsContext {
    pub previous_line: Option<String>,
    pub current_line: Option<String>,
    pub next_line: Option<String>,
    pub current_line_index: Option<usize>,
}

#[derive(Debug)]
pub enum LyricsFetchError {
    Request(reqwest::Error),
    Http(StatusCode),
    InvalidResponse(reqwest::Error),
}

impl fmt::Display for LyricsFetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(error) => write!(formatter, "lyrics request failed: {error}"),
            Self::Http(status) => write!(formatter, "lyrics provider returned {status}"),
            Self::InvalidResponse(error) => {
                write!(formatter, "lyrics response was invalid: {error}")
            }
        }
    }
}

impl std::error::Error for LyricsFetchError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LyricsCacheKey {
    title: String,
    artist: String,
    album: String,
    duration_ms: u64,
}

impl LyricsCacheKey {
    fn new(title: &str, artist: &str, album: Option<&str>, duration_ms: u64) -> Self {
        Self {
            title: normalize_for_match(title),
            artist: normalize_for_match(artist),
            album: album.map(normalize_for_match).unwrap_or_default(),
            duration_ms,
        }
    }
}

struct LyricsCacheEntry {
    lyrics: Option<Vec<LyricLine>>,
    cached_at: Instant,
}

struct LyricsCache {
    capacity: usize,
    ttl: Duration,
    entries: HashMap<LyricsCacheKey, LyricsCacheEntry>,
}

impl LyricsCache {
    fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity,
            ttl,
            entries: HashMap::new(),
        }
    }

    fn get(&mut self, key: &LyricsCacheKey, now: Instant) -> Option<Option<Vec<LyricLine>>> {
        let is_expired = self
            .entries
            .get(key)
            .is_some_and(|entry| now.saturating_duration_since(entry.cached_at) >= self.ttl);
        if is_expired {
            self.entries.remove(key);
            return None;
        }

        self.entries.get(key).map(|entry| entry.lyrics.clone())
    }

    fn insert(&mut self, key: LyricsCacheKey, lyrics: Option<Vec<LyricLine>>, now: Instant) {
        if self.capacity == 0 {
            return;
        }

        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.cached_at)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest_key);
            }
        }

        self.entries.insert(
            key,
            LyricsCacheEntry {
                lyrics,
                cached_at: now,
            },
        );
    }
}

static LYRICS_CACHE: OnceLock<Mutex<LyricsCache>> = OnceLock::new();

fn lyrics_cache() -> &'static Mutex<LyricsCache> {
    LYRICS_CACHE
        .get_or_init(|| Mutex::new(LyricsCache::new(LYRICS_CACHE_CAPACITY, LYRICS_CACHE_TTL)))
}

fn cached_lyrics(key: &LyricsCacheKey) -> Option<Option<Vec<LyricLine>>> {
    lyrics_cache().lock().ok()?.get(key, Instant::now())
}

fn store_cached_lyrics(key: LyricsCacheKey, lyrics: Option<Vec<LyricLine>>) {
    if let Ok(mut cache) = lyrics_cache().lock() {
        cache.insert(key, lyrics, Instant::now());
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    track_name: String,
    artist_name: String,
    duration: Option<f64>,
    synced_lyrics: Option<String>,
}

pub fn parse_lrc(input: &str) -> Vec<LyricLine> {
    let mut lines = input
        .lines()
        .filter_map(parse_lrc_line)
        .flatten()
        .collect::<Vec<_>>();

    lines.sort_by_key(|line| line.timestamp_ms);
    lines
}

fn parse_lrc_line(line: &str) -> Option<Vec<LyricLine>> {
    let mut remainder = line.trim();
    let mut timestamps = Vec::new();

    while remainder.starts_with('[') {
        let closing = remainder.find(']')?;
        let tag = &remainder[1..closing];
        if let Some(timestamp_ms) = parse_timestamp(tag) {
            timestamps.push(timestamp_ms);
        }
        remainder = &remainder[closing + 1..];
    }

    let text = remainder.trim();
    if text.is_empty() || timestamps.is_empty() {
        return None;
    }

    Some(
        timestamps
            .into_iter()
            .map(|timestamp_ms| LyricLine {
                timestamp_ms,
                text: text.to_owned(),
            })
            .collect(),
    )
}

fn parse_timestamp(value: &str) -> Option<u64> {
    let (minutes, seconds) = value.split_once(':')?;
    let (whole_seconds, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let minutes = minutes.parse::<u64>().ok()?;
    let whole_seconds = whole_seconds.parse::<u64>().ok()?;
    if whole_seconds >= 60 {
        return None;
    }

    let fraction = fraction.chars().take(3).collect::<String>();
    let fraction_ms = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()?.saturating_mul(100),
        2 => fraction.parse::<u64>().ok()?.saturating_mul(10),
        _ => fraction.parse::<u64>().ok()?,
    };

    Some(
        minutes
            .saturating_mul(60_000)
            .saturating_add(whole_seconds.saturating_mul(1_000))
            .saturating_add(fraction_ms),
    )
}

pub fn synchronize(lines: &[LyricLine], position_ms: u64) -> LyricsContext {
    let current_line_index = lines
        .partition_point(|line| line.timestamp_ms <= position_ms)
        .checked_sub(1);

    let previous_line = current_line_index
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| lines.get(index))
        .map(|line| line.text.clone());
    let current_line = current_line_index
        .and_then(|index| lines.get(index))
        .map(|line| line.text.clone());
    let next_line = match current_line_index {
        Some(index) => lines.get(index + 1),
        None => lines.first(),
    }
    .map(|line| line.text.clone());

    LyricsContext {
        previous_line,
        current_line,
        next_line,
        current_line_index,
    }
}

pub async fn fetch_synced_lyrics(
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration_ms: u64,
) -> Result<Option<Vec<LyricLine>>, LyricsFetchError> {
    let cache_key = LyricsCacheKey::new(title, artist, album, duration_ms);
    if let Some(cached) = cached_lyrics(&cache_key) {
        return Ok(cached);
    }

    let result = fetch_synced_lyrics_uncached(title, artist, album, duration_ms).await;
    if let Ok(lyrics) = &result {
        store_cached_lyrics(cache_key, lyrics.clone());
    }
    result
}

async fn fetch_synced_lyrics_uncached(
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration_ms: u64,
) -> Result<Option<Vec<LyricLine>>, LyricsFetchError> {
    let client = reqwest::Client::builder()
        .user_agent(LRCLIB_USER_AGENT)
        .timeout(LRCLIB_REQUEST_TIMEOUT)
        .build()
        .map_err(LyricsFetchError::Request)?;
    let request =
        client
            .get(LRCLIB_SEARCH_URL)
            .query(&build_search_query(title, artist, album, duration_ms));

    let response = request.send().await.map_err(LyricsFetchError::Request)?;
    if !response.status().is_success() && response.status() != StatusCode::NOT_FOUND {
        return Err(LyricsFetchError::Http(response.status()));
    }

    let duration_seconds = duration_ms as f64 / 1_000.0;
    let records = if response.status() == StatusCode::NOT_FOUND {
        Vec::new()
    } else {
        response
            .json::<Vec<LrclibRecord>>()
            .await
            .map_err(LyricsFetchError::InvalidResponse)?
    };

    if let Some(lines) = resolve_search_or_exact(records, None, title, artist, duration_seconds) {
        return Ok(Some(lines));
    }

    let exact_response = client
        .get(LRCLIB_GET_URL)
        .query(&build_search_query(title, artist, album, duration_ms))
        .send()
        .await
        .map_err(LyricsFetchError::Request)?;
    if exact_response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !exact_response.status().is_success() {
        return Err(LyricsFetchError::Http(exact_response.status()));
    }

    let exact_record = exact_response
        .json::<LrclibRecord>()
        .await
        .map_err(LyricsFetchError::InvalidResponse)?;

    Ok(resolve_search_or_exact(
        Vec::new(),
        Some(exact_record),
        title,
        artist,
        duration_seconds,
    ))
}

fn build_search_query(
    title: &str,
    artist: &str,
    _album: Option<&str>,
    duration_ms: u64,
) -> Vec<(String, String)> {
    vec![
        ("track_name".to_owned(), title.to_owned()),
        ("artist_name".to_owned(), artist.to_owned()),
        (
            "duration".to_owned(),
            format!("{:.3}", duration_ms as f64 / 1_000.0),
        ),
    ]
}

fn resolve_records(
    records: Vec<LrclibRecord>,
    title: &str,
    artist: &str,
    duration_seconds: f64,
) -> Option<Vec<LyricLine>> {
    let record = select_best_record(records, title, artist, duration_seconds)?;
    let lines = parse_lrc(record.synced_lyrics.as_deref().unwrap_or_default());

    (!lines.is_empty()).then_some(lines)
}

fn resolve_search_or_exact(
    search_records: Vec<LrclibRecord>,
    exact_record: Option<LrclibRecord>,
    title: &str,
    artist: &str,
    duration_seconds: f64,
) -> Option<Vec<LyricLine>> {
    resolve_records(search_records, title, artist, duration_seconds).or_else(|| {
        exact_record
            .and_then(|record| resolve_records(vec![record], title, artist, duration_seconds))
    })
}

fn select_best_record(
    records: Vec<LrclibRecord>,
    title: &str,
    artist: &str,
    duration_seconds: f64,
) -> Option<LrclibRecord> {
    records
        .into_iter()
        .filter(|record| {
            record
                .synced_lyrics
                .as_deref()
                .is_some_and(|lyrics| !lyrics.trim().is_empty())
        })
        .filter_map(|record| {
            let title_match = normalize_for_match(&record.track_name) == normalize_for_match(title);
            let artist_match =
                normalize_for_match(&record.artist_name) == normalize_for_match(artist);
            let duration_match = record
                .duration
                .map(|duration| {
                    duration_seconds <= 0.0
                        || (duration - duration_seconds).abs() <= MAX_DURATION_DRIFT_SECONDS
                })
                .unwrap_or(true);

            (title_match && artist_match && duration_match).then_some(record)
        })
        .min_by(|left, right| {
            let left_duration = left.duration.unwrap_or_default();
            let right_duration = right.duration.unwrap_or_default();
            let left_distance = (left_duration - duration_seconds).abs();
            let right_distance = (right_duration - duration_seconds).abs();
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn normalize_for_match(value: &str) -> String {
    let mut value = value.to_lowercase();

    loop {
        let trimmed = value.trim_end();
        let Some(close) = trimmed.chars().last() else {
            break;
        };
        let open = match close {
            ')' => '(',
            ']' => '[',
            _ => break,
        };
        let Some(open_index) = trimmed.rfind(open) else {
            break;
        };
        let annotation = &trimmed[open_index + 1..trimmed.len() - close.len_utf8()];
        if !is_metadata_suffix(annotation) {
            break;
        }
        value = trimmed[..open_index].to_owned();
    }

    value
        .chars()
        .map(|character| {
            if character == '&' {
                ' '
            } else if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_metadata_suffix(value: &str) -> bool {
    let value = value.trim().to_lowercase();
    [
        "acoustic",
        "bonus",
        "demo",
        "deluxe",
        "edit",
        "explicit",
        "instrumental",
        "live",
        "mix",
        "mono",
        "official",
        "radio",
        "remaster",
        "remastered",
        "slowed",
        "sped up",
        "stereo",
        "version",
    ]
    .iter()
    .any(|suffix| value.contains(suffix))
}

#[cfg(test)]
mod tests {
    use super::{
        build_search_query, parse_lrc, resolve_records, resolve_search_or_exact,
        select_best_record, synchronize, LrclibRecord, LyricsCache, LyricsCacheKey, LyricsContext,
        LyricsState,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn parses_and_sorts_lrc_timestamps() {
        let lines = parse_lrc("[00:01.20]Second\n[00:00.05]Short fraction\n[00:00.00]First\n[00:01.205]Almost second\n[bad]Ignored");

        assert_eq!(
            lines
                .iter()
                .map(|line| (line.timestamp_ms, line.text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (0, "First"),
                (50, "Short fraction"),
                (1_200, "Second"),
                (1_205, "Almost second"),
            ]
        );
    }

    #[test]
    fn synchronizes_previous_current_and_next_context_at_boundaries() {
        let lines = parse_lrc("[00:10.00]First\n[00:20.00]Second\n[00:30.00]Third");

        let before_first = synchronize(&lines, 9_999);
        assert_eq!(before_first.previous_line, None);
        assert_eq!(before_first.current_line, None);
        assert_eq!(before_first.next_line.as_deref(), Some("First"));

        let at_first = synchronize(&lines, 10_000);
        assert_eq!(at_first.previous_line, None);
        assert_eq!(at_first.current_line.as_deref(), Some("First"));
        assert_eq!(at_first.next_line.as_deref(), Some("Second"));

        let between_lines = synchronize(&lines, 25_000);
        assert_eq!(between_lines.previous_line.as_deref(), Some("First"));
        assert_eq!(between_lines.current_line.as_deref(), Some("Second"));
        assert_eq!(between_lines.next_line.as_deref(), Some("Third"));

        let after_last = synchronize(&lines, 31_000);
        assert_eq!(after_last.previous_line.as_deref(), Some("Second"));
        assert_eq!(after_last.current_line.as_deref(), Some("Third"));
        assert_eq!(after_last.next_line, None);
    }

    #[test]
    fn synchronizing_empty_lyrics_returns_empty_context() {
        let context = synchronize(&[], 42_000);

        assert_eq!(context.previous_line, None);
        assert_eq!(context.current_line, None);
        assert_eq!(context.next_line, None);
        assert_eq!(context.current_line_index, None);
    }

    #[test]
    fn chooses_the_synced_result_with_the_closest_duration() {
        let records = vec![
            LrclibRecord {
                track_name: "Track".into(),
                artist_name: "Artist".into(),
                duration: Some(240.0),
                synced_lyrics: Some("[00:00.00]Far".into()),
            },
            LrclibRecord {
                track_name: "Track".into(),
                artist_name: "Artist".into(),
                duration: Some(180.0),
                synced_lyrics: Some("[00:00.00]Near".into()),
            },
        ];

        let selected = select_best_record(records, "Track", "Artist", 181.0)
            .expect("a matching record should be selected");

        assert_eq!(selected.duration, Some(180.0));
        assert_eq!(selected.synced_lyrics.as_deref(), Some("[00:00.00]Near"));
    }

    #[test]
    fn matches_common_metadata_variants_without_matching_wrong_duration() {
        let records = vec![
            LrclibRecord {
                track_name: "Fall Into You (Live at Studio)".into(),
                artist_name: "Ash-Anders".into(),
                duration: Some(211.0),
                synced_lyrics: Some("[00:00.00]Wrong duration".into()),
            },
            LrclibRecord {
                track_name: "Fall Into You [Remastered 2024]".into(),
                artist_name: "Ash Anders".into(),
                duration: Some(180.5),
                synced_lyrics: Some("[00:00.00]Matching lyrics".into()),
            },
        ];

        let selected = select_best_record(records, " fall-into-you ", "ASH   ANDERS", 181.0)
            .expect("metadata variants should still match");

        assert_eq!(selected.duration, Some(180.5));
        assert_eq!(
            selected.synced_lyrics.as_deref(),
            Some("[00:00.00]Matching lyrics")
        );
    }

    #[test]
    fn empty_search_response_resolves_to_unavailable_without_loading() {
        let lyrics = resolve_records(Vec::new(), "Track", "Artist", 180.0);

        assert_eq!(lyrics, None);
    }

    #[test]
    fn malformed_synced_lyrics_resolve_to_unavailable() {
        let lyrics = resolve_records(
            vec![LrclibRecord {
                track_name: "Track".into(),
                artist_name: "Artist".into(),
                duration: Some(180.0),
                synced_lyrics: Some("not an lrc document".into()),
            }],
            "Track",
            "Artist",
            180.0,
        );

        assert_eq!(lyrics, None);
    }

    #[test]
    fn provider_query_does_not_require_spotify_album_label_to_match() {
        let query = build_search_query("deserve", "Jake Clark", Some("wrong album"), 170_000);

        assert_eq!(
            query,
            vec![
                ("track_name".to_owned(), "deserve".to_owned()),
                ("artist_name".to_owned(), "Jake Clark".to_owned()),
                ("duration".to_owned(), "170.000".to_owned()),
            ]
        );
    }

    #[test]
    fn lyrics_cache_keeps_unavailable_results_and_expires_them() {
        let mut cache = LyricsCache::new(2, Duration::from_secs(30));
        let key = LyricsCacheKey::new("  Track (Remastered) ", "Artist", Some("Album"), 180_000);
        let start = Instant::now();

        cache.insert(key.clone(), None, start);

        assert_eq!(cache.get(&key, start + Duration::from_secs(29)), Some(None));
        assert_eq!(cache.get(&key, start + Duration::from_secs(31)), None);
    }

    #[test]
    fn lyrics_cache_evicts_the_oldest_entry_at_capacity() {
        let mut cache = LyricsCache::new(2, Duration::from_secs(30));
        let first = LyricsCacheKey::new("First", "Artist", None, 100_000);
        let second = LyricsCacheKey::new("Second", "Artist", None, 100_000);
        let third = LyricsCacheKey::new("Third", "Artist", None, 100_000);
        let start = Instant::now();

        cache.insert(first.clone(), Some(Vec::new()), start);
        cache.insert(
            second.clone(),
            Some(Vec::new()),
            start + Duration::from_secs(1),
        );
        cache.insert(
            third.clone(),
            Some(Vec::new()),
            start + Duration::from_secs(2),
        );

        assert_eq!(cache.get(&first, start + Duration::from_secs(3)), None);
        assert_eq!(
            cache.get(&second, start + Duration::from_secs(3)),
            Some(Some(Vec::new()))
        );
        assert_eq!(
            cache.get(&third, start + Duration::from_secs(3)),
            Some(Some(Vec::new()))
        );
    }

    #[test]
    fn accepts_a_reasonable_duration_drift_for_an_alternate_cut() {
        let selected = select_best_record(
            vec![LrclibRecord {
                track_name: "Maybe Next Time".into(),
                artist_name: "Jamie Miller".into(),
                duration: Some(188.0),
                synced_lyrics: Some("[00:18.00]Lyrics".into()),
            }],
            "Maybe Next Time",
            "Jamie Miller",
            180.0,
        );

        assert!(selected.is_some());
    }

    #[test]
    fn exact_record_fallback_resolves_when_search_has_no_usable_match() {
        let lyrics = resolve_search_or_exact(
            Vec::new(),
            Some(LrclibRecord {
                track_name: "Maybe Next Time".into(),
                artist_name: "Jamie Miller".into(),
                duration: Some(188.0),
                synced_lyrics: Some("[00:18.00]Lyrics".into()),
            }),
            "Maybe Next Time",
            "Jamie Miller",
            180.0,
        );

        assert_eq!(
            lyrics.map(|lines| lines[0].text.clone()),
            Some("Lyrics".to_owned())
        );
    }

    #[test]
    fn ready_state_serializes_all_three_lyrics_context_lines() {
        let state = LyricsState::ready(LyricsContext {
            previous_line: Some("Previous".into()),
            current_line: Some("Current".into()),
            next_line: Some("Next".into()),
            current_line_index: Some(1),
        });
        let serialized = serde_json::to_value(&state).expect("lyrics state should serialize");

        assert_eq!(
            serialized.get("status").and_then(|value| value.as_str()),
            Some("Ready")
        );
        assert_eq!(
            serialized
                .get("previousLine")
                .and_then(|value| value.as_str()),
            Some("Previous")
        );
        assert_eq!(
            serialized
                .get("currentLine")
                .and_then(|value| value.as_str()),
            Some("Current")
        );
        assert_eq!(
            serialized.get("nextLine").and_then(|value| value.as_str()),
            Some("Next")
        );
        assert_eq!(
            serialized
                .get("currentLineIndex")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
    }
}
