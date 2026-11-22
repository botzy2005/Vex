//! Tauri shell for VEX (Milestone 1).
//!
//! Responsibilities in this milestone: create and position the island window,
//! own the persisted settings, register the global hotkey, provide the tray
//! menu, and expose the command surface the UI talks to. Media sessions are live
//! (Milestone 2); lyrics providers and AI still report "not available yet" so the UI
//! can handle their absence honestly; they arrive in later milestones.

mod ai;
mod clipboard;
mod hub;
#[cfg(windows)]
mod display_windows;
mod launcher;
#[cfg(windows)]
mod launcher_windows;
mod screenshot;
#[cfg(windows)]
mod screenshot_windows;
#[cfg(windows)]
mod clipboard_windows;
mod lyrics;
mod media;
mod volume;
#[cfg(windows)]
mod volume_windows;
mod notifications;
mod secrets;
#[cfg(windows)]
mod secrets_windows;
#[cfg(windows)]
mod media_windows;
#[cfg(windows)]
mod notifications_windows;

use vex_core::{
    lyrics::{LyricsResult, TrackQuery},
    media::{MediaState, TransportCommand},
    placement::{clamp_offset, island_rect, pick_monitor, strip_rect, MonitorRect},
    settings::Visibility,
    Settings, SettingsStore,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub(crate) const ISLAND: &str = "island";
const SETTINGS: &str = "settings";

pub struct AppState {
    pub(crate) settings: Mutex<Settings>,
    store: SettingsStore,
    /// Logical size the UI last asked the island window to have.
    island_size: Mutex<(f64, f64)>,
    /// Offset (logical px) the island is being dragged to. Only while a drag is in progress; the saved
    /// setting is written once, when the drag ends.
    island_drag: Mutex<Option<i32>>,
    /// Auto-Hide: the island is tucked away and the window is only a thin strip on the top edge.
    /// In memory only; a restart always begins visible.
    island_hidden: Mutex<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorInfo {
    index: usize,
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
    primary: bool,
}

#[derive(Serialize)]
struct UpdateResult {
    settings: Settings,
    warnings: Vec<String>,
}

fn monitor_rects(app: &AppHandle) -> (Vec<MonitorRect>, usize) {
    let monitors = app.available_monitors().unwrap_or_default();
    let primary_pos = app.primary_monitor().ok().flatten().map(|m| *m.position());
    let rects: Vec<MonitorRect> = monitors
        .iter()
        .map(|m| MonitorRect {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
        })
        .collect();
    let primary_idx = primary_pos
        .and_then(|p| monitors.iter().position(|m| *m.position() == p))
        .unwrap_or(0);
    (rects, primary_idx)
}

/// Move/resize the island window from the current settings and requested size.
fn apply_island_geometry(app: &AppHandle) {
    let Some(win) = app.get_webview_window(ISLAND) else {
        return;
    };
    let state = app.state::<AppState>();
    let (w, h) = *state.island_size.lock().unwrap();
    let s = state.settings.lock().unwrap().clone();

    let (rects, primary_idx) = monitor_rects(app);
    let Some((_, monitor)) = pick_monitor(&rects, primary_idx, s.island.monitor_index) else {
        return;
    };
    let offset = (*state.island_drag.lock().unwrap()).unwrap_or(s.island.offset_x);
    let hidden = *state.island_hidden.lock().unwrap() && s.island.visibility == Visibility::Auto;
    let r = if hidden {
        strip_rect(&monitor, w, offset)
    } else {
        island_rect(&monitor, w, h, offset, s.island.margin_top)
    };
    let _ = win.set_size(PhysicalSize::new(r.width, r.height));
    let _ = win.set_position(PhysicalPosition::new(r.x, r.y));
}

/// Bring a hidden island back (shortcut, tray "Show VEX"). The page is told even when it is already
/// visible, so it restarts its hide timer instead of vanishing a moment later.
pub(crate) fn reveal_island(app: &AppHandle) {
    let was_hidden = std::mem::replace(&mut *app.state::<AppState>().island_hidden.lock().unwrap(), false);
    if was_hidden {
        apply_island_geometry(app);
    }
    let _ = app.emit("island-reveal", ());
}

fn toggle_island(app: &AppHandle) {
    reveal_island(app);
    let _ = app.emit("island-toggle", ());
}

/// The page asks to tuck the island away (after its exit animation) or to restore it (pointer on the top-edge
/// strip, or a screenshot confirmation). Hiding is only honoured in Auto-Hide mode. Returns whether the island is hidden now.
#[tauri::command]
fn set_island_hidden(app: AppHandle, state: State<AppState>, hidden: bool) -> bool {
    let allowed = state.settings.lock().unwrap().island.visibility == Visibility::Auto;
    let now = hidden && allowed;
    let changed = std::mem::replace(&mut *state.island_hidden.lock().unwrap(), now) != now;
    if changed {
        apply_island_geometry(&app);
    }
    now
}

/// Replace the global shortcuts (island toggle, launcher and screenshot). Returns a warning for each one that
/// could not be registered, so a bad or taken accelerator never takes the app down.
fn register_hotkeys(app: &AppHandle, s: &Settings) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut warnings = Vec::new();
    let island = s.island.hotkey.as_str();
    if let Err(e) = gs.on_shortcut(island, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_island(app);
        }
    }) {
        warnings.push(format!("Could not register hotkey \"{island}\": {e}"));
    }
    let launch = s.launcher.hotkey.as_str();
    if s.launcher.enabled && !launch.is_empty() {
        if launch.eq_ignore_ascii_case(island) || launch.eq_ignore_ascii_case(s.screenshot.hotkey.as_str()) {
            warnings.push("The launcher shortcut is already used by another VEX shortcut; choose a different one.".into());
        } else if let Err(e) = gs.on_shortcut(launch, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                launcher::toggle(app);
            }
        }) {
            warnings.push(format!("Could not register the launcher shortcut \"{launch}\": {e}. Another app may be using it; pick a different shortcut in Settings > Launcher."));
        }
    }
    let shot = s.screenshot.hotkey.as_str();
    if !shot.is_empty() {
        if shot.eq_ignore_ascii_case(island) {
            warnings.push("The screenshot shortcut is the same as the island shortcut; choose a different one.".into());
        } else {
            let mode = s.screenshot.hotkey_mode;
            if let Err(e) = gs.on_shortcut(shot, move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    let app = app.clone();
                    // Capturing waits for the compositor, so it must not run on the shortcut thread.
                    std::thread::spawn(move || {
                        let _ = screenshot::capture(&app, mode);
                    });
                }
            }) {
                warnings.push(format!("Could not register the screenshot shortcut \"{shot}\": {e}"));
            }
        }
    }
    warnings
}

