//! The engine loop: watch the clipboard's sequence number, read what changed, filter and store it.

use super::image::{self, RawImage, MAX_PIXELS};
use super::sensitive::looks_sensitive;
use super::storage::Storage;
use super::store::{AddOutcome, Content, ImageData, Status, Store};
use crate::settings::Clipboard;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often the sequence number is checked while recording (one cheap call, no clipboard access).
pub const POLL_INTERVAL: Duration = Duration::from_millis(400);
/// Wake-up interval while nothing is being recorded.
const IDLE_INTERVAL: Duration = Duration::from_secs(5);
/// Another program can hold the clipboard open; retry a few polls before giving up on a copy.
const MAX_BUSY_RETRIES: u32 = 5;
const SAVE_EVERY_MS: u64 = 2_000;

/// What the OS clipboard held when read.
pub enum Captured {
    /// Nothing we support (files, rich formats only, empty).
    Nothing,
    /// Another program had the clipboard locked; try again shortly.
    Busy,
    /// The copying program asked not to be recorded (for example a password manager).
    Sensitive,
    Text(String),
    Image(RawImage),
    Failed(String),
}

pub trait ClipboardSource: Send {
    /// Changes whenever the clipboard content changes.
    fn sequence(&mut self) -> u32;
    fn read(&mut self, want_images: bool) -> Captured;
    fn write_text(&mut self, text: &str) -> Result<(), String>;
    /// `png` is the lossless encoding of `image`, for programs that prefer it (it keeps transparency).
    fn write_image(&mut self, image: &RawImage, png: &[u8]) -> Result<(), String>;
}

impl ClipboardSource for Box<dyn ClipboardSource> {
    fn sequence(&mut self) -> u32 {
        (**self).sequence()
    }
    fn read(&mut self, want_images: bool) -> Captured {
        (**self).read(want_images)
    }
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        (**self).write_text(text)
    }
    fn write_image(&mut self, image: &RawImage, png: &[u8]) -> Result<(), String> {
        (**self).write_image(image, png)
    }
}

pub enum Msg {
    Config(Clipboard),
    /// Put an entry back on the clipboard.
    Copy(u64, Sender<Result<(), String>>),
    Pin(u64, bool, Sender<Result<(), String>>),
    Delete(u64),
    /// Remove entries; `true` keeps the pinned ones.
    Clear(bool),
    Stop,
}

fn recording(cfg: &Clipboard) -> bool {
    cfg.enabled && !cfg.paused
}

