//! Widgets Hub shell: the dashboard window, the persisted layout, per-widget data, the island status
//! channel and the shared timer scheduler.
//!
//! What lives where:
//! * `vex_core::hub` decides what a valid layout, widget document and status are (unit-tested).
//! * `vex_core::hub::timers` is the timer state machine. It stores absolute end times and never ticks.
//! * This file owns the window, the files, the events and ONE scheduler thread.
//!
//! The Hub window is created when opened and destroyed when closed, so nothing of it runs while closed.
//! The scheduler thread is the only resident part: with no running timer it blocks with no deadline
//! (zero CPU); with running timers it wakes at the earliest end, and at least every 30 s so a sleep/wake or
//! clock change is noticed promptly.

use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use vex_core::{
    hub::{self, timers::{Timer, Timers}, Catalog, Layout, Status},
    jsonfile,
};

pub const HUB: &str = "hub";
/// Longest the scheduler sleeps while a timer is running.
const MAX_WAIT: Duration = Duration::from_secs(30);

fn unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Inner {
    catalog: Catalog,
    layout_path: PathBuf,
    data_dir: PathBuf,
    timers_path: PathBuf,
    layout: Mutex<Layout>,
    /// Guarded together with `wake` so an operation can never slip in between the scheduler's check and its sleep.
    timers: Mutex<Timers>,
    wake: Condvar,
    statuses: Mutex<BTreeMap<String, Status>>,
}

#[derive(Clone)]
pub struct HubHandle {
    inner: Arc<Inner>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    pub kind: String,
    #[serde(flatten)]
    pub status: Status,
}

impl HubHandle {
    pub fn start(app: &AppHandle, dir: PathBuf) -> Self {
        let catalog = Catalog::builtin();
        let layout_path = dir.join("hub.json");
        let (layout, recovered) = hub::load_layout(&layout_path, &catalog);
        if recovered {
            eprintln!("vex: hub.json was unreadable; it was moved aside and the default layout restored");
        }
        let timers_path = dir.join("timers.json");
        let (loaded, recovered) = jsonfile::load::<Timers>(&timers_path);
        if recovered {
            eprintln!("vex: timers.json was unreadable; it was moved aside");
        }
        let mut timers = loaded.unwrap_or_default();
        timers.sanitize();
        let handle = Self {
            inner: Arc::new(Inner {
                catalog,
                layout_path,
                data_dir: dir.join("hub-data"),
                timers_path,
                layout: Mutex::new(layout),
                timers: Mutex::new(timers),
                wake: Condvar::new(),
                statuses: Mutex::new(BTreeMap::new()),
            }),
        };
        let (a, i) = (app.clone(), handle.inner.clone());
        std::thread::Builder::new().name("vex-timers".into()).spawn(move || scheduler(a, i)).ok();
        handle
    }

    fn save_timers(&self, t: &Timers) {
        if let Err(e) = jsonfile::save(&self.inner.timers_path, t) {
            eprintln!("vex: could not save timers.json: {e}");
        }
    }

    /// Run a timer operation, persist, wake the scheduler and tell every window.
    fn timer_op<T>(&self, app: &AppHandle, f: impl FnOnce(&mut Timers, u64) -> Result<T, String>) -> Result<T, String> {
        let (out, list, finished) = {
            let mut t = lock(&self.inner.timers);
            let now = unix_ms();
            let out = f(&mut t, now)?;
            let finished = t.poll(now);
            self.save_timers(&t);
            self.inner.wake.notify_all();
            (out, t.list().to_vec(), finished)
        };
        let _ = app.emit("timers-changed", &list);
        for f in finished {
            let _ = app.emit("timer-finished", &f);
        }
        Ok(out)
    }

    fn owner_ok(&self, owner: &str) -> Result<(), String> {
        if hub::valid_kind(owner) && self.inner.catalog.get(owner).is_some() {
            Ok(())
        } else {
            Err("That widget does not exist.".into())
        }
    }

    fn status_list(&self) -> Vec<StatusEntry> {
        lock(&self.inner.statuses).iter().map(|(k, s)| StatusEntry { kind: k.clone(), status: s.clone() }).collect()
    }

    /// The Hub window is gone: its widgets can no longer be publishing anything.
    pub fn clear_statuses(&self, app: &AppHandle) {
        let had = {
            let mut s = lock(&self.inner.statuses);
            let had = !s.is_empty();
            s.clear();
            had
        };
        if had {
            let _ = app.emit("hub-status", Vec::<StatusEntry>::new());
        }
    }
}

fn scheduler(app: AppHandle, inner: Arc<Inner>) {
    let handle = HubHandle { inner: inner.clone() };
    let mut guard = lock(&inner.timers);
    loop {
        let now = unix_ms();
        let finished = guard.poll(now);
        if !finished.is_empty() {
            handle.save_timers(&guard);
            let list = guard.list().to_vec();
            drop(guard);
            let _ = app.emit("timers-changed", &list);
            for f in &finished {
                let _ = app.emit("timer-finished", f);
            }
            // Re-read everything after re-locking: an operation may have run while we were emitting.
            guard = lock(&inner.timers);
            continue;
        }
        // Check and sleep under the same lock, so a wake-up cannot be missed.
        guard = match guard.next_deadline() {
            None => inner.wake.wait(guard).unwrap_or_else(|e| e.into_inner()),
            Some(at) => {
                let wait = Duration::from_millis(at.saturating_sub(now)).min(MAX_WAIT);
                inner.wake.wait_timeout(guard, wait).unwrap_or_else(|e| e.into_inner()).0
            }
        };
    }
}

