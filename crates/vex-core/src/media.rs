//! Platform-neutral media-session logic (Milestone 2).
//!
//! The Windows glue (`src-tauri/src/media_windows.rs`) only *reads* the OS media session
//! and sends commands. Everything that decides what the UI sees lives here so it can be
//! unit-tested on any OS: which session to show, position maths, change detection,
//! artwork caching, and the polling loop.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub play: bool,
    pub pause: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
}

/// Mirrors `MediaSession` in `src/shared/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSession {
    pub app_id: String,
    pub title: String,
    pub artist: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u64>,
    /// Playback position at `position_updated_at`.
    pub position_ms: u64,
    /// Epoch milliseconds at which `position_ms` was valid.
    pub position_updated_at: u64,
    pub playback_rate: f64,
    pub status: PlaybackStatus,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaState {
    pub session: Option<MediaSession>,
}

/// Mirrors `TransportCommand` in `src/shared/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum TransportCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek { position_ms: u64 },
}

// ------------------------------------------------------------------ session choice

#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    /// Has a title or artist worth showing.
    pub usable: bool,
    pub status: PlaybackStatus,
    /// The session Windows itself considers current.
    pub is_current: bool,
}

/// Choose which of several media sessions the island shows.
/// Playing beats paused beats stopped; Windows' own "current" session wins ties within a
/// class; otherwise the earliest listed session wins. Sessions with nothing to show are skipped.
pub fn pick_session(candidates: &[Candidate]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.usable)
        .max_by_key(|(i, c)| {
            let class = match c.status {
                PlaybackStatus::Playing => 30u8,
                PlaybackStatus::Paused => 20,
                PlaybackStatus::Stopped => 10,
            };
            (class + if c.is_current { 5 } else { 0 }, Reverse(*i))
        })
        .map(|(i, _)| i)
}

// ------------------------------------------------------------------ timeline maths

/// Windows reports positions relative to a start time and "as of" `LastUpdatedTime`.
/// Returns `(duration, position now)` in milliseconds.
pub fn compute_timeline(
    start_ms: i64,
    end_ms: i64,
    position_ms: i64,
    last_updated_epoch_ms: Option<u64>,
    now_ms: u64,
    rate: f64,
    playing: bool,
) -> (Option<u64>, u64) {
    const MAX_EXTRAPOLATION_MS: u64 = 12 * 60 * 60 * 1000;
    let duration = (end_ms > start_ms).then(|| (end_ms - start_ms) as u64);
    let mut pos = (position_ms - start_ms).max(0) as f64;

    if playing {
        if let Some(updated) = last_updated_epoch_ms {
            if now_ms > updated && now_ms - updated <= MAX_EXTRAPOLATION_MS {
                pos += (now_ms - updated) as f64 * sanitize_rate(rate);
            }
        }
    }
    let mut pos = pos as u64;
    if let Some(d) = duration {
        pos = pos.min(d);
    }
    (duration, pos)
}

pub fn sanitize_rate(rate: f64) -> f64 {
    if rate.is_finite() && rate > 0.0 {
        rate.min(8.0)
    } else {
        1.0
    }
}

/// Windows `DateTime.UniversalTime` (100 ns ticks since 1601-01-01) to Unix epoch milliseconds.
/// Returns `None` for "never updated" sentinels (anything before the year 2000).
pub fn windows_ticks_to_epoch_ms(ticks: i64) -> Option<u64> {
    const EPOCH_DIFF_MS: i64 = 11_644_473_600_000;
    const YEAR_2000_MS: i64 = 946_684_800_000;
    let ms = ticks / 10_000 - EPOCH_DIFF_MS;
    (ms > YEAR_2000_MS).then_some(ms as u64)
}

// ------------------------------------------------------------------ change detection

/// Position differences below this are normal playback drift, not a seek.
pub const SEEK_TOLERANCE_MS: i64 = 1500;

