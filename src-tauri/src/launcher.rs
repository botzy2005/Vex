//! Quick Launcher shell: the window, the caches and the IPC commands.
//!
//! What lives where:
//! * `vex_core::launcher` decides what the results are (`search`), checks what may run (`validate`)
//!   and persists pins/recents/history (`store`).
//! * `launcher_windows` knows how to discover and start Windows apps (compiled on Windows only).
//! * This file glues them together. Searching only reads in-memory caches; discovery and file
//!   scanning happen on background threads and are never triggered by a keystroke.
//!
//! The launcher window is built once at startup (hidden) and then shown and hidden, so opening it is
//! a `show()` and a state reset in the page, not a page load.

use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};
use vex_core::{
    launcher::{
        commands::{Action, KnownFolder},
        files::{self, FileEntry, ScanLimits},
        search::{self, Inputs, Row, Target},
        store::{self, LauncherState, Recent, Remembered},
        AppEntry, AppKind,
    },
    settings::Launcher as LauncherSettings,
};

use crate::AppState;

#[cfg(windows)]
use crate::launcher_windows as sys;
#[cfg(not(windows))]
mod sys {
    //! Discovery and launching only exist on Windows; elsewhere the launcher simply has no apps.
    use super::AppEntry;
    pub fn discover_apps() -> Vec<AppEntry> {
        Vec::new()
    }
    pub fn launch_app(_app: &AppEntry) -> Result<(), String> {
        Err("Starting apps is only available on Windows.".into())
    }
    pub fn open_path(_p: &std::path::Path) -> Result<(), String> {
        Err("Opening files is only available on Windows.".into())
    }
    pub fn reveal(_p: &std::path::Path) -> Result<(), String> {
        Err("Opening Explorer is only available on Windows.".into())
    }
    pub fn open_url(_u: &str) -> Result<(), String> {
        Err("Opening the browser is only available on Windows.".into())
    }
    pub fn open_windows_settings() -> Result<(), String> {
        Err("Windows Settings is only available on Windows.".into())
    }
    pub fn app_icon(_app: &super::AppEntry) -> Option<String> {
        None
    }
}

pub const LAUNCHER: &str = "launcher";
const WIDTH: f64 = 680.0;
const HEIGHT: f64 = 540.0;
/// App discovery older than this is refreshed (in the background) when the launcher opens.
const APPS_STALE: Duration = Duration::from_secs(10 * 60);
const FILES_STALE: Duration = Duration::from_secs(10 * 60);
/// Never trust an on-disk app list older than a day.
const APPS_DISK_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Default)]
struct FileCache {
    entries: Vec<FileEntry>,
    updated: Option<u64>,
    unreadable: usize,
    truncated: bool,
}

struct Inner {
    state_path: PathBuf,
    apps_path: PathBuf,
    state: Mutex<LauncherState>,
    apps: Mutex<Vec<AppEntry>>,
    apps_updated: Mutex<Option<u64>>,
    apps_ready: AtomicBool,
    apps_refreshing: AtomicBool,
    files: Mutex<FileCache>,
    files_refreshing: AtomicBool,
    /// Icons already looked up (None = tried, none available), keyed by app id.
    icons: Mutex<HashMap<String, Option<String>>>,
}

#[derive(Clone)]
pub struct LauncherHandle {
    inner: Arc<Inner>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AppsDisk {
    updated: u64,
    apps: Vec<AppEntry>,
}

impl LauncherHandle {
    pub fn start(app: &AppHandle, dir: PathBuf, settings: &LauncherSettings) -> Self {
        let state_path = dir.join("launcher.json");
        let apps_path = dir.join("launcher-apps.json");
        let state = store::load(&state_path);

        let (apps, updated) = match std::fs::read_to_string(&apps_path).ok().and_then(|t| serde_json::from_str::<AppsDisk>(&t).ok()) {
            Some(d) => (d.apps, Some(d.updated)),
            None => (Vec::new(), None),
        };
        let ready = updated.is_some();
        let handle = Self {
            inner: Arc::new(Inner {
                state_path,
                apps_path,
                state: Mutex::new(state),
                apps: Mutex::new(apps),
                apps_updated: Mutex::new(updated),
                apps_ready: AtomicBool::new(ready),
                apps_refreshing: AtomicBool::new(false),
                files: Mutex::new(FileCache::default()),
                files_refreshing: AtomicBool::new(false),
                icons: Mutex::new(HashMap::new()),
            }),
        };
        let age = updated.map(|u| unix_now().saturating_sub(u)).unwrap_or(u64::MAX);
        if settings.enabled && settings.apps && age > APPS_DISK_MAX_AGE.as_secs() {
            handle.refresh_apps(app);
        }
        handle
    }

