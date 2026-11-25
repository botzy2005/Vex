//! Platform-neutral core of VEX.
//!
//! Everything here is pure logic so it can be unit-tested on any OS:
//! the settings schema (with sanitising and merge-patch updates), atomic
//! persistence, and the monitor/DPI placement math for the island window.

pub mod ai;
pub mod chat;
pub mod clipboard;
pub mod connect;
pub mod hub;
pub mod jsonfile;
pub mod launcher;
pub mod lyrics;
pub mod media;
pub mod notifications;
pub mod perf;
pub mod placement;
pub mod screenshot;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod sysmon;
pub mod volume;

pub use settings::{Settings, SettingsError};
pub use store::{LoadOutcome, SettingsStore};
