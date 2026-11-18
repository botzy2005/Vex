//! Runs the notification engine on its own thread and bridges it to Tauri.

use std::sync::{mpsc, Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use vex_core::notifications::{run_loop, Msg, NotificationSource, UnavailableSource, View};
use vex_core::settings::Notifications;

pub struct NotificationHandle {
    tx: mpsc::Sender<Msg>,
    view: Arc<Mutex<Option<View>>>,
    /// App id behind each entry id, for "open the originating app".
    launch: Arc<Mutex<Vec<(u64, String)>>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl NotificationHandle {
    pub fn view(&self) -> Option<View> {
        self.view.lock().unwrap().clone()
    }
    pub fn send(&self, msg: Msg) {
        let _ = self.tx.send(msg);
    }
    pub fn configure(&self, cfg: Notifications) {
        self.send(Msg::Config(cfg));
    }
    /// The application id that posted entry `id`, if it is still in the history.
    pub fn app_id_of(&self, id: u64) -> Option<String> {
        self.launch
            .lock()
            .unwrap()
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, a)| a.clone())
    }
}

pub fn start(app: AppHandle, initial: Notifications) -> NotificationHandle {
    let (tx, rx) = mpsc::channel();
    let view: Arc<Mutex<Option<View>>> = Arc::new(Mutex::new(None));
    let launch: Arc<Mutex<Vec<(u64, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let (shared, shared_launch) = (view.clone(), launch.clone());
    let wake = tx.clone();

    std::thread::Builder::new()
        .name("vex-notifications".into())
        .spawn(move || {
            let on_view = move |v: &View| {
                *shared_launch.lock().unwrap() =
                    v.entries.iter().map(|e| (e.id, e.app_id.clone())).collect();
                *shared.lock().unwrap() = Some(v.clone());
                let _ = app.emit("notifications-view", v);
            };
            let source: Box<dyn NotificationSource> = make_source(wake);
            run_loop(source, initial, rx, on_view, |_| {}, now_ms);
        })
        .expect("failed to start the notifications thread");

    NotificationHandle { tx, view, launch }
}

#[cfg(windows)]
fn make_source(wake: mpsc::Sender<Msg>) -> Box<dyn NotificationSource> {
    crate::media_windows::init_com_for_this_thread();
    match crate::notifications_windows::WindowsSource::new(wake) {
        Ok(s) => Box::new(s),
        Err(e) => Box::new(UnavailableSource(format!(
            "Windows notifications are unavailable: {e}"
        ))),
    }
}

#[cfg(not(windows))]
fn make_source(_wake: mpsc::Sender<Msg>) -> Box<dyn NotificationSource> {
    Box::new(UnavailableSource(
        "Notifications are only available on Windows.".into(),
    ))
}