/// Whether the UI must be told about `next`. Position alone does not trigger an update
/// (the UI extrapolates it) unless it jumped, i.e. the user seeked.
pub fn needs_emit(prev: &MediaState, next: &MediaState) -> bool {
    match (&prev.session, &next.session) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(a), Some(b)) => {
            if a.app_id != b.app_id
                || a.title != b.title
                || a.artist != b.artist
                || a.album != b.album
                || a.artwork_url != b.artwork_url
                || a.duration_ms != b.duration_ms
                || a.status != b.status
                || a.capabilities != b.capabilities
                || (a.playback_rate - b.playback_rate).abs() > f64::EPSILON
            {
                return true;
            }
            let elapsed = b.position_updated_at as i64 - a.position_updated_at as i64;
            let expected = if a.status == PlaybackStatus::Playing {
                a.position_ms as f64 + elapsed as f64 * sanitize_rate(a.playback_rate)
            } else {
                a.position_ms as f64
            };
            (b.position_ms as f64 - expected).abs() > SEEK_TOLERANCE_MS as f64
        }
    }
}

// ------------------------------------------------------------------ artwork

const MAX_ARTWORK_BYTES: usize = 6 * 1024 * 1024;

pub fn sniff_image_mime(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.starts_with(b"BM") {
        Some("image/bmp")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// `data:` URL for image bytes, or `None` if they are empty, too big or not a recognised image.
pub fn artwork_data_url(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
        return None;
    }
    let mime = sniff_image_mime(bytes)?;
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

// ------------------------------------------------------------------ engine

/// What the engine needs from the operating system. Implemented for Windows in `src-tauri`
/// and by fakes in the tests.
pub trait MediaSource {
    /// The session to show right now (artwork not included), or `None`.
    fn snapshot(&mut self, now_ms: u64) -> Result<Option<MediaSession>, String>;
    /// Thumbnail bytes of the session last returned by `snapshot`.
    fn artwork(&mut self) -> Result<Option<Vec<u8>>, String>;
    fn command(&mut self, cmd: &TransportCommand) -> Result<(), String>;
}

/// A source for platforms/situations where media sessions cannot be read.
pub struct UnavailableSource(pub String);

impl MediaSource for UnavailableSource {
    fn snapshot(&mut self, _: u64) -> Result<Option<MediaSession>, String> {
        Ok(None)
    }
    fn artwork(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }
    fn command(&mut self, _: &TransportCommand) -> Result<(), String> {
        Err(self.0.clone())
    }
}

const MAX_ARTWORK_ATTEMPTS: u32 = 10;

#[derive(Default)]
struct ArtCache {
    key: String,
    url: Option<String>,
    attempts: u32,
}

pub struct MediaEngine<S> {
    source: S,
    last: MediaState,
    art: ArtCache,
    errors_in_a_row: u32,
}

impl<S: MediaSource> MediaEngine<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            last: MediaState::default(),
            art: ArtCache::default(),
            errors_in_a_row: 0,
        }
    }

    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// The most recently emitted state.
    pub fn current(&self) -> &MediaState {
        &self.last
    }

    pub fn has_session(&self) -> bool {
        self.last.session.is_some()
    }

    /// Read the OS once. Returns the new state if (and only if) the UI needs to hear about it.
    pub fn tick(&mut self, now_ms: u64) -> Option<MediaState> {
        let mut session = match self.source.snapshot(now_ms) {
            Ok(s) => {
                self.errors_in_a_row = 0;
                s
            }
            Err(_) => {
                // A transient OS error must not make the island flicker: keep the last state
                // for a few polls, then fall back to "nothing playing".
                self.errors_in_a_row += 1;
                if self.errors_in_a_row < 4 {
                    return None;
                }
                None
            }
        };

        if let Some(s) = session.as_mut() {
            let key = format!(
                "{}\u{1}{}\u{1}{}\u{1}{}",
                s.app_id,
                s.title,
                s.artist,
                s.album.as_deref().unwrap_or("")
            );
            if key != self.art.key {
                self.art = ArtCache {
                    key,
                    url: None,
                    attempts: 0,
                };
            }
            // Players often publish the thumbnail a moment after the title, so retry a few times.
            if self.art.url.is_none() && self.art.attempts < MAX_ARTWORK_ATTEMPTS {
                self.art.attempts += 1;
                if let Ok(Some(bytes)) = self.source.artwork() {
                    self.art.url = artwork_data_url(&bytes);
                }
            }
            s.artwork_url = self.art.url.clone();
        }

        let next = MediaState { session };
        if needs_emit(&self.last, &next) {
            self.last = next.clone();
            Some(next)
        } else {
            None
        }
    }

    pub fn command(&mut self, cmd: &TransportCommand) -> Result<(), String> {
        self.source.command(cmd)
    }
}

// ------------------------------------------------------------------ polling loop

