//! Windows side of the Quick Launcher: finding installed apps, starting things, and app icons.
//!
//! Nothing here builds a shell command line. Every process is started with `Command::arg`, and the
//! only values that ever reach `explorer.exe` are paths that exist, Store app ids that pass a strict
//! character allowlist, and https URLs built by `vex_core::launcher::commands::web_search_url`.
//!
//! Compiled by Windows CI only; this code has not been run on a real PC yet.

use base64::Engine;
use std::{
    ffi::c_void,
    fs,
    io::Read,
    os::windows::{ffi::OsStrExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use vex_core::{
    clipboard::RawImage,
    launcher::{AppEntry, AppKind},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Graphics::Gdi::{DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ},
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
        System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED},
        UI::{
            Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
            WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO},
        },
    },
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const STORE_PREFIX: &str = "shell:AppsFolder\\";

// ---------------------------------------------------------------- discovery

fn start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["APPDATA", "ProgramData"] {
        if let Some(base) = std::env::var_os(var) {
            roots.push(PathBuf::from(base).join("Microsoft\\Windows\\Start Menu\\Programs"));
        }
    }
    roots
}

fn skip_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("uninstall") || n.starts_with("readme") || n.ends_with(" help") || n.ends_with(" documentation")
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<AppEntry>) {
    if depth > 5 {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk(&path, depth + 1, out);
            continue;
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "lnk" && ext != "url" {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
        let name: String = stem.chars().filter(|c| !c.is_control()).take(100).collect();
        if name.is_empty() || skip_name(&name) {
            continue;
        }
        out.push(AppEntry { id: path.to_string_lossy().into_owned(), name, kind: AppKind::Desktop });
    }
}

/// Run PowerShell with a fixed script (no user input in it), hidden, with a time limit.
fn run_powershell(script: &str, limit: Duration) -> Option<String> {
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(_) => return None,
        }
    }
    let bytes = reader.join().ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(text.trim_start_matches('\u{feff}').to_string())
}

/// App user model ids look like `Microsoft.WindowsCalculator_8wekyb3d8bbwe!App`.
fn valid_aumid(id: &str) -> bool {
    id.contains('!') && id.len() <= 200 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '!'))
}

fn store_apps() -> Vec<AppEntry> {
    let script = "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; Get-StartApps | ForEach-Object { $_.AppID + \"`t\" + $_.Name }";
    let Some(text) = run_powershell(script, Duration::from_secs(20)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let (id, name) = line.split_once('\t')?;
            let (id, name) = (id.trim(), name.trim());
            let name: String = name.chars().filter(|c| !c.is_control()).take(100).collect();
            (!name.is_empty() && valid_aumid(id)).then(|| AppEntry { id: format!("{STORE_PREFIX}{id}"), name, kind: AppKind::Store })
        })
        .collect()
}

pub fn discover_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();
    for root in start_menu_roots() {
        walk(&root, 0, &mut apps);
    }
    apps.extend(store_apps());
    // One entry per name; the first one found (user shortcuts, then common ones, then Store) wins.
    let mut seen = std::collections::HashSet::new();
    apps.retain(|a| seen.insert(a.name.to_lowercase()));
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

// ---------------------------------------------------------------- starting things

fn explorer(arg: impl AsRef<std::ffi::OsStr>) -> Result<(), String> {
    Command::new("explorer.exe").arg(arg).spawn().map(|_| ()).map_err(|e| format!("Windows could not open it: {e}"))
}

pub fn launch_app(app: &AppEntry) -> Result<(), String> {
    match app.kind {
        AppKind::Desktop => {
            let p = Path::new(&app.id);
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext != "lnk" && ext != "url" {
                return Err("That is not an app shortcut.".into());
            }
            if !p.is_file() {
                return Err(format!("{} is no longer installed.", app.name));
            }
            explorer(p)
        }
        AppKind::Store => {
            let aumid = app.id.strip_prefix(STORE_PREFIX).ok_or("That is not a Store app id.")?;
            if !valid_aumid(aumid) {
                return Err("That Store app id is not valid.".into());
            }
            explorer(&app.id)
        }
    }
}