// ---------------------------------------------------------------- commands

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<AppState>,
    patch: Value,
) -> Result<UpdateResult, String> {
    let (next, old_shortcuts, old_notifications, old_clipboard, old_history, old_launcher) = {
        let current = state.settings.lock().unwrap();
        let next = current.with_patch(&patch).map_err(|e| e.to_string())?;
        (next, (current.island.hotkey.clone(), current.screenshot.hotkey.clone(), current.screenshot.hotkey_mode, current.launcher.hotkey.clone(), current.launcher.enabled), current.notifications.clone(), current.clipboard.clone(), current.screenshot.history, current.launcher.clone())
    };

    let mut warnings = Vec::new();
    if let Err(e) = state.store.save(&next) {
        warnings.push(format!("Settings could not be saved to disk: {e}"));
    }
    *state.settings.lock().unwrap() = next.clone();

    if next.notifications != old_notifications {
        app.state::<notifications::NotificationHandle>().configure(next.notifications.clone());
    }
    if next.clipboard != old_clipboard {
        app.state::<clipboard::ClipboardHandle>().configure(next.clipboard.clone());
    }
    if next.screenshot.history != old_history {
        app.state::<screenshot::Shots>().set_history_mode(next.screenshot.history);
    }
    if next.launcher != old_launcher {
        let handle = app.state::<launcher::LauncherHandle>();
        if next.launcher.apps && !old_launcher.apps {
            handle.refresh_apps(&app);
        }
        if next.launcher.files && (!old_launcher.files || next.launcher.file_folders != old_launcher.file_folders) {
            handle.refresh_files(&app, &next.launcher);
        }
    }
    if (next.island.hotkey.clone(), next.screenshot.hotkey.clone(), next.screenshot.hotkey_mode, next.launcher.hotkey.clone(), next.launcher.enabled) != old_shortcuts {
        warnings.extend(register_hotkeys(&app, &next));
    }
    if next.island.visibility != Visibility::Auto && std::mem::replace(&mut *state.island_hidden.lock().unwrap(), false) {
        // Switching to Always Visible while hidden brings the island back at once.
        let _ = app.emit("island-reveal", ());
    }
    apply_island_geometry(&app);
    let _ = app.emit("settings-changed", &next);
    Ok(UpdateResult {
        settings: next,
        warnings,
    })
}

