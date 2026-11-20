//! Screenshots: capture, the region-selection overlay, saving, copying and the editor window.
//!
//! The island, the overlay and the editor are all windows of this app; they talk to this file through
//! commands and events. Pixels are captured here (GDI, physical pixels) and stay on this PC: the
//! webview only ever receives a thumbnail or, for the editor, the image the user chose to open.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;
use vex_core::clipboard::image::{decode_png_limit, encode_png, thumbnail, THUMB_EDGE};
use vex_core::clipboard::RawImage;
use vex_core::screenshot::{
    crop, default_dir, encode_as, file_name, format_for_path, from_drag, monitor_at, plan_window_capture, unique_path, with_format_extension, History,
    Rect, ShotView, Stamp, MAX_CAPTURE_PIXELS,
};
use vex_core::settings::{CaptureMode, ImageFormat, ShotHistory};

use crate::{AppState, ISLAND};

#[cfg(windows)]
use crate::screenshot_windows as platform;
#[cfg(not(windows))]
mod platform {
    //! Screenshots need Windows; everything else reports that plainly.
    use super::*;
    const MSG: &str = "Screenshots are only available on Windows.";
    pub fn capture_rect(_r: &Rect) -> Result<RawImage, String> {
        Err(MSG.into())
    }
    pub fn cursor_pos() -> Option<(i32, i32)> {
        None
    }
    pub fn local_stamp() -> Stamp {
        Stamp::from_epoch_ms_utc(super::now_ms())
    }
    pub fn set_excluded(_hwnd: *mut std::ffi::c_void, _excluded: bool) -> bool {
        false
    }
    pub fn active_window_rect() -> Result<Rect, String> {
        Err(MSG.into())
    }
}

const OVERLAY_PREFIX: &str = "overlay-";
const EDITOR: &str = "editor";
/// Reject absurd uploads from the editor (a 40-megapixel PNG is far below this).
const MAX_UPLOAD_BYTES: usize = 160 * 1024 * 1024;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// One display as it looked the moment the region capture started.
struct Frozen {
    rect: Rect,
    scale: f64,
    image: RawImage,
}

pub struct Shots {
    history: Mutex<History>,
    /// Snapshots held only while the selection overlays are open.
    pending: Mutex<Option<Vec<Frozen>>>,
}

impl Shots {
    pub fn new(mode: ShotHistory, dir: std::path::PathBuf) -> Self {
        let mut h = History::new(mode, dir);
        h.load();
        Self { history: Mutex::new(h), pending: Mutex::new(None) }
    }
    pub fn set_history_mode(&self, mode: ShotHistory) {
        self.history.lock().unwrap().set_mode(mode);
    }
}

#[derive(Deserialize)]
pub struct ShotSource {
    /// A capture held by VEX…
    id: Option<u64>,
    /// …or an image from the editor (base64 PNG).
    data: Option<String>,
}

fn settings_of(app: &AppHandle) -> vex_core::settings::Screenshot {
    app.state::<AppState>().settings.lock().unwrap().screenshot.clone()
}

fn monitors(app: &AppHandle) -> Vec<(Rect, f64)> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| (Rect::new(m.position().x, m.position().y, m.size().width, m.size().height), m.scale_factor()))
        .collect()
}

/// Run `f` with the island kept out of the picture. Windows 10 (2004) and later can exclude a window
/// from capture without it flickering; on anything older the island is hidden for a moment instead.
fn without_island<T>(app: &AppHandle, f: impl FnOnce() -> T) -> T {
    let Some(win) = app.get_webview_window(ISLAND) else { return f() };
    #[cfg(windows)]
    let hwnd: Option<*mut std::ffi::c_void> = win.hwnd().ok().map(|h| h.0);
    #[cfg(not(windows))]
    let hwnd: Option<*mut std::ffi::c_void> = None;
    let excluded = hwnd.map(|h| platform::set_excluded(h, true)).unwrap_or(false);
    let hidden = !excluded && win.hide().is_ok();
    // Give the compositor a frame to apply the change before reading the screen.
    std::thread::sleep(Duration::from_millis(if excluded { 70 } else { 140 }));
    let out = f();
    if excluded {
        if let Some(h) = hwnd {
            platform::set_excluded(h, false);
        }
    }
    if hidden {
        let _ = win.show();
    }
    out
}

/// Store a finished capture and tell the island.
fn finish(app: &AppHandle, img: RawImage) -> Result<ShotView, String> {
    let png = encode_png(&img)?;
    let thumb = encode_png(&thumbnail(&img, THUMB_EDGE * 2))?;
    let shots = app.state::<Shots>();
    let id = shots.history.lock().unwrap().add(png, img.width, img.height, &thumb, now_ms());
    let view = shots.history.lock().unwrap().view(id).ok_or("The screenshot could not be stored.")?;
    let _ = app.emit("screenshot-captured", &view);
    Ok(view)
}

