//! Automatic synced-lyrics lookup against LRCLIB (https://lrclib.net).
//!
//! All decision logic is pure and testable: title/artist cleaning, URL
//! building, candidate scoring by duration and name, and a positive/negative
//! cache. The only I/O is behind the [`HttpGet`] trait, implemented with a
//! real HTTP client in the Tauri shell and with fakes in tests.
//!
//! Nothing is stored in the repository; lyrics are fetched on demand, kept in
//! memory for the session, and never written to disk.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const LRCLIB_BASE: &str = "https://lrclib.net/api";

/// What the media session told us about the playing track.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackQuery {
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// One LRCLIB record (fields we use).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    #[serde(default)]
    pub track_name: String,
    #[serde(default)]
    pub artist_name: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
}

/// Result handed to the UI. `synced` is raw LRC text; the UI parses it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResult {
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
    pub source: String,
}

/// Minimal HTTP surface. `Ok(None)` means 404 (a definite "no such record").
pub trait HttpGet {
    fn get(&self, url: &str) -> Result<Option<String>, String>;
}

// ---------------------------------------------------------------- cleaning

const NOISE_WORDS: &[&str] = &[
    "official video",
    "official music video",
    "official audio",
    "official lyric video",
    "lyric video",
    "lyrics",
    "audio",
    "video",
    "hd",
    "hq",
    "4k",
    "remastered",
    "remaster",
    "live",
    "visualizer",
    "explicit",
    "clean",
    "mv",
    "m/v",
];

/// True if a bracketed group is metadata rather than part of the song name.
fn is_noise_group(inner: &str) -> bool {
    let l = inner.to_lowercase();
    if l.starts_with("feat") || l.starts_with("ft.") || l.starts_with("ft ") || l.starts_with("with ") {
        return true;
    }
    if l.contains("remaster") || l.contains("version") && l.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    NOISE_WORDS.iter().any(|n| l == *n || l.contains(n))
}

/// Remove `(…)`/`[…]` groups that are metadata, and trailing " - Remastered 2011" style suffixes.
pub fn clean_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let chars: Vec<char> = title.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let open = chars[i];
        let close = match open {
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        };
        if let Some(close) = close {
            if let Some(rel) = chars[i + 1..].iter().position(|c| *c == close) {
                let inner: String = chars[i + 1..i + 1 + rel].iter().collect();
                if is_noise_group(&inner) {
                    i += rel + 2;
                    continue;
                }
            }
        }
        out.push(open);
        i += 1;
    }
    // " - Remastered 2011", " - Live", " - Radio Edit"
    if let Some(pos) = out.rfind(" - ") {
        let tail = out[pos + 3..].to_lowercase();
        let tail_noise = tail.contains("remaster")
            || tail.starts_with("live")
            || tail.contains("radio edit")
            || tail.contains("single version")
            || tail.contains("album version")
            || tail.contains("mono")
            || tail.contains("stereo");
        if tail_noise {
            out.truncate(pos);
        }
    }
    collapse_spaces(&out)
}

/// "Artist - Topic" (YouTube auto channels) → "Artist".
pub fn clean_artist(artist: &str) -> String {
    let a = artist.trim();
    let a = a.strip_suffix(" - Topic").unwrap_or(a);
    let a = a.strip_suffix("VEVO").map(str::trim_end).unwrap_or(a);
    collapse_spaces(a)
}