    fn ensure_fresh(&self, app: &AppHandle, s: &LauncherSettings) {
        if s.apps {
            let age = self.inner.apps_updated.lock().unwrap().map(|u| unix_now().saturating_sub(u)).unwrap_or(u64::MAX);
            if age > APPS_STALE.as_secs() {
                self.refresh_apps(app);
            }
        }
        if s.files {
            let age = self.inner.files.lock().unwrap().updated.map(|u| unix_now().saturating_sub(u)).unwrap_or(u64::MAX);
            if age > FILES_STALE.as_secs() {
                self.refresh_files(app, s);
            }
        }
    }

    pub fn refresh_apps(&self, app: &AppHandle) {
        if self.inner.apps_refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let inner = self.inner.clone();
        let app = app.clone();
        let spawned = std::thread::Builder::new().name("vex-launcher-apps".into()).spawn(move || {
            let found = sys::discover_apps();
            // An empty result (PowerShell failed, no Start Menu) must not wipe a good list.
            if !found.is_empty() {
                let now = unix_now();
                *inner.apps.lock().unwrap() = found.clone();
                *inner.apps_updated.lock().unwrap() = Some(now);
                inner.apps_ready.store(true, Ordering::SeqCst);
                if let Ok(text) = serde_json::to_string(&AppsDisk { updated: now, apps: found }) {
                    let tmp = inner.apps_path.with_extension("json.tmp");
                    if let Some(parent) = inner.apps_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::write(&tmp, text).is_ok() {
                        let _ = std::fs::rename(&tmp, &inner.apps_path);
                    }
                }
            } else {
                // Try again later rather than every time the launcher opens.
                *inner.apps_updated.lock().unwrap() = Some(unix_now());
                inner.apps_ready.store(true, Ordering::SeqCst);
            }
            inner.apps_refreshing.store(false, Ordering::SeqCst);
            let _ = app.emit_to(LAUNCHER, "launcher-index", ());
        });
        if spawned.is_err() {
            self.inner.apps_refreshing.store(false, Ordering::SeqCst);
        }
    }

    pub fn refresh_files(&self, app: &AppHandle, s: &LauncherSettings) {
        if self.inner.files_refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let roots = scope_roots(app, s);
        let inner = self.inner.clone();
        let app = app.clone();
        let spawned = std::thread::Builder::new().name("vex-launcher-files".into()).spawn(move || {
            let report = files::scan(&roots, ScanLimits::default());
            {
                let mut cache = inner.files.lock().unwrap();
                cache.entries = report.entries;
                cache.unreadable = report.unreadable;
                cache.truncated = report.truncated;
                cache.updated = Some(unix_now());
            }
            inner.files_refreshing.store(false, Ordering::SeqCst);
            let _ = app.emit_to(LAUNCHER, "launcher-index", ());
        });
        if spawned.is_err() {
            self.inner.files_refreshing.store(false, Ordering::SeqCst);
        }
    }