/// Runs until `Msg::Stop` or the channel closes. `on_change` fires after every change to the history
/// or its status (the UI then asks for the list again; no clipboard data travels in the event).
pub fn run_loop<S: ClipboardSource>(
    mut source: S,
    store: Arc<Mutex<Store>>,
    mut storage: Box<dyn Storage>,
    rx: Receiver<Msg>,
    mut on_change: impl FnMut(&Status),
    now_ms: impl Fn() -> u64,
) {
    {
        let mut s = store.lock().unwrap();
        if s.config().persist {
            let items = storage.load();
            s.load(items);
        } else {
            // A previous run may have persisted; the user has since chosen not to keep anything.
            storage.clear();
        }
        s.dirty = false;
    }

    let mut active = false;
    let mut last_seq: Option<u32> = None;
    let mut busy_retries = 0u32;
    let mut last_save = 0u64;
    let emit = |store: &Arc<Mutex<Store>>, active: bool, on_change: &mut dyn FnMut(&Status)| {
        let st = store.lock().unwrap().status(active);
        on_change(&st);
    };
    {
        let cfg = store.lock().unwrap().config().clone();
        if recording(&cfg) {
            active = true;
            last_seq = Some(source.sequence()); // what was already there before VEX started is not read
        }
    }
    emit(&store, active, &mut on_change);

    loop {
        let wait = if active { POLL_INTERVAL } else { IDLE_INTERVAL };
        let mut changed = false;
        match rx.recv_timeout(wait) {
            Ok(Msg::Stop) => {
                flush(&store, &mut *storage, true, &mut last_save, now_ms());
                return;
            }
            Err(RecvTimeoutError::Disconnected) => {
                flush(&store, &mut *storage, true, &mut last_save, now_ms());
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Ok(Msg::Config(cfg)) => {
                let wipe = store.lock().unwrap().set_config(cfg);
                if wipe {
                    storage.clear();
                    last_save = 0;
                }
                changed = true;
            }
            Ok(Msg::Copy(id, reply)) => {
                let content = store.lock().unwrap().content_of(id);
                let result = match content {
                    None => Err("That entry is no longer in the history.".to_string()),
                    Some(Content::Text(t)) => source.write_text(&t),
                    Some(Content::Image(d)) => image::decode_png(&d.png).and_then(|raw| source.write_image(&raw, &d.png)),
                };
                if result.is_ok() {
                    store.lock().unwrap().touch(id, now_ms());
                    if active {
                        last_seq = Some(source.sequence()); // our own write is not a new copy
                    }
                    changed = true;
                }
                let _ = reply.send(result);
            }
            Ok(Msg::Pin(id, pinned, reply)) => {
                let r = store.lock().unwrap().pin(id, pinned).map_err(String::from);
                changed = r.is_ok();
                let _ = reply.send(r);
            }
            Ok(Msg::Delete(id)) => changed = store.lock().unwrap().delete(id),
            Ok(Msg::Clear(keep_pinned)) => {
                store.lock().unwrap().clear(keep_pinned);
                changed = true;
            }
        }

        // Start or stop recording, taking a fresh baseline so nothing copied while off is ever read.
        let want = recording(store.lock().unwrap().config());
        if want != active {
            active = want;
            busy_retries = 0;
            last_seq = if active { Some(source.sequence()) } else { None };
            changed = true;
        }

        if active {
            let seq = source.sequence();
            if Some(seq) != last_seq {
                let (images, skip_sensitive) = {
                    let s = store.lock().unwrap();
                    (s.config().images, s.config().skip_sensitive)
                };
                match source.read(images) {
                    Captured::Busy if busy_retries < MAX_BUSY_RETRIES => busy_retries += 1,
                    Captured::Busy | Captured::Nothing => {
                        busy_retries = 0;
                        last_seq = Some(seq);
                    }
                    Captured::Failed(e) => {
                        busy_retries = 0;
                        last_seq = Some(seq);
                        store.lock().unwrap().set_note(Some(format!("Could not read the clipboard: {e}")));
                        changed = true;
                    }
                    Captured::Sensitive => {
                        busy_retries = 0;
                        last_seq = Some(seq);
                        store.lock().unwrap().note_sensitive_skip();
                        changed = true;
                    }
                    Captured::Text(t) => {
                        busy_retries = 0;
                        last_seq = Some(seq);
                        let mut s = store.lock().unwrap();
                        if skip_sensitive && looks_sensitive(&t) {
                            s.note_sensitive_skip();
                            changed = true;
                        } else {
                            changed |= !matches!(s.add_text(t, now_ms()), AddOutcome::SameAsLatest | AddOutcome::Skipped(_)) || s.status(true).note.is_some();
                        }
                    }
                    Captured::Image(raw) => {
                        busy_retries = 0;
                        last_seq = Some(seq);
                        // The heavy work (hashing, PNG encoding, thumbnail) runs without holding the lock.
                        if !images {
                            // images are off: ignore
                        } else if raw.rgba.len() / 4 > MAX_PIXELS {
                            store.lock().unwrap().set_note(Some("An image was too large to keep.".into()));
                            changed = true;
                        } else {
                            let hash = image::hash_image(&raw);
                            match ImageData::from_raw(&raw) {
                                Ok(data) => {
                                    let out = store.lock().unwrap().add_image(hash, data, now_ms());
                                    changed |= !matches!(out, AddOutcome::SameAsLatest);
                                }
                                Err(e) => {
                                    store.lock().unwrap().set_note(Some(format!("Could not store an image: {e}")));
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        if changed {
            emit(&store, active, &mut on_change);
        }
        flush(&store, &mut *storage, false, &mut last_save, now_ms());
    }
}

/// Write the history to disk when persistence is on, the history changed, and it has been a moment.
fn flush(store: &Arc<Mutex<Store>>, storage: &mut dyn Storage, force: bool, last_save: &mut u64, now: u64) {
    let mut s = store.lock().unwrap();
    if !s.config().persist || !s.dirty || (!force && now.saturating_sub(*last_save) < SAVE_EVERY_MS) {
        return;
    }
    if storage.save(s.items()).is_ok() {
        s.dirty = false;
        *last_save = now;
    }
}

#[cfg(test)]
mod tests {
    use super::super::storage::FileStorage;
    use super::*;
    use std::sync::mpsc;

    #[derive(Default)]
    struct Shared {
        seq: u32,
        next: Vec<Captured>,
        reads: u32,
        written_text: Vec<String>,
        written_images: Vec<(u32, u32, usize)>,
        write_fails: bool,
        last_want_images: Option<bool>,
    }
    struct Fake(Arc<Mutex<Shared>>);
    impl ClipboardSource for Fake {
        fn sequence(&mut self) -> u32 {
            self.0.lock().unwrap().seq
        }
        fn read(&mut self, want_images: bool) -> Captured {
            let mut s = self.0.lock().unwrap();
            s.reads += 1;
            s.last_want_images = Some(want_images);
            if s.next.is_empty() {
                Captured::Nothing
            } else {
                s.next.remove(0)
            }
        }
        fn write_text(&mut self, text: &str) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            if s.write_fails {
                return Err("clipboard busy".into());
            }
            s.written_text.push(text.into());
            s.seq += 1;
            Ok(())
        }
        fn write_image(&mut self, image: &RawImage, png: &[u8]) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            s.written_images.push((image.width, image.height, png.len()));
            s.seq += 1;
            Ok(())
        }
    }

    struct Rig {
        tx: mpsc::Sender<Msg>,
        shared: Arc<Mutex<Shared>>,
        store: Arc<Mutex<Store>>,
        statuses: Arc<Mutex<Vec<Status>>>,
        join: Option<std::thread::JoinHandle<()>>,
    }

    impl Rig {
        fn new(cfg: Clipboard, dir: Option<std::path::PathBuf>) -> Rig {
            let shared = Arc::new(Mutex::new(Shared { seq: 10, ..Default::default() }));
            let store = Arc::new(Mutex::new(Store::new(cfg)));
            let statuses = Arc::new(Mutex::new(vec![]));
            let (tx, rx) = mpsc::channel();
            let storage: Box<dyn Storage> = Box::new(FileStorage::new(dir.unwrap_or_else(|| std::env::temp_dir().join("vex-clip-test-unused"))));
            let (s2, st2, ss2) = (shared.clone(), store.clone(), statuses.clone());
            let join = std::thread::spawn(move || run_loop(Fake(s2), st2, storage, rx, move |s| ss2.lock().unwrap().push(s.clone()), || 1_000_000));
            // The first status event means the loop has taken its baseline; copies made before that would be "already there".
            for _ in 0..500 {
                if !statuses.lock().unwrap().is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Rig { tx, shared, store, statuses, join: Some(join) }
        }
        /// Simulate another program copying something.
        fn copy(&self, c: Captured) {
            let mut s = self.shared.lock().unwrap();
            // The real clipboard holds only the latest content: a new copy replaces the old one.
            s.next = vec![c];
            s.seq += 1;
        }
        fn wait_for(&self, what: &str, f: impl Fn(&Store) -> bool) {
            for _ in 0..400 {
                if f(&self.store.lock().unwrap()) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("timed out waiting for: {what}");
        }
        fn texts(&self) -> Vec<String> {
            self.store.lock().unwrap().items().iter().filter_map(|i| if let Content::Text(t) = &i.content { Some(t.clone()) } else { None }).collect()
        }
        fn settle(&self) {
            std::thread::sleep(POLL_INTERVAL * 2 + Duration::from_millis(100));
        }
        fn stop(mut self) {
            self.tx.send(Msg::Stop).unwrap();
            self.join.take().unwrap().join().unwrap();
        }
    }

    fn cfg() -> Clipboard {
        Clipboard { enabled: true, ..Clipboard::default() }
    }

    fn pic(seed: u8) -> RawImage {
        RawImage::new(16, 12, (0..16 * 12 * 4).map(|i| (i as u8).wrapping_mul(seed)).collect()).unwrap()
    }

    #[test]
    fn records_new_copies_but_not_what_was_there_before_start() {
        let rig = Rig::new(cfg(), None);
        // The loop's baseline was taken at start, so a seq that was already current reads nothing.
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(rig.shared.lock().unwrap().reads, 0);
        rig.copy(Captured::Text("first".into()));
        rig.wait_for("first", |s| s.len() == 1);
        rig.copy(Captured::Text("second".into()));
        rig.wait_for("second", |s| s.len() == 2);
        assert_eq!(rig.texts(), vec!["first", "second"]);
        rig.stop();
    }

    #[test]
    fn disabled_or_paused_never_reads_and_resuming_ignores_what_was_copied_meanwhile() {
        let rig = Rig::new(Clipboard::default(), None); // disabled
        rig.copy(Captured::Text("while off".into()));
        rig.settle();
        assert_eq!(rig.shared.lock().unwrap().reads, 0);

        rig.tx.send(Msg::Config(cfg())).unwrap(); // enable: baseline now
        rig.settle();
        assert_eq!(rig.shared.lock().unwrap().reads, 0, "what was copied before enabling is not read");
        rig.copy(Captured::Text("after enabling".into()));
        rig.wait_for("recorded", |s| s.len() == 1);

        rig.tx.send(Msg::Config(Clipboard { paused: true, ..cfg() })).unwrap();
        rig.settle();
        let reads = rig.shared.lock().unwrap().reads;
        rig.copy(Captured::Text("while paused".into()));
        rig.settle();
        assert_eq!(rig.shared.lock().unwrap().reads, reads, "paused means no clipboard reads at all");

        rig.tx.send(Msg::Config(cfg())).unwrap(); // resume
        rig.settle();
        assert_eq!(rig.texts(), vec!["after enabling"]);
        rig.copy(Captured::Text("after resume".into()));
        rig.wait_for("resumed", |s| s.len() == 2);
        rig.stop();
    }

    #[test]
    fn sensitive_content_is_dropped_whether_flagged_by_the_source_or_by_its_shape() {
        let rig = Rig::new(cfg(), None);
        rig.copy(Captured::Sensitive);
        rig.settle();
        rig.copy(Captured::Text(concat!("gh", "p_abcdefghijklmnopqrstuvwxyz0123456789").into()));
        rig.settle();
        rig.copy(Captured::Text("an ordinary note".into()));
        rig.wait_for("ordinary", |s| s.len() == 1);
        let st = rig.store.lock().unwrap().status(true);
        assert_eq!(st.skipped_sensitive, 2);
        assert_eq!(rig.texts(), vec!["an ordinary note"]);

        // With the filter off the text-shape rule is not applied (the source flag always is).
        rig.tx.send(Msg::Config(Clipboard { skip_sensitive: false, ..cfg() })).unwrap();
        rig.settle();
        rig.copy(Captured::Text(concat!("gh", "p_abcdefghijklmnopqrstuvwxyz0123456789").into()));
        rig.wait_for("token kept when the filter is off", |s| s.len() == 2);
        rig.copy(Captured::Sensitive);
        rig.settle();
        assert_eq!(rig.store.lock().unwrap().len(), 2);
        rig.stop();
    }

    #[test]
    fn consecutive_duplicates_are_stored_once_and_old_duplicates_move_up() {
        let rig = Rig::new(cfg(), None);
        for t in ["a", "a", "b", "a"] {
            rig.copy(Captured::Text(t.into()));
            rig.settle();
        }
        let items = rig.store.lock().unwrap().list("");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].preview, "a"); // newest
        rig.stop();
    }

    #[test]
    fn images_are_captured_and_respect_the_images_setting() {
        let rig = Rig::new(cfg(), None);
        rig.copy(Captured::Image(pic(3)));
        rig.wait_for("image", |s| s.len() == 1);
        let v = rig.store.lock().unwrap().list("");
        assert_eq!((v[0].kind, v[0].width, v[0].height), ("image", 16, 12));

        rig.tx.send(Msg::Config(Clipboard { images: false, ..cfg() })).unwrap();
        rig.wait_for("purged", |s| s.is_empty());
        rig.copy(Captured::Image(pic(5)));
        rig.settle();
        assert!(rig.store.lock().unwrap().is_empty());
        assert_eq!(rig.shared.lock().unwrap().last_want_images, Some(false), "the source is told not to read images");
        rig.stop();
    }

    #[test]
    fn busy_clipboard_is_retried_then_given_up_on() {
        let rig = Rig::new(cfg(), None);
        {
            let mut s = rig.shared.lock().unwrap();
            s.next = vec![Captured::Busy, Captured::Busy, Captured::Text("got it eventually".into())];
            s.seq += 1;
        }
        rig.wait_for("retry succeeded", |s| s.len() == 1);
        assert_eq!(rig.shared.lock().unwrap().reads, 3);
        rig.stop();
    }

    #[test]
    fn copying_an_entry_back_writes_it_does_not_record_itself_and_moves_it_up() {
        let rig = Rig::new(cfg(), None);
        rig.copy(Captured::Text("alpha".into()));
        rig.wait_for("alpha", |s| s.len() == 1);
        rig.copy(Captured::Text("beta".into()));
        rig.wait_for("beta", |s| s.len() == 2);
        let alpha = rig.store.lock().unwrap().items()[0].id;
        let (rtx, rrx) = mpsc::channel();
        rig.tx.send(Msg::Copy(alpha, rtx)).unwrap();
        assert_eq!(rrx.recv_timeout(Duration::from_secs(2)).unwrap(), Ok(()));
        rig.settle();
        assert_eq!(rig.shared.lock().unwrap().written_text, vec!["alpha"]);
        assert_eq!(rig.store.lock().unwrap().len(), 2, "our own write was not recorded again");
        assert_eq!(rig.store.lock().unwrap().list("")[0].id, alpha);

        // An image goes back at full quality (the PNG is handed to the source untouched).
        rig.copy(Captured::Image(pic(7)));
        rig.wait_for("image", |s| s.len() == 3);
        let img_id = rig.store.lock().unwrap().list("")[0].id;
        let (rtx, rrx) = mpsc::channel();
        rig.tx.send(Msg::Copy(img_id, rtx)).unwrap();
        rrx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
        assert_eq!(rig.shared.lock().unwrap().written_images.len(), 1);
        assert_eq!(rig.shared.lock().unwrap().written_images[0].0, 16);
        rig.stop();
    }

    #[test]
    fn copy_errors_are_reported_and_unknown_ids_fail_cleanly() {
        let rig = Rig::new(cfg(), None);
        rig.copy(Captured::Text("x".into()));
        rig.wait_for("x", |s| s.len() == 1);
        let id = rig.store.lock().unwrap().items()[0].id;
        rig.shared.lock().unwrap().write_fails = true;
        let (rtx, rrx) = mpsc::channel();
        rig.tx.send(Msg::Copy(id, rtx)).unwrap();
        assert_eq!(rrx.recv_timeout(Duration::from_secs(2)).unwrap(), Err("clipboard busy".into()));
        let (rtx, rrx) = mpsc::channel();
        rig.tx.send(Msg::Copy(9999, rtx)).unwrap();
        assert!(rrx.recv_timeout(Duration::from_secs(2)).unwrap().is_err());
        rig.stop();
    }

    #[test]
    fn pin_delete_and_clear_commands() {
        let rig = Rig::new(cfg(), None);
        for t in ["one", "two", "three"] {
            rig.copy(Captured::Text(t.into()));
            rig.settle();
        }
        let ids: Vec<u64> = rig.store.lock().unwrap().items().iter().map(|i| i.id).collect();
        let (rtx, rrx) = mpsc::channel();
        rig.tx.send(Msg::Pin(ids[0], true, rtx)).unwrap();
        assert_eq!(rrx.recv_timeout(Duration::from_secs(2)).unwrap(), Ok(()));
        rig.tx.send(Msg::Delete(ids[1])).unwrap();
        rig.wait_for("deleted", |s| s.len() == 2);
        rig.tx.send(Msg::Clear(true)).unwrap();
        rig.wait_for("only pinned left", |s| s.len() == 1);
        assert_eq!(rig.store.lock().unwrap().items()[0].id, ids[0]);
        rig.tx.send(Msg::Clear(false)).unwrap();
        rig.wait_for("empty", |s| s.is_empty());
        rig.stop();
    }

    #[test]
    fn status_events_describe_recording_state() {
        let rig = Rig::new(cfg(), None);
        rig.settle();
        assert!(rig.statuses.lock().unwrap().first().unwrap().recording);
        rig.tx.send(Msg::Config(Clipboard { paused: true, ..cfg() })).unwrap();
        rig.settle();
        let last = rig.statuses.lock().unwrap().last().cloned().unwrap();
        assert!(last.enabled && last.paused && !last.recording);
        rig.stop();
    }

    #[test]
    fn persistence_saves_restores_and_is_deleted_when_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip");
        let persist = Clipboard { persist: true, ..cfg() };

        let rig = Rig::new(persist.clone(), Some(root.clone()));
        rig.copy(Captured::Text("remember me".into()));
        rig.wait_for("stored", |s| s.len() == 1);
        rig.copy(Captured::Image(pic(9)));
        rig.wait_for("image stored", |s| s.len() == 2);
        rig.stop(); // flushes
        assert!(root.join("index.json").exists());

        // A new run with persistence on gets it back.
        let rig = Rig::new(persist.clone(), Some(root.clone()));
        rig.wait_for("restored", |s| s.len() == 2);
        // Turning persistence off wipes the files at once but keeps this session's history.
        rig.tx.send(Msg::Config(Clipboard { persist: false, ..cfg() })).unwrap();
        for _ in 0..200 {
            if !root.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!root.exists());
        assert_eq!(rig.store.lock().unwrap().len(), 2);
        rig.stop();
        assert!(!root.exists(), "stopping with persistence off writes nothing");

        // Starting with persistence off also removes anything left from an earlier run.
        let mut leftover = FileStorage::new(root.clone());
        leftover.save(&[super::super::store::Item { id: 1, ts_ms: 1, pinned: false, hash: 1, content: Content::Text("old".into()) }]).unwrap();
        let rig = Rig::new(cfg(), Some(root.clone()));
        rig.settle();
        assert!(!root.exists());
        assert!(rig.store.lock().unwrap().is_empty());
        rig.stop();
    }

    #[test]
    fn disabling_with_persistence_off_forgets_and_stops_reading() {
        let rig = Rig::new(cfg(), None);
        rig.copy(Captured::Text("gone soon".into()));
        rig.wait_for("recorded", |s| s.len() == 1);
        rig.tx.send(Msg::Config(Clipboard::default())).unwrap();
        rig.wait_for("forgotten", |s| s.is_empty());
        let reads = rig.shared.lock().unwrap().reads;
        rig.copy(Captured::Text("ignored".into()));
        rig.settle();
        assert_eq!(rig.shared.lock().unwrap().reads, reads);
        rig.stop();
    }
}
