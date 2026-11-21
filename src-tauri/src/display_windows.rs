//! Tells the shell when Windows changes the display layout, so the island can be re-placed.
//!
//! Tauri has no event for "resolution changed" or "monitor plugged in", so this subclasses the island's
//! window and listens for the Windows messages that mean the monitor geometry may have changed: display
//! change, DPI change, work-area change and resume from sleep. It only observes and forwards every message
//! unchanged. Bursts are collapsed: the callback runs once, about 300 ms after the last message.
//! Compiled by Windows CI only; not run on a real PC yet.

use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::Duration,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::Shell::{DefSubclassProc, SetWindowSubclass},
};

const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_POWERBROADCAST: u32 = 0x0218;
const WM_DPICHANGED: u32 = 0x02E0;
const SPI_SETWORKAREA: usize = 0x002F;
const PBT_APMRESUMESUSPEND: usize = 0x0007;
const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
const SUBCLASS_ID: usize = 0x5645_5831; // "VEX1"
const DEBOUNCE: Duration = Duration::from_millis(300);

static GENERATION: AtomicU64 = AtomicU64::new(0);
static CALLBACK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

fn is_relevant(msg: u32, wparam: usize) -> bool {
    match msg {
        WM_DISPLAYCHANGE | WM_DPICHANGED => true,
        WM_SETTINGCHANGE => wparam == SPI_SETWORKAREA,
        WM_POWERBROADCAST => wparam == PBT_APMRESUMEAUTOMATIC || wparam == PBT_APMRESUMESUSPEND,
        _ => false,
    }
}

fn schedule() {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(DEBOUNCE);
        if GENERATION.load(Ordering::SeqCst) == generation {
            if let Some(cb) = CALLBACK.get() {
                cb();
            }
        }
    });
}

unsafe extern "system" fn subclass_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if is_relevant(msg, wparam.0) {
        schedule();
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

/// Start watching. Returns false if Windows refused the subclass (the island then re-places itself only
/// when its settings or size change, as before).
pub fn watch(hwnd: *mut c_void, on_change: impl Fn() + Send + Sync + 'static) -> bool {
    let _ = CALLBACK.set(Box::new(on_change));
    unsafe { SetWindowSubclass(HWND(hwnd), Some(subclass_proc), SUBCLASS_ID, 0).as_bool() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_display_related_messages_trigger_a_re_placement() {
        assert!(is_relevant(WM_DISPLAYCHANGE, 0));
        assert!(is_relevant(WM_DPICHANGED, 0));
        assert!(is_relevant(WM_SETTINGCHANGE, SPI_SETWORKAREA));
        assert!(!is_relevant(WM_SETTINGCHANGE, 0x0001));
        assert!(is_relevant(WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC));
        assert!(is_relevant(WM_POWERBROADCAST, PBT_APMRESUMESUSPEND));
        assert!(!is_relevant(WM_POWERBROADCAST, 0x0004));
        assert!(!is_relevant(0x0200, 0)); // WM_MOUSEMOVE
    }
}