    fn save_state(&self) {
        let state = self.inner.state.lock().unwrap().clone();
        if let Err(e) = store::save(&self.inner.state_path, &state) {
            eprintln!("vex: launcher state could not be saved: {e}");
        }
    }
}

/// The folders the file index covers: the user's choice, or the standard user folders.
fn scope_roots(app: &AppHandle, s: &LauncherSettings) -> Vec<PathBuf> {
    if !s.file_folders.is_empty() {
        return s.file_folders.iter().map(PathBuf::from).collect();
    }
    let p = app.path();
    [p.desktop_dir(), p.document_dir(), p.download_dir(), p.picture_dir(), p.video_dir(), p.audio_dir()]
        .into_iter()
        .filter_map(Result::ok)
        .collect()
}

fn folder_path(app: &AppHandle, folder: KnownFolder) -> Result<PathBuf, String> {
    let p = app.path();
    match folder {
        KnownFolder::Downloads => p.download_dir(),
        KnownFolder::Documents => p.document_dir(),
        KnownFolder::Desktop => p.desktop_dir(),
        KnownFolder::Pictures => p.picture_dir(),
        KnownFolder::Videos => p.video_dir(),
        KnownFolder::Music => p.audio_dir(),
    }
    .map_err(|_| "Windows did not tell me where that folder is.".to_string())
}

fn launcher_settings(app: &AppHandle) -> LauncherSettings {
    app.state::<AppState>().settings.lock().unwrap().launcher.clone()
}

// ---------------------------------------------------------------- window

/// Build the (hidden) launcher window. Called once from `setup`.
pub fn build_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LAUNCHER, WebviewUrl::App("launcher.html".into()))
        .title("VEX Launcher")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .build()?;
    Ok(())
}

/// Centre horizontally in the upper part of the monitor the pointer is on.
fn place(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LAUNCHER) else { return };
    let monitors = app.available_monitors().unwrap_or_default();
    let cursor = app.cursor_position().ok();
    let chosen = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let (p, s) = (m.position(), m.size());
                c.x >= p.x as f64 && c.x < (p.x + s.width as i32) as f64 && c.y >= p.y as f64 && c.y < (p.y + s.height as i32) as f64
            })
        })
        .or_else(|| monitors.first());
    let Some(m) = chosen else { return };
    let scale = m.scale_factor();
    let (w, h) = ((WIDTH * scale).round() as u32, (HEIGHT * scale).round() as u32);
    let x = m.position().x + (m.size().width as i32 - w as i32) / 2;
    let y = m.position().y + (m.size().height as f64 * 0.14) as i32;
    let _ = win.set_size(PhysicalSize::new(w, h));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

pub fn show(app: &AppHandle) {
    let s = launcher_settings(app);
    if !s.enabled {
        return;
    }
    let Some(win) = app.get_webview_window(LAUNCHER) else { return };
    place(app);
    let _ = win.show();
    let _ = win.set_focus();
    let _ = app.emit_to(LAUNCHER, "launcher-open", ());
    app.state::<LauncherHandle>().ensure_fresh(app, &s);
}

/// Shortcut behaviour: open when hidden, ask the page to close (with its exit animation) when visible.
pub fn toggle(app: &AppHandle) {
    let visible = app.get_webview_window(LAUNCHER).and_then(|w| w.is_visible().ok()).unwrap_or(false);
    if visible {
        let _ = app.emit_to(LAUNCHER, "launcher-close", ());
    } else {
        show(app);
    }
}

#[tauri::command]
pub fn launcher_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window(LAUNCHER) {
        let _ = w.hide();
    }
}

// ---------------------------------------------------------------- search

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub rows: Vec<Row>,
    /// App discovery has not finished for the first time yet.
    pub apps_loading: bool,
    /// The file index is being built or has never been built.
    pub files_loading: bool,
}

fn with_inputs<T>(h: &LauncherHandle, settings: &LauncherSettings, f: impl FnOnce(&Inputs) -> T) -> T {
    let state = h.inner.state.lock().unwrap();
    let apps = h.inner.apps.lock().unwrap();
    let files = h.inner.files.lock().unwrap();
    let inputs = Inputs {
        settings,
        state: &state,
        apps: &apps,
        apps_ready: h.inner.apps_ready.load(Ordering::SeqCst),
        files: &files.entries,
        timers_ready: false,
        notes_ready: false,
    };
    f(&inputs)
}

#[tauri::command]
pub async fn launcher_search(app: AppHandle, handle: State<'_, LauncherHandle>, query: String) -> Result<SearchResponse, String> {
    let s = launcher_settings(&app);
    if !s.enabled {
        return Ok(SearchResponse { rows: Vec::new(), apps_loading: false, files_loading: false });
    }
    let rows = with_inputs(&handle, &s, |i| search::search(&query, i));
    let apps_loading = s.apps && !handle.inner.apps_ready.load(Ordering::SeqCst);
    let files_loading = s.files && handle.inner.files.lock().unwrap().updated.is_none();
    Ok(SearchResponse { rows, apps_loading, files_loading })
}