/// First credited artist: "A, B", "A & B", "A feat. B", "A x B" → "A".
pub fn primary_artist(artist: &str) -> String {
    let a = clean_artist(artist);
    let lower = a.to_lowercase();
    let mut cut = a.len();
    for sep in [", ", " & ", " feat. ", " feat ", " ft. ", " featuring ", " x ", " × ", " and ", " with "] {
        if let Some(p) = lower.find(sep) {
            cut = cut.min(p);
        }
    }
    a[..cut].trim().to_string()
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lower-case, strip punctuation: used for fuzzy equality.
pub fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------- URLs

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn get_url(title: &str, artist: &str, album: Option<&str>, duration_secs: Option<u64>) -> String {
    let mut u = format!(
        "{LRCLIB_BASE}/get?track_name={}&artist_name={}",
        percent_encode(title),
        percent_encode(artist)
    );
    if let Some(a) = album.filter(|a| !a.trim().is_empty()) {
        u.push_str(&format!("&album_name={}", percent_encode(a)));
    }
    if let Some(d) = duration_secs {
        u.push_str(&format!("&duration={d}"));
    }
    u
}

pub fn search_url(title: &str, artist: &str) -> String {
    format!(
        "{LRCLIB_BASE}/search?track_name={}&artist_name={}",
        percent_encode(title),
        percent_encode(artist)
    )
}

// ---------------------------------------------------------------- selection

/// Max difference between the player's duration and a record's duration.
pub const DURATION_TOLERANCE_SECS: f64 = 4.0;

/// Choose the best record with synced lyrics for the playing track, or None.
/// Records whose duration disagrees by more than the tolerance are rejected
/// (they are almost always a different recording with different timing).
pub fn pick_best<'a>(cands: &'a [Candidate], title: &str, artist: &str, duration_ms: Option<u64>) -> Option<&'a Candidate> {
    let want_title = normalize(&clean_title(title));
    let want_artist = normalize(&primary_artist(artist));
    let want_secs = duration_ms.map(|d| d as f64 / 1000.0);

    let mut best: Option<(&Candidate, f64)> = None;
    for c in cands {
        let has_synced = c.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty());
        if !has_synced {
            continue;
        }
        let mut score = 0.0;
        let t = normalize(&clean_title(&c.track_name));
        if t == want_title {
            score += 10.0;
        } else if word_prefix(&t, &want_title) || word_prefix(&want_title, &t) {
            score += 4.0;
        } else {
            continue; // title unrelated
        }
        let a = normalize(&primary_artist(&c.artist_name));
        if a == want_artist {
            score += 6.0;
        } else if word_prefix(&a, &want_artist) || word_prefix(&want_artist, &a) {
            score += 2.0;
        } else if !want_artist.is_empty() {
            continue; // different artist
        }
        if let (Some(w), Some(d)) = (want_secs, c.duration) {
            let diff = (w - d).abs();
            if diff > DURATION_TOLERANCE_SECS {
                continue;
            }
            score += 4.0 - diff; // closer is better
        }
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((c, score));
        }
    }
    best.map(|(c, _)| c)
}

/// `long` starts with `short` followed by a word boundary ("song extended" vs "song").
fn word_prefix(long: &str, short: &str) -> bool {
    !short.is_empty() && long.len() > short.len() && long.starts_with(short) && long.as_bytes()[short.len()] == b' '
}

fn result_from(c: &Candidate) -> LyricsResult {
    LyricsResult {
        synced: c.synced_lyrics.clone().filter(|s| !s.trim().is_empty()),
        plain: c.plain_lyrics.clone().filter(|s| !s.trim().is_empty()),
        instrumental: c.instrumental,
        source: "LRCLIB".to_string(),
    }
}

// ---------------------------------------------------------------- client

#[derive(Debug, Clone)]
enum CacheEntry {
    Hit(LyricsResult),
    /// A definite miss: do not ask again this session.
    Miss,
}

/// Lookup with an in-memory positive/negative cache. Network *errors* are
/// never cached as misses, so a temporary outage does not hide lyrics.
pub struct LyricsClient<H: HttpGet> {
    http: H,
    cache: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LookupError {
    Network(String),
}

impl<H: HttpGet> LyricsClient<H> {
    pub fn new(http: H) -> Self {
        Self { http, cache: HashMap::new() }
    }

    fn key(q: &TrackQuery) -> String {
        format!(
            "{}\u{0}{}\u{0}{}",
            normalize(&q.artist),
            normalize(&q.title),
            q.duration_ms.map(|d| d / 1000).unwrap_or(0)
        )
    }

    pub fn lookup(&mut self, q: &TrackQuery) -> Result<Option<LyricsResult>, LookupError> {
        let key = Self::key(q);
        match self.cache.get(&key) {
            Some(CacheEntry::Hit(r)) => return Ok(Some(r.clone())),
            Some(CacheEntry::Miss) => return Ok(None),
            None => {}
        }
        let found = self.lookup_uncached(q)?;
        self.cache.insert(
            key,
            match &found {
                Some(r) => CacheEntry::Hit(r.clone()),
                None => CacheEntry::Miss,
            },
        );
        Ok(found)
    }