/// Take a screenshot. For `Region` this opens the selection overlays and returns; the capture then
/// completes in `screenshot_region_select`.
pub fn capture(app: &AppHandle, mode: CaptureMode) -> Result<(), String> {
    let result = capture_inner(app, mode);
    if let Err(e) = &result {
        let _ = app.emit("screenshot-error", e);
    }
    result
}

fn capture_inner(app: &AppHandle, mode: CaptureMode) -> Result<(), String> {
    let mons = monitors(app);
    if mons.is_empty() {
        return Err("No display was found.".into());
    }
    let rects: Vec<Rect> = mons.iter().map(|(r, _)| *r).collect();
    match mode {
        CaptureMode::Screen => {
            let (cx, cy) = platform::cursor_pos().unwrap_or((rects[0].x, rects[0].y));
            let i = monitor_at(&rects, cx, cy).unwrap_or(0);
            let img = without_island(app, || platform::capture_rect(&rects[i]))?;
            finish(app, img).map(|_| ())
        }
        CaptureMode::Window => {
            let desktop = rects.iter().skip(1).fold(rects[0], |a, r| a.union(r));
            let window = platform::active_window_rect()?;
            let area = plan_window_capture(&window, &desktop).ok_or("That window has nothing visible to capture.")?;
            let img = without_island(app, || platform::capture_rect(&area))?;
            finish(app, img).map(|_| ())
        }
        CaptureMode::Region => {
            // Freeze every display now, so what you select is what you saw when you pressed the shortcut,
            // and the selection overlay can never end up in the picture.
            let frozen = without_island(app, || {
                mons.iter()
                    .map(|(rect, scale)| platform::capture_rect(rect).map(|image| Frozen { rect: *rect, scale: *scale, image }))
                    .collect::<Result<Vec<_>, _>>()
            })?;
            close_overlays(app);
            *app.state::<Shots>().pending.lock().unwrap() = Some(frozen);
            let handle = app.clone();
            // Building windows must not happen on the event-handler thread on Windows.
            std::thread::spawn(move || {
                if let Err(e) = open_overlays(&handle) {
                    *handle.state::<Shots>().pending.lock().unwrap() = None;
                    let _ = handle.emit("screenshot-error", e);
                }
            });
            Ok(())
        }
    }
}

fn open_overlays(app: &AppHandle) -> Result<(), String> {
    let mons = monitors(app);
    for (i, (rect, _)) in mons.iter().enumerate() {
        let label = format!("{OVERLAY_PREFIX}{i}");
        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(format!("overlay.html?i={i}").into()))
            .title("VEX screenshot")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .build()
            .map_err(|e| format!("Could not open the selection overlay: {e}"))?;
        // Place it on its own display in device pixels; a second pass fixes the size Windows may have
        // rescaled when the window crossed a display with different scaling.
        for _ in 0..2 {
            let _ = win.set_position(PhysicalPosition::new(rect.x, rect.y));
            let _ = win.set_size(PhysicalSize::new(rect.w, rect.h));
            std::thread::sleep(Duration::from_millis(40));
        }
        let _ = win.show();
        if i == 0 {
            let _ = win.set_focus();
        }
    }
    Ok(())
}

fn close_overlays(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) {
            let _ = win.close();
        }
    }
}

// ---------------------------------------------------------------- commands

#[tauri::command]
pub async fn screenshot_capture(app: AppHandle, mode: CaptureMode) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || capture(&app, mode)).await.map_err(|e| e.to_string())?
}

