//! Windows screen capture (GDI) and the few window queries screenshots need.
//!
//! This is the only Windows-specific screenshot code. It reads pixels and window rectangles in
//! physical (device) pixels; every decision (which monitor, region maths, cropping, naming, history)
//! lives in `vex_core::screenshot`, where it is unit-tested.
//!
//! The process is per-monitor DPI aware (Tauri's manifest), so every coordinate here is a real device
//! pixel, whatever each display's scaling is. `BitBlt` from the screen DC with `CAPTUREBLT` also
//! captures layered windows.

use std::ffi::c_void;
use vex_core::clipboard::RawImage;
use vex_core::screenshot::naming::Stamp;
use vex_core::screenshot::{Rect, MAX_CAPTURE_PIXELS};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HGDIOBJ, SRCCOPY,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetWindowDisplayAffinity, GWL_EXSTYLE, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WS_EX_TOOLWINDOW,
};

/// Copy a rectangle of the desktop (physical pixels, may span monitors) into an RGBA image.
pub fn capture_rect(r: &Rect) -> Result<RawImage, String> {
    if r.is_empty() || r.pixels() > MAX_CAPTURE_PIXELS as u64 {
        return Err("That area is too large to capture.".into());
    }
    let (w, h) = (r.w as i32, r.h as i32);
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err("Windows would not let VEX read the screen.".into());
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let result = if mem.is_invalid() || bmp.is_invalid() {
            Err("Could not allocate memory for the screenshot.".to_string())
        } else {
            let old = SelectObject(mem, HGDIOBJ(bmp.0));
            let copied = BitBlt(mem, 0, 0, w, h, Some(screen), r.x, r.y, SRCCOPY | CAPTUREBLT);
            let out = match copied {
                Err(e) => Err(format!("Could not read the screen: {e}")),
                Ok(()) => {
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
                    let mut buf = vec![0u8; r.w as usize * r.h as usize * 4];
                    let lines = GetDIBits(mem, bmp, 0, r.h, Some(buf.as_mut_ptr() as *mut c_void), &mut info, DIB_RGB_COLORS);
                    if lines == 0 {
                        Err("Could not read the screen pixels.".to_string())
                    } else {
                        // GDI gives BGRA with an unused alpha byte.
                        for px in buf.chunks_exact_mut(4) {
                            px.swap(0, 2);
                            px[3] = 255;
                        }
                        RawImage::with_limit(r.w, r.h, buf, MAX_CAPTURE_PIXELS).ok_or_else(|| "The screenshot was invalid.".to_string())
                    }
                }
            };
            SelectObject(mem, old);
            out
        };
        if !bmp.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(bmp.0));
        }
        if !mem.is_invalid() {
            let _ = DeleteDC(mem);
        }
        ReleaseDC(None, screen);
        result
    }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
}

/// The local date and time for file names.
pub fn local_stamp() -> Stamp {
    let t = unsafe { GetLocalTime() };
    Stamp { year: i32::from(t.wYear), month: u32::from(t.wMonth), day: u32::from(t.wDay), hour: u32::from(t.wHour), minute: u32::from(t.wMinute), second: u32::from(t.wSecond) }
}

/// Keep a window out of screen captures (or put it back). Returns false when Windows refuses
/// (before Windows 10 version 2004), in which case the caller hides the window instead.
pub fn set_excluded(hwnd: *mut c_void, excluded: bool) -> bool {
    unsafe { SetWindowDisplayAffinity(HWND(hwnd), if excluded { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE }) }.is_ok()
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// The visible bounds of a window VEX may capture, or `None` if it is VEX's own, minimised, hidden,
/// cloaked (on another virtual desktop), a tool window, or part of the desktop shell.
fn candidate(hwnd: HWND, require_title: bool) -> Option<Rect> {
    unsafe {
        if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() {
            return None;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4).is_ok() && cloaked != 0 {
            return None;
        }
        if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
        if matches!(class_name(hwnd).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return None;
        }
        if require_title && GetWindowTextLengthW(hwnd) == 0 {
            return None;
        }
        // The frame without the invisible resize border and shadow.
        let mut r = RECT::default();
        if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut c_void, std::mem::size_of::<RECT>() as u32).is_err() {
            GetWindowRect(hwnd, &mut r).ok()?;
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        (w > 0 && h > 0).then(|| Rect::new(r.left, r.top, w as u32, h as u32))
    }
}

unsafe extern "system" fn first_window(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let slot = &mut *(lparam.0 as *mut Option<Rect>);
    match candidate(hwnd, true) {
        Some(r) => {
            *slot = Some(r);
            windows::core::BOOL(0) // stop: windows are enumerated top of the z-order first
        }
        None => windows::core::BOOL(1),
    }
}

/// The window the user is working in: the foreground window, or, when that is VEX itself (or the
/// desktop), the topmost ordinary window.
pub fn active_window_rect() -> Result<Rect, String> {
    if let Some(r) = candidate(unsafe { GetForegroundWindow() }, false) {
        return Ok(r);
    }
    let mut found: Option<Rect> = None;
    unsafe {
        let _ = EnumWindows(Some(first_window), LPARAM(&mut found as *mut Option<Rect> as isize));
    }
    found.ok_or_else(|| "There is no window to capture. Click the window you want first.".to_string())
}