pub fn open_path(p: &Path) -> Result<(), String> {
    explorer(p)
}

pub fn reveal(p: &Path) -> Result<(), String> {
    Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only https links are opened.".into());
    }
    explorer(url)
}

pub fn open_windows_settings() -> Result<(), String> {
    explorer("ms-settings:")
}

// ---------------------------------------------------------------- icons

/// The shell's large icon for a shortcut, as a small PNG data URL. Store apps have no icon here
/// (their tile art lives in the package), so they get the page's lettered tile.
pub fn app_icon(app: &AppEntry) -> Option<String> {
    if app.kind != AppKind::Desktop {
        return None;
    }
    let img = icon_rgba(Path::new(&app.id))?;
    let png = vex_core::clipboard::image::encode_png(&img).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

fn read_bits(hdc: windows::Win32::Graphics::Gdi::HDC, bmp: windows::Win32::Graphics::Gdi::HBITMAP, w: i32, h: i32) -> Option<Vec<u8>> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    // SAFETY: `buf` holds w*h*4 bytes, which is what a 32-bit top-down DIB of this size needs.
    let lines = unsafe { GetDIBits(hdc, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut c_void), &mut info, DIB_RGB_COLORS) };
    (lines != 0).then_some(buf)
}

fn icon_rgba(path: &Path) -> Option<RawImage> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: plain Win32 calls with valid, NUL-terminated and correctly sized arguments; every GDI
    // object and the icon handle created here is released before returning.
    unsafe {
        // The shell wants COM on the calling thread; "already initialised" results are fine.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut info = SHFILEINFOW::default();
        let got = SHGetFileInfoW(PCWSTR(wide.as_ptr()), FILE_FLAGS_AND_ATTRIBUTES(0), Some(&mut info), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
        if got == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let hicon = info.hIcon;
        let mut ii = ICONINFO::default();
        if GetIconInfo(hicon, &mut ii).is_err() {
            let _ = DestroyIcon(hicon);
            return None;
        }
        let result = (|| {
            let mut bm = BITMAP::default();
            let n = GetObjectW(HGDIOBJ(ii.hbmColor.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut BITMAP as *mut c_void));
            if n == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 || bm.bmWidth > 256 || bm.bmHeight > 256 {
                return None;
            }
            let (w, h) = (bm.bmWidth, bm.bmHeight);
            let screen = GetDC(None);
            if screen.is_invalid() {
                return None;
            }
            let color = read_bits(screen, ii.hbmColor, w, h);
            let mask = read_bits(screen, ii.hbmMask, w, h);
            ReleaseDC(None, screen);
            let mut px = color?;
            let has_alpha = px.chunks_exact(4).any(|p| p[3] != 0);
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                p.swap(0, 2); // BGRA -> RGBA
                if !has_alpha {
                    // Old-style icon: transparency lives in the mask (black = opaque).
                    let opaque = mask.as_ref().is_none_or(|m| m[i * 4] == 0);
                    p[3] = if opaque { 255 } else { 0 };
                }
            }
            Some(RawImage { width: w as u32, height: h as u32, rgba: px })
        })();
        let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
        let _ = DestroyIcon(hicon);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_ids_are_checked_strictly() {
        assert!(valid_aumid("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"));
        assert!(!valid_aumid("NoBang"));
        assert!(!valid_aumid("x!y & calc.exe"));
        assert!(!valid_aumid("x!y\"z"));
        assert!(!valid_aumid(&format!("a!{}", "b".repeat(300))));
    }

    #[test]
    fn uninstallers_and_readmes_are_skipped() {
        assert!(skip_name("Uninstall Foo"));
        assert!(skip_name("Readme"));
        assert!(!skip_name("Visual Studio Code"));
    }

    #[test]
    fn only_https_urls_open() {
        assert!(open_url("http://example.com").is_err());
        assert!(open_url("file:///c:/windows/system32/cmd.exe").is_err());
    }
}
