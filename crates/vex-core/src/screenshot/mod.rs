//! Screenshots: geometry (monitors, DPI, drag regions, cropping), file naming and save paths, JPEG
//! encoding, and the capture history. All platform-neutral and unit-tested; the only Windows code is
//! the screen capture in the shell crate.
//!
//! Privacy rules enforced here: nothing is captured except on an explicit request; captures stay in
//! memory unless the history preference is `Keep` (then they are written to VEX's data folder, switching
//! it off deletes them); nothing is sent anywhere.

pub mod encode;
pub mod geometry;
pub mod history;
pub mod naming;

pub use encode::{encode_as, encode_jpeg, JPEG_QUALITY};
pub use geometry::{crop, from_drag, monitor_at, plan_window_capture, Rect, MAX_CAPTURE_PIXELS, MIN_REGION};
pub use history::{History, ShotView};
pub use naming::{default_dir, file_name, format_for_path, unique_path, with_format_extension, Stamp};