#[tauri::command]
fn list_monitors(app: AppHandle) -> Vec<MonitorInfo> {
    let monitors = app.available_monitors().unwrap_or_default();
    let (_, primary_idx) = monitor_rects(&app);
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            index: i,
            name: m
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Display {}", i + 1)),
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
            primary: i == primary_idx,
        })
        .collect()
}

#[tauri::command]
fn set_island_size(app: AppHandle, state: State<AppState>, width: f64, height: f64) {
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        *state.island_size.lock().unwrap() = (width.min(1600.0), height.min(900.0));
        apply_island_geometry(&app);
    }
}

/// Move the island along the top edge while it is dragged (`commit = false`: move only) or when the drag
/// ends (`commit = true`: move and save). Returns the offset that was actually applied, after keeping the
/// window inside the chosen monitor.
#[tauri::command]
fn set_island_offset(app: AppHandle, state: State<AppState>, offset_x: i32, commit: bool) -> i32 {
    let (w, _) = *state.island_size.lock().unwrap();
    let saved = state.settings.lock().unwrap().clone();
    let (rects, primary_idx) = monitor_rects(&app);
    let Some((_, monitor)) = pick_monitor(&rects, primary_idx, saved.island.monitor_index) else {
        return saved.island.offset_x;
    };
    let offset = clamp_offset(&monitor, w, offset_x.clamp(-4000, 4000));
    if commit {
        *state.island_drag.lock().unwrap() = None;
        let _ = update_settings(app.clone(), app.state::<AppState>(), json!({ "island": { "offsetX": offset } }));
    } else {
        *state.island_drag.lock().unwrap() = Some(offset);
        apply_island_geometry(&app);
    }
    offset
}

/// Put the island back in the centre (Settings and the tray both use this).
pub(crate) fn reset_island_position(app: &AppHandle) {
    *app.state::<AppState>().island_drag.lock().unwrap() = None;
    let _ = update_settings(app.clone(), app.state::<AppState>(), json!({ "island": { "offsetX": 0 } }));
}

// Must be `async`: building a window inside a synchronous command deadlocks on Windows.
#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_window(&app).map_err(|e| e.to_string())
}

pub(crate) fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("VEX Settings")
        .inner_size(860.0, 620.0)
        .min_inner_size(680.0, 480.0)
        .center()
        .build()?;
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// Media (Milestone 2) --------------------------------------------------------

#[tauri::command]
fn media_get_state(media: State<media::MediaHandle>) -> MediaState {
    media.state()
}

// `async` so waiting for the player never blocks the UI thread.
#[tauri::command]
async fn media_transport(
    media: State<'_, media::MediaHandle>,
    command: TransportCommand,
) -> Result<(), String> {
    media.send(command)
}

// Volume (desktop controls, stage 3) -------------------------------------------

// `async` so a slow audio driver never blocks the UI thread.
#[tauri::command]
async fn volume_get(app: AppHandle, volume: State<'_, volume::VolumeHandle>) -> Result<vex_core::volume::VolumeState, String> {
    volume.run(&app, volume::Cmd::Get)
}

#[tauri::command]
async fn volume_set(app: AppHandle, volume: State<'_, volume::VolumeHandle>, percent: i64) -> Result<vex_core::volume::VolumeState, String> {
    volume.run(&app, volume::Cmd::Set(percent))
}