    /// Strategy, cheapest and most exact first:
    ///  1. exact `get` with cleaned title, full artist, album and duration
    ///  2. `search` with cleaned title and primary artist, scored by duration
    ///  3. `search` with cleaned title and the raw artist (catches odd credits)
    fn lookup_uncached(&mut self, q: &TrackQuery) -> Result<Option<LyricsResult>, LookupError> {
        let title = clean_title(&q.title);
        if title.is_empty() {
            return Ok(None);
        }
        let artist_full = clean_artist(&q.artist);
        let artist_main = primary_artist(&q.artist);
        let secs = q.duration_ms.map(|d| (d as f64 / 1000.0).round() as u64);

        // 1. exact
        if !artist_full.is_empty() && secs.is_some() {
            let url = get_url(&title, &artist_full, q.album.as_deref(), secs);
            if let Some(body) = self.http.get(&url).map_err(LookupError::Network)? {
                if let Ok(c) = serde_json::from_str::<Candidate>(&body) {
                    if c.instrumental {
                        return Ok(Some(result_from(&c)));
                    }
                    if pick_best(std::slice::from_ref(&c), &q.title, &q.artist, q.duration_ms).is_some() {
                        return Ok(Some(result_from(&c)));
                    }
                }
            }
        }

        // 2 & 3. search
        let mut tried: Vec<String> = Vec::new();
        for artist in [artist_main.as_str(), artist_full.as_str(), q.artist.as_str()] {
            if tried.iter().any(|t| t == artist) {
                continue;
            }
            tried.push(artist.to_string());
            let url = search_url(&title, artist);
            let Some(body) = self.http.get(&url).map_err(LookupError::Network)? else { continue };
            let Ok(cands) = serde_json::from_str::<Vec<Candidate>>(&body) else { continue };
            if let Some(c) = pick_best(&cands, &q.title, &q.artist, q.duration_ms) {
                return Ok(Some(result_from(c)));
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[test]
    fn cleans_titles() {
        assert_eq!(clean_title("Blinding Lights (Official Video)"), "Blinding Lights");
        assert_eq!(clean_title("Song [Official Audio]"), "Song");
        assert_eq!(clean_title("Hey Jude - Remastered 2015"), "Hey Jude");
        assert_eq!(clean_title("Hello (feat. Someone)"), "Hello");
        assert_eq!(clean_title("Song (Live)"), "Song");
        assert_eq!(clean_title("Hurt - Live"), "Hurt");
        // meaningful brackets survive
        assert_eq!(clean_title("(I Can't Get No) Satisfaction"), "(I Can't Get No) Satisfaction");
        assert_eq!(clean_title("Sweet Child O' Mine"), "Sweet Child O' Mine");
        assert_eq!(clean_title("Paranoid Android"), "Paranoid Android");
        assert_eq!(clean_title("  Spaced   Out  "), "Spaced Out");
    }

    #[test]
    fn cleans_artists() {
        assert_eq!(clean_artist("Daft Punk - Topic"), "Daft Punk");
        assert_eq!(clean_artist("ArtistVEVO"), "Artist");
        assert_eq!(primary_artist("Drake, Future"), "Drake");
        assert_eq!(primary_artist("Calvin Harris & Rihanna"), "Calvin Harris");
        assert_eq!(primary_artist("Artist feat. Guest"), "Artist");
        assert_eq!(primary_artist("Solo Act"), "Solo Act");
    }

    #[test]
    fn percent_encoding() {
        assert_eq!(percent_encode("a b&c"), "a%20b%26c");
        assert_eq!(percent_encode("Beyoncé"), "Beyonc%C3%A9");
        assert_eq!(percent_encode("safe-_.~"), "safe-_.~");
    }

    #[test]
    fn builds_urls() {
        assert_eq!(
            get_url("A B", "C", Some("Alb"), Some(213)),
            "https://lrclib.net/api/get?track_name=A%20B&artist_name=C&album_name=Alb&duration=213"
        );
        assert_eq!(get_url("A", "C", Some("  "), None), "https://lrclib.net/api/get?track_name=A&artist_name=C");
        assert_eq!(search_url("A", "C"), "https://lrclib.net/api/search?track_name=A&artist_name=C");
    }

    fn cand(title: &str, artist: &str, dur: Option<f64>, synced: Option<&str>) -> Candidate {
        Candidate {
            track_name: title.into(),
            artist_name: artist.into(),
            duration: dur,
            instrumental: false,
            plain_lyrics: None,
            synced_lyrics: synced.map(Into::into),
        }
    }

    #[test]
    fn picks_closest_duration_with_synced_lyrics() {
        let cands = vec![
            cand("Song", "Artist", Some(250.0), Some("[00:01.00] far")),
            cand("Song", "Artist", Some(201.0), Some("[00:01.00] near")),
            cand("Song", "Artist", Some(200.0), None),
        ];
        let best = pick_best(&cands, "Song", "Artist", Some(200_000)).unwrap();
        assert_eq!(best.synced_lyrics.as_deref(), Some("[00:01.00] near"));
    }

    #[test]
    fn rejects_wrong_duration_title_or_artist() {
        let cands = vec![cand("Song", "Artist", Some(260.0), Some("[00:01.00] x"))];
        assert!(pick_best(&cands, "Song", "Artist", Some(200_000)).is_none(), "60s off");
        let cands = vec![cand("Other Song", "Artist", Some(200.0), Some("[00:01.00] x"))];
        assert!(pick_best(&cands, "Song", "Artist", Some(200_000)).is_none(), "title");
        let cands = vec![cand("Song", "Someone Else", Some(200.0), Some("[00:01.00] x"))];
        assert!(pick_best(&cands, "Song", "Artist", Some(200_000)).is_none(), "artist");
    }

    #[test]
    fn tolerates_decorated_names_and_missing_duration() {
        let cands = vec![cand("Song", "Artist", Some(200.0), Some("[00:01.00] x"))];
        assert!(pick_best(&cands, "Song (Official Video)", "Artist - Topic", Some(201_000)).is_some());
        assert!(pick_best(&cands, "Song", "Artist", None).is_some());
    }

    #[test]
    fn prefers_exact_title_over_substring() {
        let cands = vec![
            cand("Song Extended Mix", "Artist", Some(200.0), Some("[00:01.00] sub")),
            cand("Song", "Artist", Some(200.0), Some("[00:01.00] exact")),
        ];
        let best = pick_best(&cands, "Song", "Artist", Some(200_000)).unwrap();
        assert_eq!(best.synced_lyrics.as_deref(), Some("[00:01.00] exact"));
    }

    // ---- client with a fake HTTP layer

    struct Fake {
        routes: HashMap<String, Result<Option<String>, String>>,
        calls: RefCell<Vec<String>>,
    }
    impl Fake {
        fn new() -> Self {
            Self { routes: HashMap::new(), calls: RefCell::new(vec![]) }
        }
    }
    impl HttpGet for &Fake {
        fn get(&self, url: &str) -> Result<Option<String>, String> {
            self.calls.borrow_mut().push(url.to_string());
            self.routes.get(url).cloned().unwrap_or(Ok(None))
        }
    }

    fn q() -> TrackQuery {
        TrackQuery {
            title: "Song (Official Video)".into(),
            artist: "Artist - Topic".into(),
            album: Some("Album".into()),
            duration_ms: Some(200_000),
        }
    }

    const REC: &str = r#"{"trackName":"Song","artistName":"Artist","duration":200,"instrumental":false,"plainLyrics":"la","syncedLyrics":"[00:01.00] la"}"#;

    #[test]
    fn exact_match_wins_and_is_cached() {
        let mut f = Fake::new();
        f.routes.insert(get_url("Song", "Artist", Some("Album"), Some(200)), Ok(Some(REC.into())));
        let mut c = LyricsClient::new(&f);
        let r = c.lookup(&q()).unwrap().unwrap();
        assert_eq!(r.synced.as_deref(), Some("[00:01.00] la"));
        assert_eq!(r.source, "LRCLIB");
        assert_eq!(f.calls.borrow().len(), 1);
        c.lookup(&q()).unwrap();
        assert_eq!(f.calls.borrow().len(), 1, "second lookup is served from cache");
    }

    #[test]
    fn falls_back_to_search() {
        let mut f = Fake::new();
        f.routes.insert(search_url("Song", "Artist"), Ok(Some(format!("[{REC}]"))));
        let mut c = LyricsClient::new(&f);
        assert!(c.lookup(&q()).unwrap().is_some());
        assert_eq!(f.calls.borrow().len(), 2, "get then search");
    }

    #[test]
    fn misses_are_cached_but_errors_are_not() {
        let f = Fake::new();
        let mut c = LyricsClient::new(&f);
        assert_eq!(c.lookup(&q()).unwrap(), None);
        let n = f.calls.borrow().len();
        assert_eq!(c.lookup(&q()).unwrap(), None);
        assert_eq!(f.calls.borrow().len(), n, "miss cached");

        let mut f2 = Fake::new();
        f2.routes.insert(get_url("Song", "Artist", Some("Album"), Some(200)), Err("offline".into()));
        let mut c2 = LyricsClient::new(&f2);
        assert!(matches!(c2.lookup(&q()), Err(LookupError::Network(_))));
        let n = f2.calls.borrow().len();
        assert!(c2.lookup(&q()).is_err());
        assert!(f2.calls.borrow().len() > n, "error not cached, so it retries");
    }

    #[test]
    fn instrumental_is_reported() {
        let mut f = Fake::new();
        f.routes.insert(
            get_url("Song", "Artist", Some("Album"), Some(200)),
            Ok(Some(r#"{"trackName":"Song","artistName":"Artist","duration":200,"instrumental":true}"#.into())),
        );
        let mut c = LyricsClient::new(&f);
        let r = c.lookup(&q()).unwrap().unwrap();
        assert!(r.instrumental && r.synced.is_none());
    }

    #[test]
    fn garbage_json_is_a_miss_not_a_crash() {
        let mut f = Fake::new();
        f.routes.insert(search_url("Song", "Artist"), Ok(Some("<html>".into())));
        let mut c = LyricsClient::new(&f);
        assert_eq!(c.lookup(&q()).unwrap(), None);
    }

    #[test]
    fn empty_title_is_a_miss_without_network() {
        let f = Fake::new();
        let mut c = LyricsClient::new(&f);
        let r = c.lookup(&TrackQuery { title: "".into(), artist: "A".into(), album: None, duration_ms: None });
        assert_eq!(r.unwrap(), None);
        assert!(f.calls.borrow().is_empty());
    }
}
