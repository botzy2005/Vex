//! Windows clipboard access through the Win32 clipboard API.
//!
//! This is the only Windows-specific clipboard code. It reads and writes the OS clipboard; every
//! decision (what to keep, duplicates, sensitive content, limits, persistence) lives in
//! `vex_core::clipboard`, where it is unit-tested.
//!
//! * Changes are detected with `GetClipboardSequenceNumber`, which needs no clipboard access, so
//!   idle polling does not disturb other programs. The clipboard is only opened when it changed.
//! * Password managers and Windows' own clipboard-history opt-out are honoured through the
//!   `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory` formats.
//! * Images: the registered `PNG` format when offered (it keeps transparency), else `CF_DIB`.

use std::time::Duration;
use vex_core::clipboard::image::{decode_dib, decode_png, encode_dib};
use vex_core::clipboard::{Captured, ClipboardSource, RawImage};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;
/// Larger payloads are ignored rather than copied into memory.
const MAX_BYTES: usize = 100 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

fn register(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

/// Holds the clipboard open; closes it when dropped.
struct Open;

impl Open {
    fn acquire() -> Option<Open> {
        for _ in 0..6 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Some(Open);
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        None
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn available(format: u32) -> bool {
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Copy a clipboard format's bytes (clipboard must be open). `None` when absent, empty or too big.
fn read_bytes(format: u32, limit: usize) -> Option<Vec<u8>> {
    unsafe {
        let handle = GetClipboardData(format).ok()?;
        let global = HGLOBAL(handle.0);
        let size = GlobalSize(global);
        if size == 0 || size > limit {
            return None;
        }
        let ptr = GlobalLock(global);
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
        let _ = GlobalUnlock(global);
        Some(bytes)
    }
}

/// Hand `data` to the clipboard under `format` (clipboard must be open and emptied by us).
fn set_bytes(format: u32, data: &[u8]) -> Result<(), String> {
    unsafe {
        let global = GlobalAlloc(GMEM_MOVEABLE, data.len()).map_err(|e| e.to_string())?;
        let ptr = GlobalLock(global);
        if ptr.is_null() {
            return Err("Could not allocate clipboard memory.".into());
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        let _ = GlobalUnlock(global);
        // On success the system owns the memory.
        SetClipboardData(format, Some(HANDLE(global.0))).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub struct WindowsClipboard {
    png: u32,
    exclude_from_monitoring: u32,
    can_include_in_history: u32,
}

impl WindowsClipboard {
    pub fn new() -> Self {
        Self {
            png: register("PNG"),
            exclude_from_monitoring: register("ExcludeClipboardContentFromMonitorProcessing"),
            can_include_in_history: register("CanIncludeInClipboardHistory"),
        }
    }

    /// The copying program asked not to be recorded.
    fn marked_private(&self) -> bool {
        if available(self.exclude_from_monitoring) {
            return true;
        }
        if available(self.can_include_in_history) {
            // A DWORD of 0 means "do not include in clipboard history".
            if let Some(b) = read_bytes(self.can_include_in_history, 16) {
                if b.len() >= 4 && u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 0 {
                    return true;
                }
            }
        }
        false
    }
}

impl ClipboardSource for WindowsClipboard {
    fn sequence(&mut self) -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    fn read(&mut self, want_images: bool) -> Captured {
        let Some(_open) = Open::acquire() else {
            return Captured::Busy;
        };
        if self.marked_private() {
            return Captured::Sensitive;
        }
        if available(CF_UNICODETEXT) {
            return match read_bytes(CF_UNICODETEXT, MAX_TEXT_BYTES) {
                Some(b) => {
                    let units: Vec<u16> = b
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|&u| u != 0)
                        .collect();
                    let text = String::from_utf16_lossy(&units);
                    if text.trim().is_empty() {
                        Captured::Nothing
                    } else {
                        Captured::Text(text)
                    }
                }
                None => Captured::Nothing,
            };
        }
        if want_images {
            if available(self.png) {
                if let Some(img) = read_bytes(self.png, MAX_BYTES).and_then(|b| decode_png(&b).ok()) {
                    return Captured::Image(img);
                }
            }
            if available(CF_DIB) {
                return match read_bytes(CF_DIB, MAX_BYTES).and_then(|b| decode_dib(&b)) {
                    Some(img) => Captured::Image(img),
                    None => Captured::Nothing,
                };
            }
        }
        Captured::Nothing
    }

    fn write_text(&mut self, text: &str) -> Result<(), String> {
        let _open = Open::acquire().ok_or("The clipboard is in use by another program. Try again.")?;
        let mut data: Vec<u8> = Vec::with_capacity(text.len() * 2 + 2);
        for u in text.encode_utf16().chain(std::iter::once(0)) {
            data.extend_from_slice(&u.to_le_bytes());
        }
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        set_bytes(CF_UNICODETEXT, &data)
    }

    fn write_image(&mut self, image: &RawImage, png: &[u8]) -> Result<(), String> {
        let _open = Open::acquire().ok_or("The clipboard is in use by another program. Try again.")?;
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        set_bytes(CF_DIB, &encode_dib(image))?;
        // The PNG keeps transparency for programs that understand it; failing here is not fatal.
        let _ = set_bytes(self.png, png);
        Ok(())
    }
}