// ---------------------------------------------------------------- window

/// Must be called from an async command or another thread: building a window on the event-handler thread
/// deadlocks on Windows (same rule as Settings).
pub fn open_window(app: &AppHandle, focus: Option<&str>) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(HUB) {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        if let Some(k) = focus {
            let _ = app.emit_to(HUB, "hub-focus", k);
        }
        return Ok(());
    }
    let url = match focus {
        Some(k) if hub::valid_kind(k) => format!("hub.html?focus={k}"),
        _ => "hub.html".to_string(),
    };
    WebviewWindowBuilder::new(app, HUB, WebviewUrl::App(url.into()))
        .title("VEX Widgets")
        .inner_size(980.0, 680.0)
        .min_inner_size(640.0, 480.0)
        .center()
        .build()?;
    Ok(())
}

#[tauri::command]
pub async fn hub_open(app: AppHandle, widget: Option<String>) -> Result<(), String> {
    open_window(&app, widget.as_deref()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- layout and data

#[tauri::command]
pub fn hub_layout(h: State<HubHandle>) -> Layout {
    lock(&h.inner.layout).clone()
}

#[tauri::command]
pub fn hub_save_layout(app: AppHandle, h: State<HubHandle>, mut layout: Layout) -> Result<Layout, String> {
    layout.sanitize(&h.inner.catalog);
    jsonfile::save(&h.inner.layout_path, &layout).map_err(|_| "Could not save the dashboard layout.".to_string())?;
    *lock(&h.inner.layout) = layout.clone();
    let _ = app.emit("hub-changed", &layout);
    Ok(layout)
}

#[tauri::command]
pub fn hub_data_get(h: State<HubHandle>, kind: String) -> Result<serde_json::Value, String> {
    hub::read_data(&h.inner.data_dir, &h.inner.catalog, &kind)
}

#[tauri::command]
pub fn hub_data_set(app: AppHandle, h: State<HubHandle>, kind: String, value: serde_json::Value) -> Result<(), String> {
    hub::write_data(&h.inner.data_dir, &h.inner.catalog, &kind, &value)?;
    let _ = app.emit("hub-data-changed", &kind);
    Ok(())
}

// ---------------------------------------------------------------- island status

#[tauri::command]
pub fn hub_set_status(app: AppHandle, h: State<HubHandle>, kind: String, status: Option<Status>) -> Result<(), String> {
    h.owner_ok(&kind)?;
    {
        let mut s = lock(&h.inner.statuses);
        match status {
            Some(st) => {
                s.insert(kind, hub::clean_status(st)?);
            }
            None => {
                s.remove(&kind);
            }
        }
    }
    let _ = app.emit("hub-status", h.status_list());
    Ok(())
}

#[tauri::command]
pub fn hub_statuses(h: State<HubHandle>) -> Vec<StatusEntry> {
    h.status_list()
}

// ---------------------------------------------------------------- timers

#[tauri::command]
pub fn timers_list(h: State<HubHandle>) -> Vec<Timer> {
    lock(&h.inner.timers).list().to_vec()
}

#[tauri::command]
pub fn timer_create(app: AppHandle, h: State<HubHandle>, owner: String, label: String, duration_ms: u64) -> Result<u64, String> {
    h.owner_ok(&owner)?;
    h.timer_op(&app, |t, _| t.create(&owner, &label, duration_ms))
}

#[tauri::command]
pub fn timer_start(app: AppHandle, h: State<HubHandle>, id: u64) -> Result<(), String> {
    h.timer_op(&app, |t, now| t.start(id, now))
}

#[tauri::command]
pub fn timer_pause(app: AppHandle, h: State<HubHandle>, id: u64) -> Result<(), String> {
    h.timer_op(&app, |t, now| t.pause(id, now))
}

#[tauri::command]
pub fn timer_reset(app: AppHandle, h: State<HubHandle>, id: u64) -> Result<(), String> {
    h.timer_op(&app, |t, now| t.reset(id, now))
}

#[tauri::command]
pub fn timer_delete(app: AppHandle, h: State<HubHandle>, id: u64) -> Result<(), String> {
    h.timer_op(&app, |t, _| t.delete(id))
}

#[tauri::command]
pub fn timer_set_duration(app: AppHandle, h: State<HubHandle>, id: u64, duration_ms: u64) -> Result<(), String> {
    h.timer_op(&app, |t, now| t.set_duration(id, duration_ms, now))
}

#[tauri::command]
pub fn timer_set_label(app: AppHandle, h: State<HubHandle>, id: u64, label: String) -> Result<(), String> {
    h.timer_op(&app, |t, _| t.set_label(id, &label))
}