/// `direction` is only its sign; the size of one step comes from Settings (island.volumeStep).
#[tauri::command]
async fn volume_step(
    app: AppHandle,
    state: State<'_, AppState>,
    volume: State<'_, volume::VolumeHandle>,
    direction: i64,
) -> Result<vex_core::volume::VolumeState, String> {
    let step = state.settings.lock().unwrap().island.volume_step as i64;
    volume.run(&app, volume::Cmd::Step(direction.signum() * step))
}

#[tauri::command]
async fn volume_toggle_mute(app: AppHandle, volume: State<'_, volume::VolumeHandle>) -> Result<vex_core::volume::VolumeState, String> {
    volume.run(&app, volume::Cmd::ToggleMute)
}

// Clipboard -------------------------------------------------------------------

#[tauri::command]
fn clipboard_status(handle: State<clipboard::ClipboardHandle>) -> vex_core::clipboard::Status {
    handle.status()
}

/// The history, pinned first. `query` filters text entries (case-insensitive).
#[tauri::command]
fn clipboard_list(handle: State<clipboard::ClipboardHandle>, query: Option<String>) -> Vec<vex_core::clipboard::ItemView> {
    handle.list(query.as_deref().unwrap_or(""))
}

// `async` so waiting for the clipboard never blocks the UI thread.
#[tauri::command]
async fn clipboard_copy(handle: State<'_, clipboard::ClipboardHandle>, id: u64) -> Result<(), String> {
    handle.copy(id)
}

#[tauri::command]
async fn clipboard_pin(handle: State<'_, clipboard::ClipboardHandle>, id: u64, pinned: bool) -> Result<(), String> {
    handle.pin(id, pinned)
}

#[tauri::command]
fn clipboard_delete(handle: State<clipboard::ClipboardHandle>, id: u64) {
    handle.delete(id);
}

/// Remove entries from the history; pinned ones stay unless `keep_pinned` is false.
#[tauri::command]
fn clipboard_clear(handle: State<clipboard::ClipboardHandle>, keep_pinned: bool) {
    handle.clear(keep_pinned);
}

// Notifications ---------------------------------------------------------------

#[tauri::command]
fn notifications_get(handle: State<notifications::NotificationHandle>) -> Option<vex_core::notifications::View> {
    handle.view()
}

#[tauri::command]
fn notification_dismiss(handle: State<notifications::NotificationHandle>, id: u64) {
    handle.send(vex_core::notifications::Msg::Dismiss(id));
}

#[tauri::command]
fn notification_dismiss_all(handle: State<notifications::NotificationHandle>) {
    handle.send(vex_core::notifications::Msg::DismissAll);
}

#[tauri::command]
fn notification_clear(handle: State<notifications::NotificationHandle>, id: u64) {
    handle.send(vex_core::notifications::Msg::Clear(id));
}

#[tauri::command]
fn notification_clear_all(handle: State<notifications::NotificationHandle>) {
    handle.send(vex_core::notifications::Msg::ClearAll);
}

/// Open the app that posted a notification (Windows offers no way to "click" the original toast
/// from outside, so VEX launches the app by its id) and mark the notification read.
#[tauri::command]
fn notification_open(handle: State<notifications::NotificationHandle>, id: u64) -> Result<(), String> {
    let app_id = handle.app_id_of(id).ok_or("That notification is no longer in the history.")?;
    let target = vex_core::notifications::launch_target(&app_id).ok_or("This app cannot be opened from here.")?;
    launch_app(&target)?;
    handle.send(vex_core::notifications::Msg::Dismiss(id));
    Ok(())
}

#[cfg(windows)]
fn launch_app(target: &str) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the app: {e}"))
}

#[cfg(not(windows))]
fn launch_app(_target: &str) -> Result<(), String> {
    Err("Opening apps is only available on Windows.".into())
}

// Lyrics (Milestone 3) --------------------------------------------------------

/// Automatic LRCLIB lookup. Returns Ok(None) when the track has no lyrics there;
/// Err only for network failures (which are not cached, so the UI may retry).
/// Honours the `music.lyricsOnline` setting so nothing leaves the PC when it is off.
#[tauri::command]
async fn lyrics_fetch(
    app: AppHandle,
    state: State<'_, AppState>,
    track: TrackQuery,
) -> Result<Option<LyricsResult>, String> {
    if !state.settings.lock().map_err(|_| "settings poisoned")?.music.lyrics_online {
        return Ok(None);
    }
    tauri::async_runtime::spawn_blocking(move || app.state::<lyrics::LyricsHandle>().lookup(&track))
        .await
        .map_err(|e| e.to_string())?
}