// ---------------------------------------------------------------- running

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutcome {
    /// Shown briefly when the launcher stays open or the action needs a word of explanation.
    pub message: Option<String>,
}

/// File types Windows would *run* when opened. The launcher shows these in Explorer instead.
fn is_runnable(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    matches!(
        ext.as_str(),
        "exe" | "com" | "bat" | "cmd" | "msi" | "msp" | "scr" | "ps1" | "psm1" | "vbs" | "vbe" | "js" | "jse" | "wsf" | "wsh" | "hta" | "cpl" | "reg" | "lnk" | "url" | "pif" | "jar" | "appx" | "msix"
    )
}

#[tauri::command]
pub async fn launcher_run(
    app: AppHandle,
    handle: State<'_, LauncherHandle>,
    input: String,
    target: Target,
    reveal: Option<bool>,
) -> Result<RunOutcome, String> {
    let s = launcher_settings(&app);
    if !s.enabled {
        return Err("The launcher is switched off.".into());
    }
    with_inputs(&handle, &s, |i| search::validate(&input, &target, i))?;

    match target {
        Target::App { id } => {
            let entry = {
                let apps = handle.inner.apps.lock().unwrap();
                apps.iter().find(|a| a.id == id).cloned()
            };
            let Some(entry) = entry else {
                return Err("That app is no longer installed.".into());
            };
            if let Err(e) = sys::launch_app(&entry) {
                if entry.kind == AppKind::Desktop && !Path::new(&entry.id).exists() {
                    // The shortcut is gone: drop it everywhere so it stops being offered.
                    handle.inner.apps.lock().unwrap().retain(|a| a.id != entry.id);
                    handle.inner.state.lock().unwrap().forget_app(&entry.id);
                    handle.save_state();
                    return Err(format!("{} is no longer installed. I removed it from the list.", entry.name));
                }
                return Err(e);
            }
            if s.remember_recents {
                let now_ms = unix_now() * 1000;
                handle.inner.state.lock().unwrap().record_launch(Remembered { id: entry.id, name: entry.name, kind: entry.kind }, now_ms);
                handle.save_state();
            }
            Ok(RunOutcome { message: None })
        }
        Target::File { path } => {
            let p = PathBuf::from(&path);
            if !p.exists() {
                handle.inner.files.lock().unwrap().entries.retain(|f| f.path != path);
                return Err("That file or folder no longer exists. I removed it from the list.".into());
            }
            if reveal.unwrap_or(false) {
                sys::reveal(&p)?;
                return Ok(RunOutcome { message: None });
            }
            if p.is_file() && is_runnable(&p) {
                sys::reveal(&p)?;
                return Ok(RunOutcome { message: Some("Programs and scripts are shown in Explorer, not run from here.".into()) });
            }
            sys::open_path(&p)?;
            Ok(RunOutcome { message: None })
        }
        Target::Command { action } => run_action(&app, &s, action),
    }
}

fn run_action(app: &AppHandle, s: &LauncherSettings, action: Action) -> Result<RunOutcome, String> {
    match action {
        Action::OpenFolder { folder } => {
            let p = folder_path(app, folder)?;
            sys::open_path(&p)?;
            Ok(RunOutcome { message: None })
        }
        Action::OpenVexSettings => {
            let app = app.clone();
            // Building a window must stay off the command thread on Windows.
            std::thread::spawn(move || {
                let _ = crate::open_settings_window(&app);
            });
            Ok(RunOutcome { message: None })
        }
        Action::OpenWindowsSettings => {
            sys::open_windows_settings()?;
            Ok(RunOutcome { message: None })
        }
        Action::WebSearch { query } => {
            let url = vex_core::launcher::commands::web_search_url(s.web_engine, &query).ok_or("That search text is empty or too long.")?;
            sys::open_url(&url)?;
            Ok(RunOutcome { message: None })
        }
        // Shown or copied by the page itself; nothing to run natively.
        Action::Calc { .. } | Action::Time | Action::Date => Ok(RunOutcome { message: None }),
        Action::Timer { .. } | Action::Note { .. } => Err("Timers and notes arrive with the Widgets Hub.".into()),
        // AI commands go through launcher_ai_send so the answer can stream.
        Action::Ai { .. } => Err("AI commands are started with the AI button.".into()),
        Action::Fill { .. } | Action::Invalid => Err("That entry cannot be run.".into()),
    }
}