/// The overlay reports a finished drag: page (CSS) pixels from its own top-left corner.
#[tauri::command]
pub async fn screenshot_region_select(app: AppHandle, monitor: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Result<(), String> {
    let frozen = app.state::<Shots>().pending.lock().unwrap().take();
    close_overlays(&app);
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let frozen = frozen.ok_or("The selection has expired. Press the shortcut again.")?;
        let m = frozen.get(monitor).ok_or("That display is no longer available.")?;
        let Some(region) = from_drag(&m.rect, m.scale, x0, y0, x1, y1) else {
            return Err("The selection was too small.".into());
        };
        let img = crop(&m.image, (m.rect.x, m.rect.y), &region).ok_or("The selection was outside the screen.")?;
        drop(frozen);
        finish(&app, img).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?;
    result
}

#[tauri::command]
pub fn screenshot_region_cancel(app: AppHandle) {
    *app.state::<Shots>().pending.lock().unwrap() = None;
    close_overlays(&app);
}

#[tauri::command]
pub fn screenshot_list(shots: State<Shots>) -> Vec<ShotView> {
    shots.history.lock().unwrap().list()
}

/// The full-size image as a PNG data URL (for the editor and quick preview).
#[tauri::command]
pub async fn screenshot_image(shots: State<'_, Shots>, id: u64) -> Result<String, String> {
    let png = shots.history.lock().unwrap().png(id).ok_or("That screenshot is no longer available.")?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

#[tauri::command]
pub fn screenshot_delete(shots: State<Shots>, id: u64) {
    shots.history.lock().unwrap().delete(id);
}

#[tauri::command]
pub fn screenshot_clear(shots: State<Shots>) {
    shots.history.lock().unwrap().clear();
}

/// Decode what the editor (or the history) hands over into pixels.
fn load_source(shots: &Shots, src: &ShotSource) -> Result<(RawImage, Vec<u8>), String> {
    let png = match (&src.data, src.id) {
        (Some(data), _) => {
            let b64 = data.split_once(',').map(|(_, rest)| rest).unwrap_or(data);
            if b64.len() > MAX_UPLOAD_BYTES * 4 / 3 {
                return Err("That image is too large.".into());
            }
            STANDARD.decode(b64).map_err(|_| "The image data was not valid.".to_string())?
        }
        (None, Some(id)) => shots.history.lock().unwrap().png(id).ok_or("That screenshot is no longer available.")?,
        (None, None) => return Err("No screenshot was chosen.".into()),
    };
    let img = decode_png_limit(&png, MAX_CAPTURE_PIXELS)?;
    Ok((img, png))
}

#[tauri::command]
pub async fn screenshot_copy(shots: State<'_, Shots>, source: ShotSource) -> Result<(), String> {
    let (img, png) = load_source(&shots, &source)?;
    tauri::async_runtime::spawn_blocking(move || copy_to_clipboard(&img, &png)).await.map_err(|e| e.to_string())?
}

#[cfg(windows)]
fn copy_to_clipboard(img: &RawImage, png: &[u8]) -> Result<(), String> {
    use vex_core::clipboard::ClipboardSource;
    crate::clipboard_windows::WindowsClipboard::new().write_image(img, png)
}

#[cfg(not(windows))]
fn copy_to_clipboard(_img: &RawImage, _png: &[u8]) -> Result<(), String> {
    Err("Copying to the clipboard is only available on Windows.".into())
}

/// Save to the chosen folder (or ask where). Returns the path written, or `None` if the user cancelled.
#[tauri::command]
pub async fn screenshot_save(app: AppHandle, shots: State<'_, Shots>, source: ShotSource, ask: bool) -> Result<Option<String>, String> {
    let cfg = settings_of(&app);
    let (img, png) = load_source(&shots, &source)?;
    let pictures = app.path().picture_dir().ok().or_else(|| app.path().home_dir().ok());
    let dir = default_dir(&cfg.save_dir, pictures).ok_or("The save folder must be a full path such as C:\\Users\\you\\Pictures. Change it in Settings.")?;
    let name = file_name(&platform::local_stamp(), cfg.format);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        let _ = std::fs::create_dir_all(&dir);
        let (path, format) = if ask {
            let mut dialog = handle
                .dialog()
                .file()
                .set_title("Save screenshot")
                .set_file_name(&name)
                .add_filter("PNG image", &["png"])
                .add_filter("JPEG image", &["jpg", "jpeg"]);
            if dir.is_dir() {
                dialog = dialog.set_directory(&dir);
            }
            let Some(chosen) = dialog.blocking_save_file() else { return Ok(None) };
            let chosen = chosen.into_path().map_err(|e| e.to_string())?;
            let format = format_for_path(&chosen, cfg.format);
            (with_format_extension(&chosen, format), format)
        } else {
            (unique_path(&dir, &name, |p| p.exists()), cfg.format)
        };
        // The editor's PNG is already lossless; only convert when JPEG was asked for.
        let bytes = if format == ImageFormat::Png { png } else { encode_as(&img, format)? };
        std::fs::write(&path, bytes).map_err(|e| format!("Could not save to {}: {e}", path.display()))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Let the user pick the screenshot folder (for Settings). `None` if cancelled.
#[tauri::command]
pub async fn screenshot_pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title("Choose where screenshots are saved").blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())
}

/// Show a saved file in Explorer. Only existing files are accepted.
#[tauri::command]
pub fn screenshot_reveal(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.is_file() {
        return Err("That file no longer exists.".into());
    }
    reveal(&p)
}

#[cfg(windows)]
fn reveal(p: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn reveal(_p: &std::path::Path) -> Result<(), String> {
    Err("Opening Explorer is only available on Windows.".into())
}

/// Open (or re-use) the editor window on a capture.
#[tauri::command]
pub async fn screenshot_open_editor(app: AppHandle, id: u64) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(EDITOR) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        app.emit_to(EDITOR, "editor-load", id).map_err(|e| e.to_string())?;
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, EDITOR, WebviewUrl::App(format!("editor.html?id={id}").into()))
        .title("VEX Screenshot Editor")
        .inner_size(1120.0, 760.0)
        .min_inner_size(760.0, 520.0)
        .center()
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
