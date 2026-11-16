//! Tauri shell for Islet (Milestone 1).
//!
//! Responsibilities in this milestone: create and position the island window,
//! own the persisted settings, register the global hotkey, provide the tray
//! menu, and expose the command surface the UI talks to. Media sessions are live
//! (Milestone 2); lyrics providers and AI still report "not available yet" so the UI
//! can handle their absence honestly; they arrive in later milestones.

mod lyrics;
mod media;
#[cfg(windows)]
mod media_windows;

use islet_core::{
    lyrics::{LyricsResult, TrackQuery},
    media::{MediaState, TransportCommand},
    placement::{island_rect, pick_monitor, MonitorRect},
    Settings, SettingsStore,
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const ISLAND: &str = "island";
const SETTINGS: &str = "settings";

struct AppState {
    settings: Mutex<Settings>,
    store: SettingsStore,
    /// Logical size the UI last asked the island window to have.
    island_size: Mutex<(f64, f64)>,
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
    let r = island_rect(&monitor, w, h, s.island.offset_x, s.island.margin_top);
    let _ = win.set_size(PhysicalSize::new(r.width, r.height));
    let _ = win.set_position(PhysicalPosition::new(r.x, r.y));
}

fn toggle_island(app: &AppHandle) {
    let _ = app.emit("island-toggle", ());
}

/// Replace the global shortcut. Returns a warning string on failure so a bad
/// accelerator never takes the app down.
fn register_hotkey(app: &AppHandle, accelerator: &str) -> Option<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let result = gs.on_shortcut(accelerator, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_island(app);
        }
    });
    result
        .err()
        .map(|e| format!("Could not register hotkey \"{accelerator}\": {e}"))
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
    let (next, old_hotkey) = {
        let current = state.settings.lock().unwrap();
        let next = current.with_patch(&patch).map_err(|e| e.to_string())?;
        (next, current.island.hotkey.clone())
    };

    let mut warnings = Vec::new();
    if let Err(e) = state.store.save(&next) {
        warnings.push(format!("Settings could not be saved to disk: {e}"));
    }
    *state.settings.lock().unwrap() = next.clone();

    if next.island.hotkey != old_hotkey {
        if let Some(w) = register_hotkey(&app, &next.island.hotkey) {
            warnings.push(w);
        }
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

// Must be `async`: building a window inside a synchronous command deadlocks on Windows.
#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_window(&app).map_err(|e| e.to_string())
}

fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("Islet Settings")
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

// Milestone 5/6 (assistant) ---------------------------------------------------

#[tauri::command]
fn assistant_send(_text: String) -> Result<(), String> {
    Err("No AI provider is connected yet (Milestone 5).".into())
}

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
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let store = SettingsStore::new(dir.join("settings.json"));
            let loaded = store.load();
            if loaded.recovered_from_corrupt {
                eprintln!("islet: settings.json was unreadable; moved aside and defaults restored");
            }
            let hotkey = loaded.settings.island.hotkey.clone();
            app.manage(AppState {
                settings: Mutex::new(loaded.settings),
                store,
                island_size: Mutex::new((260.0, 60.0)),
            });

            app.manage(media::start(app.handle().clone()));
            app.manage(lyrics::LyricsHandle::new());

            if let Some(w) = register_hotkey(app.handle(), &hotkey) {
                eprintln!("islet: {w}");
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
            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Islet", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &toggle,
                    &settings,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Islet")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => toggle_island(app),
                    "settings" => {
                        // Same Windows deadlock rule as the command: build the window off the event-handler thread.
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let _ = open_settings_window(&app);
                        });
                    }
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
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            list_monitors,
            set_island_size,
            open_settings,
            quit_app,
            media_get_state,
            media_transport,
            lyrics_fetch,
            assistant_send,
            assistant_preview_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Islet");
}

#[cfg(test)]
mod tests {
    use islet_core::settings::DEFAULT_HOTKEY;
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
