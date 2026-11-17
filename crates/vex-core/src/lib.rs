//! Platform-neutral core of VEX.
//!
//! Everything here is pure logic so it can be unit-tested on any OS:
//! the settings schema (with sanitising and merge-patch updates), atomic
//! persistence, and the monitor/DPI placement math for the island window.

pub mod ai;
pub mod chat;
pub mod lyrics;
pub mod media;
pub mod perf;
pub mod placement;
pub mod secrets;
pub mod settings;
pub mod store;

pub use settings::{Settings, SettingsError};
pub use store::{LoadOutcome, SettingsStore};
