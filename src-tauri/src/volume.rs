//! Windows master volume for the island's speaker panel.
//!
//! One dedicated thread owns the Core Audio objects. It starts the first time the page asks for the volume
//! (so nothing runs when the volume button is off), then blocks on a channel: Windows pushes every change
//! (keyboard keys, other apps, the Windows flyout, a different default device) as a message, so there is no
//! polling. The pure rules (rounding, clamping, stepping) live in `vex_core::volume`.

use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use vex_core::volume::VolumeState;

#[cfg_attr(not(windows), allow(dead_code))]
pub enum Cmd {
    Get,
    /// Absolute percent (any integer; clamped before it reaches Windows).
    Set(i64),
    /// Relative change in percent.
    Step(i64),
    ToggleMute,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub enum Msg {
    Cmd(Cmd, mpsc::Sender<VolumeState>),
    /// Windows reported a volume or mute change.
    Changed,
    /// The default output device changed, was added or removed.
    DeviceChanged,
}

#[derive(Default)]
pub struct VolumeHandle {
    tx: Mutex<Option<mpsc::Sender<Msg>>>,
}

impl VolumeHandle {
    fn sender(&self, app: &AppHandle) -> mpsc::Sender<Msg> {
        let mut guard = self.tx.lock().unwrap();
        if let Some(tx) = guard.as_ref() {
            return tx.clone();
        }
        let tx = start(app.clone());
        *guard = Some(tx.clone());
        tx
    }

    pub fn run(&self, app: &AppHandle, cmd: Cmd) -> Result<VolumeState, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.sender(app)
            .send(Msg::Cmd(cmd, reply_tx))
            .map_err(|_| "The volume service has stopped.".to_string())?;
        reply_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "Windows did not answer the volume request in time.".to_string())
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn publish(app: &AppHandle, last: &mut Option<VolumeState>, state: &VolumeState) {
    if last.as_ref() != Some(state) {
        *last = Some(state.clone());
        let _ = app.emit("volume-state", state);
    }
}

#[cfg(windows)]
fn start(app: AppHandle) -> mpsc::Sender<Msg> {
    use vex_core::volume::stepped;
    let (tx, rx) = mpsc::channel::<Msg>();
    let events = tx.clone();
    let _ = std::thread::Builder::new().name("vex-volume".into()).spawn(move || {
        let mut last: Option<VolumeState> = None;
        crate::volume_windows::init_com_for_this_thread();
        let mut audio = match crate::volume_windows::Audio::new(events) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("vex: Windows volume control is unavailable: {e}");
                let st = VolumeState::unavailable("Volume control is unavailable");
                publish(&app, &mut last, &st);
                for msg in rx {
                    if let Msg::Cmd(_, reply) = msg {
                        let _ = reply.send(st.clone());
                    }
                }
                return;
            }
        };
        for msg in rx {
            match msg {
                Msg::DeviceChanged => {
                    audio.rebind();
                    publish(&app, &mut last, &audio.state());
                }
                Msg::Changed => publish(&app, &mut last, &audio.state()),
                Msg::Cmd(cmd, reply) => {
                    let result = match cmd {
                        Cmd::Get => Ok(()),
                        Cmd::Set(p) => audio.set_percent(p),
                        Cmd::Step(d) => audio.set_percent(stepped(audio.state().percent, d) as i64),
                        Cmd::ToggleMute => audio.toggle_mute(),
                    };
                    let mut st = audio.state();
                    if let Err(e) = result {
                        st.error = Some(e);
                    }
                    publish(&app, &mut last, &st);
                    let _ = reply.send(st);
                }
            }
        }
    });
    tx
}

#[cfg(not(windows))]
fn start(_app: AppHandle) -> mpsc::Sender<Msg> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let _ = std::thread::Builder::new().name("vex-volume".into()).spawn(move || {
        let st = VolumeState::unavailable("Volume control is available on Windows only");
        for msg in rx {
            if let Msg::Cmd(_, reply) = msg {
                let _ = reply.send(st.clone());
            }
        }
    });
    tx
}
