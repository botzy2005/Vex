//! Local clipboard history: image conversion, sensitive-content filtering, the history store,
//! optional on-disk persistence and the engine loop. All platform-neutral and unit-tested; the only
//! Windows code is the `ClipboardSource` implementation in the shell crate.
//!
//! Privacy rules enforced here (not in the UI):
//! * Nothing is recorded unless the module is enabled and not paused; what was copied before enabling,
//!   resuming or during a pause is never read (a sequence-number baseline is taken at that moment).
//! * Entries the source marks as sensitive (password managers) and text that looks like a secret are
//!   dropped before they reach the store.
//! * The history is memory-only unless `persist` is on; turning persistence off deletes the files and
//!   so does starting up with it off.
//! * Nothing here is reachable from the AI code.

pub mod engine;
pub mod image;
pub mod sensitive;
pub mod storage;
pub mod store;

pub use engine::{run_loop, ClipboardSource, Captured, Msg, POLL_INTERVAL};
pub use image::{RawImage, MAX_PIXELS};
pub use storage::{FileStorage, Storage};
pub use store::{AddOutcome, Content, ImageData, Item, ItemView, Status, Store};
