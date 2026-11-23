//! System Monitor: one command the page calls on a timer, only while the widget is visible.
//! Nothing runs in the background. The previous reading is kept so rates can be computed from the difference.

use std::sync::Mutex;
use vex_core::sysmon::{compute, Raw, Snapshot};

#[derive(Default)]
pub struct SysmonHandle {
    prev: Mutex<Option<Raw>>,
}

impl SysmonHandle {
    /// Forget the previous reading (the page calls this when the widget is shown again after a pause,
    /// so a long gap does not produce one misleading average).
    pub fn reset(&self) {
        *self.prev.lock().unwrap() = None;
    }

    pub fn sample(&self) -> Snapshot {
        let cur = match read() {
            Ok(r) => r,
            Err(e) => return Snapshot::unavailable(&e),
        };
        let mut prev = self.prev.lock().unwrap();
        let snap = compute(prev.as_ref(), &cur);
        *prev = Some(cur);
        snap
    }
}

#[cfg(windows)]
fn read() -> Result<Raw, String> {
    crate::sysmon_windows::read()
}

#[cfg(not(windows))]
fn read() -> Result<Raw, String> {
    Err("System Monitor is available on Windows only".to_string())
}
