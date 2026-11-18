//! Runs the clipboard engine on its own thread and bridges it to Tauri.
//!
//! The history itself lives in `vex_core::clipboard::Store`; this file only owns the thread, the
//! on-disk location and the channel. Clipboard data never travels in an event: `clipboard-changed`
//! carries only the status, and the UI asks for the list when it is shown.

use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use vex_core::clipboard::{run_loop, ClipboardSource, FileStorage, ItemView, Msg, Status, Store};
use vex_core::settings::Clipboard;

pub struct ClipboardHandle {
    tx: mpsc::Sender<Msg>,
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<Option<Status>>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl ClipboardHandle {
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.store.lock().unwrap().status(false))
    }
    pub fn list(&self, query: &str) -> Vec<ItemView> {
        self.store.lock().unwrap().list(query)
    }
    pub fn configure(&self, cfg: Clipboard) {
        let _ = self.tx.send(Msg::Config(cfg));
    }
    pub fn delete(&self, id: u64) {
        let _ = self.tx.send(Msg::Delete(id));
    }
    pub fn clear(&self, keep_pinned: bool) {
        let _ = self.tx.send(Msg::Clear(keep_pinned));
    }
    pub fn pin(&self, id: u64, pinned: bool) -> Result<(), String> {
        let (r, rx) = mpsc::channel();
        self.tx
            .send(Msg::Pin(id, pinned, r))
            .map_err(|_| "The clipboard service has stopped.".to_string())?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "The clipboard service did not respond in time.".to_string())?
    }
    /// Put an entry back on the clipboard and wait for the verdict.
    pub fn copy(&self, id: u64) -> Result<(), String> {
        let (r, rx) = mpsc::channel();
        self.tx
            .send(Msg::Copy(id, r))
            .map_err(|_| "The clipboard service has stopped.".to_string())?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "The clipboard service did not respond in time.".to_string())?
    }
}

pub fn start(app: AppHandle, initial: Clipboard, dir: std::path::PathBuf) -> ClipboardHandle {
    let (tx, rx) = mpsc::channel();
    let store = Arc::new(Mutex::new(Store::new(initial)));
    let status: Arc<Mutex<Option<Status>>> = Arc::new(Mutex::new(None));
    let (s_store, s_status) = (store.clone(), status.clone());

    std::thread::Builder::new()
        .name("vex-clipboard".into())
        .spawn(move || {
            let on_change = move |st: &Status| {
                *s_status.lock().unwrap() = Some(st.clone());
                let _ = app.emit("clipboard-changed", st);
            };
            run_loop(
                make_source(),
                s_store,
                Box::new(FileStorage::new(dir)),
                rx,
                on_change,
                now_ms,
            );
        })
        .expect("failed to start the clipboard thread");

    ClipboardHandle { tx, store, status }
}

#[cfg(windows)]
fn make_source() -> Box<dyn ClipboardSource> {
    Box::new(crate::clipboard_windows::WindowsClipboard::new())
}

#[cfg(not(windows))]
fn make_source() -> Box<dyn ClipboardSource> {
    Box::new(Unsupported)
}

/// Stand-in on platforms other than Windows: nothing is ever read or written.
#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl ClipboardSource for Unsupported {
    fn sequence(&mut self) -> u32 {
        0
    }
    fn read(&mut self, _want_images: bool) -> vex_core::clipboard::Captured {
        vex_core::clipboard::Captured::Nothing
    }
    fn write_text(&mut self, _text: &str) -> Result<(), String> {
        Err("The clipboard history is only available on Windows.".into())
    }
    fn write_image(&mut self, _image: &vex_core::clipboard::RawImage, _png: &[u8]) -> Result<(), String> {
        Err("The clipboard history is only available on Windows.".into())
    }
}
