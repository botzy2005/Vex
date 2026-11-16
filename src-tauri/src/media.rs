//! Runs the media engine on its own thread and bridges it to Tauri.

use vex_core::media::{
    run_loop, MediaEngine, MediaState, Msg, TransportCommand, UnavailableSource,
};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

pub struct MediaHandle {
    state: Arc<Mutex<MediaState>>,
    tx: mpsc::Sender<Msg>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl MediaHandle {
    pub fn state(&self) -> MediaState {
        self.state.lock().unwrap().clone()
    }

    /// Send a command to the player and wait (briefly) for its verdict.
    pub fn send(&self, cmd: TransportCommand) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Msg::Command(cmd, reply_tx))
            .map_err(|_| "The media service has stopped.".to_string())?;
        reply_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "The player did not respond in time.".to_string())?
    }
}

pub fn start(app: AppHandle) -> MediaHandle {
    let state = Arc::new(Mutex::new(MediaState::default()));
    let (tx, rx) = mpsc::channel();
    let shared = state.clone();

    std::thread::Builder::new()
        .name("vex-media".into())
        .spawn(move || {
            let on_state = move |s: &MediaState| {
                *shared.lock().unwrap() = s.clone();
                let _ = app.emit("media-state", s);
            };
            #[cfg(windows)]
            {
                crate::media_windows::init_com_for_this_thread();
                match crate::media_windows::WindowsSource::new() {
                    Ok(source) => run_loop(MediaEngine::new(source), rx, on_state, now_ms),
                    Err(e) => {
                        eprintln!("vex: Windows media sessions are unavailable: {e}");
                        let msg = format!("Windows media sessions are unavailable: {e}");
                        run_loop(
                            MediaEngine::new(UnavailableSource(msg)),
                            rx,
                            on_state,
                            now_ms,
                        )
                    }
                }
            }
            #[cfg(not(windows))]
            run_loop(
                MediaEngine::new(UnavailableSource(
                    "Media control is only available on Windows.".into(),
                )),
                rx,
                on_state,
                now_ms,
            );
        })
        .expect("failed to start the media thread");

    MediaHandle { state, tx }
}