pub enum Msg {
    Command(TransportCommand, Sender<Result<(), String>>),
}

pub const POLL_ACTIVE_MS: u64 = 500;
pub const POLL_IDLE_MS: u64 = 1500;
pub const POLL_AFTER_COMMAND_MS: u64 = 120;

/// Drive the engine until the sender side of `rx` is dropped. Commands are handled immediately;
/// after one, the OS is polled quickly a few times so the UI reflects the result without waiting.
pub fn run_loop<S: MediaSource>(
    mut engine: MediaEngine<S>,
    rx: Receiver<Msg>,
    mut on_state: impl FnMut(&MediaState),
    now_ms: impl Fn() -> u64,
) {
    let mut fast_polls = 0u32;
    // Publish the initial state (even "nothing playing") only if it differs from the default.
    if let Some(s) = engine.tick(now_ms()) {
        on_state(&s);
    }
    loop {
        let interval = if fast_polls > 0 {
            POLL_AFTER_COMMAND_MS
        } else if engine.has_session() {
            POLL_ACTIVE_MS
        } else {
            POLL_IDLE_MS
        };
        match rx.recv_timeout(Duration::from_millis(interval)) {
            Ok(Msg::Command(cmd, reply)) => {
                let result = engine.command(&cmd);
                if result.is_ok() {
                    fast_polls = 5;
                }
                let _ = reply.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        fast_polls = fast_polls.saturating_sub(1);
        if let Some(s) = engine.tick(now_ms()) {
            on_state(&s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};

    fn cand(usable: bool, status: PlaybackStatus, is_current: bool) -> Candidate {
        Candidate {
            usable,
            status,
            is_current,
        }
    }
    use PlaybackStatus::*;

    // ---- session choice
    #[test]
    fn playing_beats_a_paused_current_session() {
        let c = [cand(true, Paused, true), cand(true, Playing, false)];
        assert_eq!(pick_session(&c), Some(1));
    }
    #[test]
    fn current_wins_among_playing() {
        let c = [cand(true, Playing, false), cand(true, Playing, true)];
        assert_eq!(pick_session(&c), Some(1));
    }
    #[test]
    fn ties_go_to_the_first_listed() {
        let c = [cand(true, Playing, false), cand(true, Playing, false)];
        assert_eq!(pick_session(&c), Some(0));
    }
    #[test]
    fn unusable_sessions_are_skipped_and_none_is_none() {
        assert_eq!(
            pick_session(&[cand(false, Playing, true), cand(true, Paused, false)]),
            Some(1)
        );
        assert_eq!(pick_session(&[cand(false, Playing, true)]), None);
        assert_eq!(pick_session(&[]), None);
    }
    #[test]
    fn paused_beats_stopped_even_if_stopped_is_current() {
        let c = [cand(true, Stopped, true), cand(true, Paused, false)];
        assert_eq!(pick_session(&c), Some(1));
    }

    // ---- timeline
    #[test]
    fn playing_position_is_extrapolated_from_last_update() {
        let (d, p) = compute_timeline(0, 200_000, 10_000, Some(1_000_000), 1_002_000, 1.0, true);
        assert_eq!((d, p), (Some(200_000), 12_000));
    }
    #[test]
    fn paused_position_is_not_extrapolated() {
        assert_eq!(
            compute_timeline(0, 200_000, 10_000, Some(1_000_000), 1_005_000, 1.0, false).1,
            10_000
        );
    }
    #[test]
    fn playback_rate_scales_extrapolation_and_bad_rates_fall_back() {
        assert_eq!(compute_timeline(0, 0, 0, Some(0), 0, 1.0, true).1, 0);
        assert_eq!(
            compute_timeline(0, 200_000, 0, Some(1_000_000), 1_002_000, 2.0, true).1,
            4_000
        );
        assert_eq!(
            compute_timeline(0, 200_000, 0, Some(1_000_000), 1_002_000, f64::NAN, true).1,
            2_000
        );
        assert_eq!(
            compute_timeline(0, 200_000, 0, Some(1_000_000), 1_002_000, -3.0, true).1,
            2_000
        );
    }
    #[test]
    fn unknown_duration_and_missing_update_time() {
        let (d, p) = compute_timeline(0, 0, 5_000, None, 99, 1.0, true);
        assert_eq!((d, p), (None, 5_000));
    }
    #[test]
    fn start_offset_is_subtracted_and_position_clamps_to_duration() {
        assert_eq!(
            compute_timeline(60_000, 160_000, 70_000, None, 0, 1.0, false),
            (Some(100_000), 10_000)
        );
        assert_eq!(
            compute_timeline(0, 10_000, 9_000, Some(1_000_000), 1_005_000, 1.0, true).1,
            10_000
        );
    }
    #[test]
    fn absurdly_stale_update_times_are_not_extrapolated() {
        let day = 24 * 3600 * 1000;
        assert_eq!(
            compute_timeline(0, 0, 5_000, Some(1_000_000), 1_000_000 + day, 1.0, true).1,
            5_000
        );
    }
    #[test]
    fn windows_ticks_convert_and_sentinels_are_rejected() {
        // 2024-01-01T00:00:00Z = 1_704_067_200_000 ms since Unix epoch
        let ticks = (1_704_067_200_000i64 + 11_644_473_600_000) * 10_000;
        assert_eq!(windows_ticks_to_epoch_ms(ticks), Some(1_704_067_200_000));
        assert_eq!(windows_ticks_to_epoch_ms(0), None);
        assert_eq!(windows_ticks_to_epoch_ms(-5), None);
    }

    // ---- change detection
    fn sess(title: &str, status: PlaybackStatus, pos: u64, at: u64) -> MediaSession {
        MediaSession {
            app_id: "player.exe".into(),
            title: title.into(),
            artist: "Artist".into(),
            album: None,
            artwork_url: None,
            duration_ms: Some(200_000),
            position_ms: pos,
            position_updated_at: at,
            playback_rate: 1.0,
            status,
            capabilities: Capabilities {
                play: true,
                pause: true,
                next: true,
                previous: true,
                seek: true,
            },
        }
    }
    fn st(s: MediaSession) -> MediaState {
        MediaState { session: Some(s) }
    }

    #[test]
    fn identical_and_naturally_advancing_state_is_not_re_emitted() {
        let a = st(sess("T", Playing, 10_000, 1_000));
        assert!(!needs_emit(&a, &a));
        let b = st(sess("T", Playing, 10_500, 1_500)); // half a second later, half a second further
        assert!(!needs_emit(&a, &b));
        let drift = st(sess("T", Playing, 10_900, 1_500)); // 400 ms off: within tolerance
        assert!(!needs_emit(&a, &drift));
    }
    #[test]
    fn seeks_are_detected_while_playing_and_paused() {
        let a = st(sess("T", Playing, 10_000, 1_000));
        assert!(needs_emit(&a, &st(sess("T", Playing, 90_000, 1_500))));
        assert!(needs_emit(&a, &st(sess("T", Playing, 2_000, 1_500))));
        let p = st(sess("T", Paused, 10_000, 1_000));
        assert!(!needs_emit(&p, &st(sess("T", Paused, 10_000, 9_000)))); // paused: time passing is irrelevant
        assert!(needs_emit(&p, &st(sess("T", Paused, 50_000, 9_000))));
    }
    #[test]
    fn real_changes_are_emitted() {
        let a = st(sess("T", Playing, 10_000, 1_000));
        assert!(needs_emit(&a, &st(sess("U", Playing, 10_000, 1_000))));
        assert!(needs_emit(&a, &st(sess("T", Paused, 10_000, 1_000))));
        let mut art = sess("T", Playing, 10_000, 1_000);
        art.artwork_url = Some("data:x".into());
        assert!(needs_emit(&a, &st(art)));
        let mut caps = sess("T", Playing, 10_000, 1_000);
        caps.capabilities.next = false;
        assert!(needs_emit(&a, &st(caps)));
        assert!(needs_emit(&MediaState::default(), &a));
        assert!(needs_emit(&a, &MediaState::default()));
        assert!(!needs_emit(&MediaState::default(), &MediaState::default()));
    }

    // ---- artwork
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    #[test]
    fn image_types_are_sniffed_and_encoded() {
        assert_eq!(sniff_image_mime(PNG), Some("image/png"));
        assert_eq!(
            sniff_image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("image/jpeg")
        );
        assert_eq!(
            sniff_image_mime(b"RIFF\0\0\0\0WEBPVP8 "),
            Some("image/webp")
        );
        assert_eq!(sniff_image_mime(b"<html>"), None);
        let url = artwork_data_url(PNG).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        assert_eq!(
            STANDARD
                .decode(url.trim_start_matches("data:image/png;base64,"))
                .unwrap(),
            PNG
        );
        assert!(artwork_data_url(&[]).is_none());
        assert!(artwork_data_url(b"not an image").is_none());
        assert!(artwork_data_url(&vec![0xFF; MAX_ARTWORK_BYTES + 1]).is_none());
    }

    // ---- engine with a fake OS
    #[derive(Default)]
    struct Fake {
        session: Option<MediaSession>,
        art: Option<Vec<u8>>,
        art_calls: u32,
        fail_snapshots: bool,
        commands: Vec<TransportCommand>,
    }
    impl MediaSource for Fake {
        fn snapshot(&mut self, _: u64) -> Result<Option<MediaSession>, String> {
            if self.fail_snapshots {
                return Err("boom".into());
            }
            Ok(self.session.clone())
        }
        fn artwork(&mut self) -> Result<Option<Vec<u8>>, String> {
            self.art_calls += 1;
            Ok(self.art.clone())
        }
        fn command(&mut self, c: &TransportCommand) -> Result<(), String> {
            self.commands.push(c.clone());
            Ok(())
        }
    }

    #[test]
    fn engine_emits_once_then_stays_quiet_and_fetches_artwork_once() {
        let mut e = MediaEngine::new(Fake {
            session: Some(sess("T", Playing, 0, 1_000)),
            art: Some(PNG.to_vec()),
            ..Default::default()
        });
        let first = e.tick(1_000).expect("first state is emitted");
        assert!(first
            .session
            .unwrap()
            .artwork_url
            .unwrap()
            .starts_with("data:image/png"));
        e.source_mut().session = Some(sess("T", Playing, 500, 1_500));
        assert!(e.tick(1_500).is_none());
        assert!(e.tick(2_000).is_none());
        assert_eq!(
            e.source_mut().art_calls,
            1,
            "artwork must be fetched once per track, not per poll"
        );
    }
    #[test]
    fn late_artwork_is_picked_up_and_new_tracks_refetch() {
        let mut e = MediaEngine::new(Fake {
            session: Some(sess("A", Playing, 0, 0)),
            ..Default::default()
        });
        assert!(e.tick(0).unwrap().session.unwrap().artwork_url.is_none());
        assert!(e.tick(500).is_none());
        e.source_mut().art = Some(PNG.to_vec()); // thumbnail arrives a moment later
        let withart = e.tick(1_000).expect("arrival of artwork is an update");
        assert!(withart.session.unwrap().artwork_url.is_some());

        e.source_mut().session = Some(sess("B", Playing, 0, 2_000));
        e.source_mut().art = None;
        let next = e.tick(2_000).unwrap();
        assert!(
            next.session.unwrap().artwork_url.is_none(),
            "must not reuse the previous track's cover"
        );
    }
    #[test]
    fn artwork_retries_are_bounded_and_garbage_is_ignored() {
        let mut e = MediaEngine::new(Fake {
            session: Some(sess("A", Playing, 0, 0)),
            art: Some(b"not an image".to_vec()),
            ..Default::default()
        });
        for i in 0..50 {
            e.tick(i * 10);
        }
        assert_eq!(e.source_mut().art_calls, MAX_ARTWORK_ATTEMPTS);
        assert!(e.current().session.as_ref().unwrap().artwork_url.is_none());
    }
    #[test]
    fn transient_errors_do_not_flicker_but_persistent_ones_clear_the_session() {
        let mut e = MediaEngine::new(Fake {
            session: Some(sess("A", Playing, 0, 0)),
            ..Default::default()
        });
        e.tick(0).unwrap();
        e.source_mut().fail_snapshots = true;
        for t in 1..4 {
            assert!(e.tick(t * 500).is_none(), "error #{t} should be tolerated");
        }
        let cleared = e
            .tick(2_000)
            .expect("fourth consecutive error clears the session");
        assert!(cleared.session.is_none());
        e.source_mut().fail_snapshots = false;
        assert!(
            e.tick(2_500).unwrap().session.is_some(),
            "recovers when the OS recovers"
        );
    }
    #[test]
    fn session_ending_is_reported() {
        let mut e = MediaEngine::new(Fake {
            session: Some(sess("A", Playing, 0, 0)),
            ..Default::default()
        });
        e.tick(0).unwrap();
        e.source_mut().session = None;
        assert!(e.tick(500).unwrap().session.is_none());
    }

    // ---- command wire format
    #[test]
    fn transport_commands_match_the_typescript_shape() {
        let c: TransportCommand =
            serde_json::from_str(r#"{"type":"seek","positionMs":42000}"#).unwrap();
        assert_eq!(
            c,
            TransportCommand::Seek {
                position_ms: 42_000
            }
        );
        for (j, want) in [
            (r#"{"type":"play"}"#, TransportCommand::Play),
            (r#"{"type":"pause"}"#, TransportCommand::Pause),
            (r#"{"type":"toggle"}"#, TransportCommand::Toggle),
            (r#"{"type":"next"}"#, TransportCommand::Next),
            (r#"{"type":"previous"}"#, TransportCommand::Previous),
        ] {
            assert_eq!(serde_json::from_str::<TransportCommand>(j).unwrap(), want);
        }
        assert!(serde_json::from_str::<TransportCommand>(r#"{"type":"explode"}"#).is_err());
        assert!(serde_json::from_str::<TransportCommand>(r#"{"type":"seek"}"#).is_err());
    }
    #[test]
    fn state_serialises_in_the_shape_the_ui_expects() {
        let mut s = sess("T", Playing, 5, 6);
        s.album = Some("Alb".into());
        let v = serde_json::to_value(st(s)).unwrap();
        let ses = &v["session"];
        assert_eq!(ses["appId"], "player.exe");
        assert_eq!(ses["durationMs"], 200_000);
        assert_eq!(ses["positionUpdatedAt"], 6);
        assert_eq!(ses["status"], "playing");
        assert_eq!(ses["capabilities"]["previous"], true);
        assert!(
            ses.get("artworkUrl").is_none(),
            "absent, not null, so it matches `artworkUrl?: string`"
        );
        assert!(serde_json::to_value(MediaState::default()).unwrap()["session"].is_null());
        let mut open_ended = sess("T", Playing, 5, 6);
        open_ended.duration_ms = None;
        assert!(serde_json::to_value(st(open_ended)).unwrap()["session"]["durationMs"].is_null());
    }

    // ---- threaded loop
    struct Shared(Arc<Mutex<Fake>>);
    impl MediaSource for Shared {
        fn snapshot(&mut self, n: u64) -> Result<Option<MediaSession>, String> {
            self.0.lock().unwrap().snapshot(n)
        }
        fn artwork(&mut self) -> Result<Option<Vec<u8>>, String> {
            self.0.lock().unwrap().artwork()
        }
        fn command(&mut self, c: &TransportCommand) -> Result<(), String> {
            let mut f = self.0.lock().unwrap();
            f.commands.push(c.clone());
            // A real player flips its state when asked; mimic that.
            if let (TransportCommand::Pause, Some(s)) = (c, f.session.as_mut()) {
                s.status = Paused;
            }
            Ok(())
        }
    }

    #[test]
    fn loop_publishes_state_handles_commands_and_stops_when_disconnected() {
        let fake = Arc::new(Mutex::new(Fake {
            session: Some(sess("Loop", Playing, 0, 0)),
            ..Default::default()
        }));
        let (tx, rx) = mpsc::channel();
        let (ev_tx, ev_rx) = mpsc::channel();
        let f2 = fake.clone();
        let t = std::thread::spawn(move || {
            run_loop(
                MediaEngine::new(Shared(f2)),
                rx,
                move |s| ev_tx.send(s.clone()).unwrap(),
                || 0,
            );
        });

        let first = ev_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("initial state is published");
        assert_eq!(first.session.unwrap().title, "Loop");

        let (rtx, rrx) = mpsc::channel();
        tx.send(Msg::Command(TransportCommand::Pause, rtx)).unwrap();
        assert_eq!(rrx.recv_timeout(Duration::from_secs(2)).unwrap(), Ok(()));
        let after = ev_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("state after a command arrives quickly");
        assert_eq!(after.session.unwrap().status, Paused);
        assert_eq!(fake.lock().unwrap().commands, vec![TransportCommand::Pause]);

        drop(tx);
        t.join()
            .expect("loop exits cleanly when the handle is dropped");
    }

    #[test]
    fn unavailable_source_reports_why_commands_fail() {
        let mut s = UnavailableSource("only on Windows".into());
        assert_eq!(s.snapshot(0), Ok(None));
        assert_eq!(
            s.command(&TransportCommand::Play),
            Err("only on Windows".to_string())
        );
    }
}