// Assistant face preview -------------------------------------------------------

/// Lets the settings window drive the island's assistant face so every
/// assistant state can be seen on the real overlay.
#[tauri::command]
fn assistant_preview_state(app: AppHandle, state: String) -> Result<(), String> {
    match state.as_str() {
        "idle" | "listening" | "thinking" | "speaking" | "error" => app
            .emit("assistant-state", state)
            .map_err(|e| e.to_string()),
        other => Err(format!("unknown assistant state: {other}")),
    }
}

// ---------------------------------------------------------------- entry point

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let store = SettingsStore::new(dir.join("settings.json"));
            let loaded = store.load();
            if loaded.recovered_from_corrupt {
                eprintln!("vex: settings.json was unreadable; moved aside and defaults restored");
            }
            let startup_settings = loaded.settings.clone();
            let shot_history = loaded.settings.screenshot.history;
            let notif_cfg = loaded.settings.notifications.clone();
            let clip_cfg = loaded.settings.clipboard.clone();
            app.manage(AppState {
                settings: Mutex::new(loaded.settings),
                store,
                island_size: Mutex::new((260.0, 60.0)),
                island_drag: Mutex::new(None),
                island_hidden: Mutex::new(false),
            });

            app.manage(volume::VolumeHandle::default());
            app.manage(media::start(app.handle().clone()));
            app.manage(lyrics::LyricsHandle::new());
            app.manage(notifications::start(app.handle().clone(), notif_cfg));
            app.manage(screenshot::Shots::new(shot_history, dir.join("screenshots")));
            app.manage(clipboard::start(app.handle().clone(), clip_cfg, dir.join("clipboard")));

            let chats = vex_core::chat::ChatStore::load(dir.join("chats.json"));
            if chats.recovered_from_corrupt {
                eprintln!("vex: chats.json was unreadable; it was moved aside");
            }
            app.manage(std::sync::Arc::new(ai::AiService::production(chats)));

            app.manage(hub::HubHandle::start(app.handle(), dir.clone()));

            let launcher_cfg = startup_settings.launcher.clone();
            app.manage(launcher::LauncherHandle::start(app.handle(), dir.clone(), &launcher_cfg));
            if let Err(e) = launcher::build_window(app.handle()) {
                eprintln!("vex: could not create the launcher window: {e}");
            }

            for w in register_hotkeys(app.handle(), &startup_settings) {
                eprintln!("vex: {w}");
            }

            // Tray menu: the island has no taskbar button, so this is the
            // always-available way to reach settings or quit.
            let toggle = MenuItem::with_id(
                app,
                "toggle",
                "Expand / collapse island",
                true,
                None::<&str>,
            )?;
            let widgets = MenuItem::with_id(app, "hub", "Widgets Hub", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show VEX", true, None::<&str>)?;
            let reset_pos = MenuItem::with_id(app, "reset-position", "Reset island position", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit VEX", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &show,
                    &toggle,
                    &widgets,
                    &reset_pos,
                    &settings,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("VEX")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => reveal_island(app),
                    "toggle" => toggle_island(app),
                    "settings" => {
                        // Same Windows deadlock rule as the command: build the window off the event-handler thread.
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let _ = open_settings_window(&app);
                        });
                    }
                    "hub" => {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let _ = hub::open_window(&app, None);
                        });
                    }
                    "reset-position" => reset_island_position(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            apply_island_geometry(app.handle());
            if let Some(w) = app.get_webview_window(ISLAND) {
                let _ = w.show();
                // Re-place the island when the resolution, DPI, monitors or work area change (and after sleep).
                #[cfg(windows)]
                if let Ok(hwnd) = w.hwnd() {
                    let handle = app.handle().clone();
                    if !display_windows::watch(hwnd.0, move || apply_island_geometry(&handle)) {
                        eprintln!("vex: could not watch for display changes; the island is re-placed only when its size or settings change");
                    }
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // The Hub's widgets are gone with its window, so anything they asked the island to show goes too.
            if window.label() == ISLAND {
                if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                    apply_island_geometry(window.app_handle());
                }
            }
            if window.label() == hub::HUB {
                if let tauri::WindowEvent::Destroyed = event {
                    window.state::<hub::HubHandle>().clear_statuses(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            list_monitors,
            set_island_size,
            set_island_offset,
            set_island_hidden,
            open_settings,
            quit_app,
            media_get_state,
            volume_get,
            volume_set,
            volume_step,
            volume_toggle_mute,
            media_transport,
            lyrics_fetch,
            notifications_get,
            notification_dismiss,
            notification_dismiss_all,
            notification_clear,
            notification_clear_all,
            notification_open,
            clipboard_status,
            clipboard_list,
            clipboard_copy,
            clipboard_pin,
            clipboard_delete,
            clipboard_clear,
            screenshot::screenshot_capture,
            screenshot::screenshot_region_select,
            screenshot::screenshot_region_cancel,
            screenshot::screenshot_list,
            screenshot::screenshot_image,
            screenshot::screenshot_delete,
            screenshot::screenshot_clear,
            screenshot::screenshot_copy,
            screenshot::screenshot_save,
            screenshot::screenshot_pick_folder,
            screenshot::screenshot_reveal,
            screenshot::screenshot_open_editor,
            ai::ai_send,
            ai::ai_cancel,
            ai::ai_metrics,
            ai::ai_metrics_clear,
            ai::bench::ai_preload,
            ai::bench::ai_benchmark,
            ai::ai_key_status,
            ai::ai_set_key,
            ai::ai_clear_key,
            ai::ai_list_models,
            ai::ai_test,
            ai::chat_list,
            ai::chat_get,
            ai::chat_delete,
            ai::chat_clear_all,
            assistant_preview_state,
            hub::hub_open,
            hub::hub_layout,
            hub::hub_save_layout,
            hub::hub_data_get,
            hub::hub_data_set,
            hub::hub_set_status,
            hub::hub_statuses,
            hub::timers_list,
            hub::timer_create,
            hub::timer_start,
            hub::timer_pause,
            hub::timer_reset,
            hub::timer_delete,
            hub::timer_set_duration,
            hub::timer_set_label,
            launcher::launcher_hide,
            launcher::launcher_search,
            launcher::launcher_run,
            launcher::launcher_state,
            launcher::launcher_set_pin,
            launcher::launcher_record_search,
            launcher::launcher_clear_history,
            launcher::launcher_refresh,
            launcher::launcher_app_icon,
            ai::launcher_ai_info,
            ai::launcher_ai_send,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VEX");
}

#[cfg(test)]
mod tests {
    use vex_core::settings::DEFAULT_HOTKEY;
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn default_hotkey_is_a_valid_accelerator() {
        assert!(DEFAULT_HOTKEY.parse::<Shortcut>().is_ok());
    }

    /// The Settings shortcut recorder emits strings like these; every shape it can produce must parse.
    #[test]
    fn recorder_output_shapes_parse() {
        for s in [
            "Ctrl+Alt+K",
            "Ctrl+Shift+5",
            "Alt+Space",
            "Ctrl+Alt+F5",
            "Super+Shift+A",
            "Ctrl+ArrowUp",
            "Ctrl+Alt+Digit1",
        ] {
            let parsed = s.parse::<Shortcut>();
            println!("{s}: {}", if parsed.is_ok() { "ok" } else { "REJECTED" });
        }
        assert!("Ctrl+Alt+K".parse::<Shortcut>().is_ok());
        assert!("Ctrl+Shift+5".parse::<Shortcut>().is_ok());
        assert!("Alt+Space".parse::<Shortcut>().is_ok());
        assert!("Ctrl+Alt+F5".parse::<Shortcut>().is_ok());
        assert!("Super+Shift+A".parse::<Shortcut>().is_ok());
    }

    #[test]
    fn garbage_is_rejected_not_panicking() {
        assert!("".parse::<Shortcut>().is_err());
        assert!("Ctrl+".parse::<Shortcut>().is_err());
        assert!("Ctrl+Alt+NotAKey".parse::<Shortcut>().is_err());
    }
}
