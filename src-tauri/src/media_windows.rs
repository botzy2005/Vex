//! Windows media sessions (System Media Transport Controls).
//!
//! This is the only Windows-specific media code. It reads the OS state and forwards commands;
//! all decisions (which session to show, position maths, change detection, artwork caching)
//! live in `islet_core::media` where they are unit-tested.

use islet_core::media::{
    compute_timeline, pick_session, windows_ticks_to_epoch_ms, Candidate, Capabilities,
    MediaSession, MediaSource, PlaybackStatus, TransportCommand,
};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as RawStatus,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

const TICKS_PER_MS: i64 = 10_000;
const MAX_THUMBNAIL_BYTES: u64 = 6 * 1024 * 1024;

fn err<T>(r: windows::core::Result<T>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}

/// Must run on the thread that will use the source: WinRT needs COM initialised there.
pub fn init_com_for_this_thread() {
    // S_FALSE / RPC_E_CHANGED_MODE just mean "already initialised"; either way WinRT is usable.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

pub struct WindowsSource {
    manager: Manager,
    /// The session returned by the last `snapshot`, used for artwork and commands.
    current: Option<Session>,
}

impl WindowsSource {
    pub fn new() -> Result<Self, String> {
        let manager = err(err(Manager::RequestAsync())?.join())?;
        Ok(Self {
            manager,
            current: None,
        })
    }
}

fn status_of(s: &Session) -> Option<PlaybackStatus> {
    let raw = s.GetPlaybackInfo().ok()?.PlaybackStatus().ok()?;
    if raw == RawStatus::Closed {
        None
    } else if raw == RawStatus::Playing {
        Some(PlaybackStatus::Playing)
    } else if raw == RawStatus::Stopped {
        Some(PlaybackStatus::Stopped)
    } else {
        // Opened / Changing / Paused: a track exists but is not producing sound.
        Some(PlaybackStatus::Paused)
    }
}

struct Described {
    session: Session,
    app_id: String,
    title: String,
    artist: String,
    album: Option<String>,
    status: PlaybackStatus,
}

fn describe(session: Session) -> Option<Described> {
    let status = status_of(&session)?;
    let app_id = session.SourceAppUserModelId().ok()?.to_string();
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let title = props.Title().ok()?.to_string().trim().to_string();
    let artist = props
        .Artist()
        .map(|a| a.to_string().trim().to_string())
        .unwrap_or_default();
    let album = props
        .AlbumTitle()
        .ok()
        .map(|a| a.to_string().trim().to_string())
        .filter(|a| !a.is_empty());
    Some(Described {
        session,
        app_id,
        title,
        artist,
        album,
        status,
    })
}

impl MediaSource for WindowsSource {
    fn snapshot(&mut self, now_ms: u64) -> Result<Option<MediaSession>, String> {
        let current_id = self
            .manager
            .GetCurrentSession()
            .ok()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|h| h.to_string());

        let mut all: Vec<Described> = Vec::new();
        for s in err(self.manager.GetSessions())? {
            if let Some(d) = describe(s) {
                all.push(d);
            }
        }

        let candidates: Vec<Candidate> = all
            .iter()
            .map(|d| Candidate {
                usable: !d.title.is_empty() || !d.artist.is_empty(),
                status: d.status,
                is_current: current_id.as_deref() == Some(d.app_id.as_str()),
            })
            .collect();

        let Some(idx) = pick_session(&candidates) else {
            self.current = None;
            return Ok(None);
        };
        let d = all.swap_remove(idx);

        let controls = d.session.GetPlaybackInfo().ok();
        let flag = |f: fn(
            &windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackControls,
        ) -> windows::core::Result<bool>| {
            controls
                .as_ref()
                .and_then(|i| i.Controls().ok())
                .and_then(|c| f(&c).ok())
                .unwrap_or(false)
        };
        let capabilities = Capabilities {
            play: flag(|c| c.IsPlayEnabled()),
            pause: flag(|c| c.IsPauseEnabled()),
            next: flag(|c| c.IsNextEnabled()),
            previous: flag(|c| c.IsPreviousEnabled()),
            seek: flag(|c| c.IsPlaybackPositionEnabled()),
        };
        let rate = controls
            .as_ref()
            .and_then(|i| i.PlaybackRate().ok())
            .and_then(|r| r.Value().ok())
            .unwrap_or(1.0);

        let (duration_ms, position_ms) = match d.session.GetTimelineProperties() {
            Ok(t) => {
                let ms = |r: windows::core::Result<windows::Foundation::TimeSpan>| {
                    r.map(|v| v.Duration / TICKS_PER_MS).unwrap_or(0)
                };
                compute_timeline(
                    ms(t.StartTime()),
                    ms(t.EndTime()),
                    ms(t.Position()),
                    t.LastUpdatedTime()
                        .ok()
                        .and_then(|u| windows_ticks_to_epoch_ms(u.UniversalTime)),
                    now_ms,
                    rate,
                    d.status == PlaybackStatus::Playing,
                )
            }
            Err(_) => (None, 0),
        };

        self.current = Some(d.session);
        Ok(Some(MediaSession {
            app_id: d.app_id,
            title: d.title,
            artist: d.artist,
            album: d.album,
            artwork_url: None,
            duration_ms,
            position_ms,
            position_updated_at: now_ms,
            playback_rate: rate,
            status: d.status,
            capabilities,
        }))
    }

    fn artwork(&mut self) -> Result<Option<Vec<u8>>, String> {
        let Some(session) = &self.current else {
            return Ok(None);
        };
        let props = err(err(session.TryGetMediaPropertiesAsync())?.join())?;
        let Ok(thumb) = props.Thumbnail() else {
            return Ok(None);
        }; // the player published no cover
        let stream = err(err(thumb.OpenReadAsync())?.join())?;
        let size = err(stream.Size())?;
        if size == 0 || size > MAX_THUMBNAIL_BYTES {
            return Ok(None);
        }
        let reader = err(DataReader::CreateDataReader(&stream))?;
        err(err(reader.LoadAsync(size as u32))?.join())?;
        let mut buf = vec![0u8; size as usize];
        err(reader.ReadBytes(&mut buf))?;
        Ok(Some(buf))
    }

    fn command(&mut self, cmd: &TransportCommand) -> Result<(), String> {
        let Some(session) = self.current.clone() else {
            return Err("Nothing is playing.".into());
        };
        let accepted = match cmd {
            TransportCommand::Play => err(err(session.TryPlayAsync())?.join())?,
            TransportCommand::Pause => err(err(session.TryPauseAsync())?.join())?,
            TransportCommand::Toggle => {
                // Ask the player what it is doing right now rather than trusting a stale snapshot.
                if status_of(&session) == Some(PlaybackStatus::Playing) {
                    err(err(session.TryPauseAsync())?.join())?
                } else {
                    err(err(session.TryPlayAsync())?.join())?
                }
            }
            TransportCommand::Next => err(err(session.TrySkipNextAsync())?.join())?,
            TransportCommand::Previous => err(err(session.TrySkipPreviousAsync())?.join())?,
            TransportCommand::Seek { position_ms } => {
                // Positions are absolute ticks, so add the timeline's start offset back.
                let start = session
                    .GetTimelineProperties()
                    .ok()
                    .and_then(|t| t.StartTime().ok())
                    .map(|s| s.Duration)
                    .unwrap_or(0);
                let ticks =
                    start.saturating_add((*position_ms as i64).saturating_mul(TICKS_PER_MS));
                err(err(session.TryChangePlaybackPositionAsync(ticks))?.join())?
            }
        };
        if accepted {
            Ok(())
        } else {
            Err("The player did not accept that command.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the real Windows API. A CI runner has no player open, so this checks the
    /// integration is reachable and returns a clean "nothing playing", not that audio exists.
    #[test]
    #[ignore = "needs the real Windows media APIs; CI runs it as a separate non-blocking step"]
    fn smtc_is_reachable_and_idle_machine_reports_no_session() {
        init_com_for_this_thread();
        let mut source = WindowsSource::new().expect("media session manager should be available");
        let snap = source
            .snapshot(1_700_000_000_000)
            .expect("reading sessions must not fail");
        println!("snapshot on this machine: {snap:?}");
        // If some player happens to be open its data must at least be coherent.
        if let Some(s) = snap {
            assert!(!s.app_id.is_empty());
            assert!(s.position_ms <= s.duration_ms.unwrap_or(u64::MAX));
        }
    }
}