// ---------------------------------------------------------------- state, pins, history

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherView {
    pub pins: Vec<Remembered>,
    pub recents: Vec<Recent>,
    pub history: Vec<String>,
    pub apps_count: usize,
    pub apps_updated: Option<u64>,
    pub files_count: usize,
    pub files_updated: Option<u64>,
    pub files_truncated: bool,
    pub files_unreadable: usize,
    pub refreshing: bool,
}

#[tauri::command]
pub async fn launcher_state(handle: State<'_, LauncherHandle>) -> Result<LauncherView, String> {
    let st = handle.inner.state.lock().unwrap().clone();
    let files = handle.inner.files.lock().unwrap();
    Ok(LauncherView {
        pins: st.pins,
        recents: st.recents,
        history: st.history,
        apps_count: handle.inner.apps.lock().unwrap().len(),
        apps_updated: *handle.inner.apps_updated.lock().unwrap(),
        files_count: files.entries.len(),
        files_updated: files.updated,
        files_truncated: files.truncated,
        files_unreadable: files.unreadable,
        refreshing: handle.inner.apps_refreshing.load(Ordering::SeqCst) || handle.inner.files_refreshing.load(Ordering::SeqCst),
    })
}

#[tauri::command]
pub async fn launcher_set_pin(handle: State<'_, LauncherHandle>, id: String, pinned: bool) -> Result<(), String> {
    {
        let mut st = handle.inner.state.lock().unwrap();
        if pinned {
            let entry = handle.inner.apps.lock().unwrap().iter().find(|a| a.id == id).cloned();
            let Some(entry) = entry else {
                return Err("That app is not in the launcher's list.".into());
            };
            if !st.pin(Remembered { id: entry.id, name: entry.name, kind: entry.kind }) {
                return Err("You can pin up to 30 apps. Unpin one first.".into());
            }
        } else {
            st.unpin(&id);
        }
    }
    handle.save_state();
    Ok(())
}

/// Remember what was typed (only when the user keeps recents on).
#[tauri::command]
pub async fn launcher_record_search(app: AppHandle, handle: State<'_, LauncherHandle>, text: String) -> Result<(), String> {
    if !launcher_settings(&app).remember_recents {
        return Ok(());
    }
    handle.inner.state.lock().unwrap().record_search(&text);
    handle.save_state();
    Ok(())
}

/// Clear recent launches and search history. Pins stay.
#[tauri::command]
pub async fn launcher_clear_history(handle: State<'_, LauncherHandle>) -> Result<(), String> {
    handle.inner.state.lock().unwrap().clear_history();
    handle.save_state();
    Ok(())
}

#[tauri::command]
pub async fn launcher_refresh(app: AppHandle, handle: State<'_, LauncherHandle>) -> Result<(), String> {
    let s = launcher_settings(&app);
    handle.refresh_apps(&app);
    if s.files {
        handle.refresh_files(&app, &s);
    }
    Ok(())
}

/// Small PNG data URL for an app, or None (the page then shows a lettered tile). Looked up lazily and cached.
#[tauri::command]
pub async fn launcher_app_icon(handle: State<'_, LauncherHandle>, id: String) -> Result<Option<String>, String> {
    if let Some(hit) = handle.inner.icons.lock().unwrap().get(&id) {
        return Ok(hit.clone());
    }
    let entry = handle.inner.apps.lock().unwrap().iter().find(|a| a.id == id).cloned();
    let icon = entry.and_then(|a| sys::app_icon(&a));
    handle.inner.icons.lock().unwrap().insert(id, icon.clone());
    Ok(icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_and_scripts_are_not_run() {
        for n in ["a.exe", "b.BAT", "c.ps1", "d.msi", "e.lnk", "f.js", "g.vbs"] {
            assert!(is_runnable(Path::new(n)), "{n}");
        }
        for n in ["a.pdf", "b.docx", "c.png", "d.txt", "folder", "e.mp4"] {
            assert!(!is_runnable(Path::new(n)), "{n}");
        }
    }
}
