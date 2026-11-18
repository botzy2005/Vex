//! The history: capture-ordered items with de-duplication, pinning, search and limits.

use super::image::{self, RawImage};
use crate::settings::Clipboard;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;

/// Total PNG bytes kept for images (oldest unpinned images are dropped beyond this).
pub const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
/// Longer text is not recorded (copying it back would need the whole of it, and it would bloat memory).
pub const MAX_TEXT_CHARS: usize = 200_000;
const PREVIEW_CHARS: usize = 240;
const PREVIEW_LINES: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
    pub thumb_png: Vec<u8>,
    /// `data:image/png;base64,…` of the thumbnail, built once.
    pub thumb_url: String,
}

impl ImageData {
    pub fn from_raw(raw: &RawImage) -> Result<Self, String> {
        let png = image::encode_png(raw)?;
        let thumb = image::thumbnail(raw, image::THUMB_EDGE);
        let thumb_png = image::encode_png(&thumb)?;
        Ok(Self::from_parts(raw.width, raw.height, png, thumb_png))
    }

    pub fn from_parts(width: u32, height: u32, png: Vec<u8>, thumb_png: Vec<u8>) -> Self {
        let thumb_url = format!("data:image/png;base64,{}", STANDARD.encode(&thumb_png));
        Self { width, height, png, thumb_png, thumb_url }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Text(String),
    Image(ImageData),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: u64,
    pub ts_ms: u64,
    pub pinned: bool,
    pub hash: u64,
    pub content: Content,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    Added(u64),
    /// Already in the history; moved to the top instead of being stored twice.
    Bumped(u64),
    /// The same as the most recent copy; ignored.
    SameAsLatest,
    Skipped(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub id: u64,
    pub ts_ms: u64,
    pub pinned: bool,
    /// `"text"` or `"image"`.
    pub kind: &'static str,
    pub preview: String,
    pub chars: usize,
    pub lines: usize,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub paused: bool,
    /// True only while copies are actually being recorded.
    pub recording: bool,
    pub count: usize,
    pub pinned: usize,
    pub image_bytes: usize,
    pub skipped_sensitive: u32,
    /// The latest thing worth telling the user (for example an image that was too large to keep).
    pub note: Option<String>,
    pub persist: bool,
}

pub struct Store {
    cfg: Clipboard,
    /// Capture order, oldest first; the last item is the latest copy.
    items: Vec<Item>,
    next_id: u64,
    image_bytes: usize,
    skipped_sensitive: u32,
    note: Option<String>,
    pub dirty: bool,
}

fn preview_of(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).take(PREVIEW_LINES).collect();
    let joined = lines.join("\n");
    if joined.chars().count() > PREVIEW_CHARS || text.lines().filter(|l| !l.trim().is_empty()).count() > PREVIEW_LINES {
        let mut t: String = joined.chars().take(PREVIEW_CHARS).collect();
        t.push('…');
        t
    } else {
        joined
    }
}

impl Store {
    pub fn new(cfg: Clipboard) -> Self {
        Self { cfg, items: Vec::new(), next_id: 1, image_bytes: 0, skipped_sensitive: 0, note: None, dirty: false }
    }

    pub fn config(&self) -> &Clipboard {
        &self.cfg
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn note_sensitive_skip(&mut self) {
        self.skipped_sensitive = self.skipped_sensitive.saturating_add(1);
    }

    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    /// Replace the history with items loaded from disk.
    pub fn load(&mut self, items: Vec<Item>) {
        self.items = items;
        self.items.sort_by_key(|i| i.ts_ms);
        self.next_id = self.items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        self.image_bytes = self.items.iter().map(image_size).sum();
        self.enforce_limits();
    }

    /// Apply new settings; returns true when the persisted copy must be deleted.
    pub fn set_config(&mut self, cfg: Clipboard) -> bool {
        let old = std::mem::replace(&mut self.cfg, cfg);
        let mut wipe_storage = false;
        if old.persist && !self.cfg.persist {
            wipe_storage = true;
        }
        if !old.persist && self.cfg.persist {
            self.dirty = true;
        }
        // Switching the module off with persistence off ends the session's history.
        if old.enabled && !self.cfg.enabled && !self.cfg.persist {
            self.clear(false);
        }
        if old.images && !self.cfg.images {
            self.items.retain(|i| !matches!(i.content, Content::Image(_)));
            self.image_bytes = self.items.iter().map(image_size).sum();
            self.dirty = true;
        }
        self.enforce_limits();
        wipe_storage
    }

    fn evict_oldest_unpinned(&mut self, only_images: bool) -> bool {
        let at = self.items.iter().position(|i| !i.pinned && (!only_images || matches!(i.content, Content::Image(_))));
        match at {
            Some(i) => {
                let it = self.items.remove(i);
                self.image_bytes -= image_size(&it);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    fn enforce_limits(&mut self) {
        while self.items.len() > self.cfg.max_items as usize && self.evict_oldest_unpinned(false) {}
        while self.image_bytes > MAX_IMAGE_BYTES && self.evict_oldest_unpinned(true) {}
    }

    fn push(&mut self, hash: u64, now_ms: u64, content: Content) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.image_bytes += match &content {
            Content::Image(d) => d.png.len() + d.thumb_png.len(),
            Content::Text(_) => 0,
        };
        self.items.push(Item { id, ts_ms: now_ms, pinned: false, hash, content });
        self.dirty = true;
        self.enforce_limits();
        id
    }

    /// De-duplicate by content: same as the latest copy is ignored; an older duplicate moves to the top.
    fn dedupe(&mut self, hash: u64, now_ms: u64) -> Option<AddOutcome> {
        if self.items.last().is_some_and(|i| i.hash == hash) {
            return Some(AddOutcome::SameAsLatest);
        }
        let at = self.items.iter().position(|i| i.hash == hash)?;
        let mut it = self.items.remove(at);
        it.ts_ms = now_ms;
        let id = it.id;
        self.items.push(it);
        self.dirty = true;
        Some(AddOutcome::Bumped(id))
    }

    pub fn add_text(&mut self, text: String, now_ms: u64) -> AddOutcome {
        if text.trim().is_empty() {
            return AddOutcome::Skipped("empty".into());
        }
        if text.chars().count() > MAX_TEXT_CHARS {
            self.note = Some(format!("A copy of more than {MAX_TEXT_CHARS} characters was too long to keep."));
            return AddOutcome::Skipped("too long".into());
        }
        let hash = image::hash_text(&text);
        if let Some(o) = self.dedupe(hash, now_ms) {
            return o;
        }
        AddOutcome::Added(self.push(hash, now_ms, Content::Text(text)))
    }

    pub fn add_image(&mut self, hash: u64, data: ImageData, now_ms: u64) -> AddOutcome {
        if !self.cfg.images {
            return AddOutcome::Skipped("images are off".into());
        }
        if data.png.len() + data.thumb_png.len() > MAX_IMAGE_BYTES / 2 {
            self.note = Some("An image was too large to keep.".into());
            return AddOutcome::Skipped("too large".into());
        }
        if let Some(o) = self.dedupe(hash, now_ms) {
            return o;
        }
        AddOutcome::Added(self.push(hash, now_ms, Content::Image(data)))
    }

    /// Move an item to the top (it was just copied back to the clipboard).
    pub fn touch(&mut self, id: u64, now_ms: u64) {
        if let Some(at) = self.items.iter().position(|i| i.id == id) {
            let mut it = self.items.remove(at);
            it.ts_ms = now_ms;
            self.items.push(it);
            self.dirty = true;
        }
    }

    pub fn pin(&mut self, id: u64, pinned: bool) -> Result<(), &'static str> {
        if pinned && self.items.iter().filter(|i| i.pinned).count() >= self.cfg.max_items as usize {
            return Err("Too many pinned entries. Unpin one first.");
        }
        match self.items.iter_mut().find(|i| i.id == id) {
            Some(it) => {
                it.pinned = pinned;
                self.dirty = true;
                self.enforce_limits();
                Ok(())
            }
            None => Err("That entry is no longer in the history."),
        }
    }

    pub fn delete(&mut self, id: u64) -> bool {
        match self.items.iter().position(|i| i.id == id) {
            Some(at) => {
                let it = self.items.remove(at);
                self.image_bytes -= image_size(&it);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Remove entries. With `keep_pinned` the pinned ones stay.
    pub fn clear(&mut self, keep_pinned: bool) {
        self.items.retain(|i| keep_pinned && i.pinned);
        self.image_bytes = self.items.iter().map(image_size).sum();
        self.note = None;
        self.dirty = true;
    }

    pub fn content_of(&self, id: u64) -> Option<Content> {
        self.items.iter().find(|i| i.id == id).map(|i| i.content.clone())
    }

    /// Pinned first, then everything else; newest first inside each. An empty query lists everything;
    /// otherwise only text entries containing it (case-insensitive) are returned.
    pub fn list(&self, query: &str) -> Vec<ItemView> {
        let q = query.trim().to_lowercase();
        let mut out: Vec<&Item> = self
            .items
            .iter()
            .rev()
            .filter(|i| match &i.content {
                Content::Text(t) => q.is_empty() || t.to_lowercase().contains(&q),
                Content::Image(_) => q.is_empty(),
            })
            .collect();
        out.sort_by_key(|i| !i.pinned); // stable: keeps newest-first within each group
        out.into_iter().map(view_of).collect()
    }

    pub fn status(&self, recording: bool) -> Status {
        Status {
            enabled: self.cfg.enabled,
            paused: self.cfg.paused,
            recording,
            count: self.items.len(),
            pinned: self.items.iter().filter(|i| i.pinned).count(),
            image_bytes: self.image_bytes,
            skipped_sensitive: self.skipped_sensitive,
            note: self.note.clone(),
            persist: self.cfg.persist,
        }
    }
}

fn image_size(i: &Item) -> usize {
    match &i.content {
        Content::Image(d) => d.png.len() + d.thumb_png.len(),
        Content::Text(_) => 0,
    }
}

fn view_of(i: &Item) -> ItemView {
    match &i.content {
        Content::Text(t) => ItemView {
            id: i.id,
            ts_ms: i.ts_ms,
            pinned: i.pinned,
            kind: "text",
            preview: preview_of(t),
            chars: t.chars().count(),
            lines: t.lines().count().max(1),
            width: 0,
            height: 0,
            bytes: t.len(),
            thumb: None,
        },
        Content::Image(d) => ItemView {
            id: i.id,
            ts_ms: i.ts_ms,
            pinned: i.pinned,
            kind: "image",
            preview: String::new(),
            chars: 0,
            lines: 0,
            width: d.width,
            height: d.height,
            bytes: d.png.len(),
            thumb: Some(d.thumb_url.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Clipboard {
        Clipboard { enabled: true, ..Clipboard::default() }
    }

    fn img(seed: u8, w: u32, h: u32) -> (u64, ImageData) {
        let raw = RawImage::new(w, h, (0..w * h * 4).map(|i| (i as u8).wrapping_mul(seed)).collect()).unwrap();
        (image::hash_image(&raw), ImageData::from_raw(&raw).unwrap())
    }

    #[test]
    fn text_is_added_and_consecutive_duplicates_are_ignored() {
        let mut s = Store::new(cfg());
        assert!(matches!(s.add_text("hello".into(), 1), AddOutcome::Added(_)));
        assert_eq!(s.add_text("hello".into(), 2), AddOutcome::SameAsLatest);
        assert_eq!(s.len(), 1);
        assert!(matches!(s.add_text("world".into(), 3), AddOutcome::Added(_)));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn an_older_duplicate_moves_to_the_top_and_keeps_its_pin() {
        let mut s = Store::new(cfg());
        let AddOutcome::Added(a) = s.add_text("a".into(), 1) else { panic!() };
        s.add_text("b".into(), 2);
        s.pin(a, true).unwrap();
        assert_eq!(s.add_text("a".into(), 3), AddOutcome::Bumped(a));
        assert_eq!(s.len(), 2);
        let v = s.list("");
        assert_eq!(v[0].id, a);
        assert!(v[0].pinned);
        assert_eq!(s.items().last().unwrap().id, a); // now the latest copy
        assert_eq!(s.add_text("a".into(), 4), AddOutcome::SameAsLatest);
    }

    #[test]
    fn blank_and_oversized_text_is_not_kept() {
        let mut s = Store::new(cfg());
        assert!(matches!(s.add_text("  \n\t ".into(), 1), AddOutcome::Skipped(_)));
        assert!(matches!(s.add_text("x".repeat(MAX_TEXT_CHARS + 1), 2), AddOutcome::Skipped(_)));
        assert!(s.is_empty());
        assert!(s.status(true).note.is_some());
        assert!(matches!(s.add_text("x".repeat(MAX_TEXT_CHARS), 3), AddOutcome::Added(_)));
    }

    #[test]
    fn images_are_stored_deduplicated_and_limited_by_the_images_setting() {
        let mut s = Store::new(cfg());
        let (h, d) = img(3, 40, 30);
        assert!(matches!(s.add_image(h, d.clone(), 1), AddOutcome::Added(_)));
        assert_eq!(s.add_image(h, d.clone(), 2), AddOutcome::SameAsLatest);
        let v = s.list("");
        assert_eq!(v[0].kind, "image");
        assert_eq!((v[0].width, v[0].height), (40, 30));
        assert!(v[0].thumb.as_deref().unwrap().starts_with("data:image/png;base64,"));
        let mut off = Store::new(Clipboard { images: false, ..cfg() });
        assert!(matches!(off.add_image(h, d, 1), AddOutcome::Skipped(_)));
    }

    #[test]
    fn turning_images_off_purges_stored_images_but_keeps_text() {
        let mut s = Store::new(cfg());
        let (h, d) = img(5, 20, 20);
        s.add_image(h, d, 1);
        s.add_text("keep me".into(), 2);
        s.set_config(Clipboard { images: false, ..cfg() });
        assert_eq!(s.len(), 1);
        assert_eq!(s.status(true).image_bytes, 0);
    }

    #[test]
    fn history_limit_drops_the_oldest_unpinned_and_never_a_pinned_one() {
        let mut s = Store::new(Clipboard { max_items: 10, ..cfg() });
        let AddOutcome::Added(first) = s.add_text("t0".into(), 0) else { panic!() };
        s.pin(first, true).unwrap();
        for i in 1..=14u64 {
            s.add_text(format!("t{i}"), i);
        }
        assert_eq!(s.len(), 10);
        assert!(s.items().iter().any(|i| i.id == first), "the pinned entry survived");
        assert_eq!(s.list("")[0].id, first); // pinned sorts first
        assert!(!s.items().iter().any(|i| matches!(&i.content, Content::Text(t) if t == "t1")));
        // Lowering the limit trims at once.
        s.set_config(Clipboard { max_items: 10, ..cfg() });
        assert_eq!(s.len(), 10);
    }

    #[test]
    fn pinning_respects_a_cap_and_unknown_ids_fail_cleanly() {
        let mut s = Store::new(Clipboard { max_items: 10, ..cfg() });
        let ids: Vec<u64> = (0..10).map(|i| match s.add_text(format!("p{i}"), i) { AddOutcome::Added(id) => id, o => panic!("{o:?}") }).collect();
        for id in &ids {
            s.pin(*id, true).unwrap();
        }
        let AddOutcome::Added(extra) = s.add_text("extra".into(), 99) else { panic!() };
        assert!(s.pin(extra, true).is_err());
        assert!(s.pin(12345, false).is_err());
        s.pin(ids[0], false).unwrap();
        assert_eq!(s.status(true).pinned, 9);
    }

    #[test]
    fn delete_and_clear_with_and_without_pinned() {
        let mut s = Store::new(cfg());
        let AddOutcome::Added(a) = s.add_text("a".into(), 1) else { panic!() };
        let AddOutcome::Added(b) = s.add_text("b".into(), 2) else { panic!() };
        s.add_text("c".into(), 3);
        s.pin(b, true).unwrap();
        assert!(s.delete(a));
        assert!(!s.delete(a));
        s.clear(true);
        assert_eq!(s.len(), 1);
        assert_eq!(s.list("")[0].id, b);
        s.clear(false);
        assert!(s.is_empty());
    }

    #[test]
    fn search_is_case_insensitive_covers_the_whole_text_and_skips_images() {
        let mut s = Store::new(cfg());
        let long = format!("{} NEEDLE at the very end", "filler ".repeat(100));
        s.add_text(long, 1);
        s.add_text("Another Entry".into(), 2);
        let (h, d) = img(7, 10, 10);
        s.add_image(h, d, 3);
        assert_eq!(s.list("needle").len(), 1);
        assert_eq!(s.list("  ENTRY ").len(), 1);
        assert_eq!(s.list("nothing like this").len(), 0);
        assert_eq!(s.list("").len(), 3);
        assert!(s.list("needle")[0].preview.ends_with('…')); // the preview is short even though the match is far in
    }

    #[test]
    fn previews_are_short_readable_and_report_size() {
        let mut s = Store::new(cfg());
        s.add_text("line one\n\n   \nline two\nline three\nline four\nline five\nline six\nline seven".into(), 1);
        let v = &s.list("")[0];
        assert!(v.preview.starts_with("line one\nline two"));
        assert!(v.preview.ends_with('…'));
        assert!(!v.preview.contains("\n\n"));
        assert_eq!(v.kind, "text");
        assert!(v.chars > 40 && v.lines >= 7);
    }

    #[test]
    fn disabling_forgets_history_unless_persistence_is_on() {
        let mut s = Store::new(cfg());
        s.add_text("x".into(), 1);
        s.set_config(Clipboard::default()); // enabled: false, persist: false
        assert!(s.is_empty());

        let mut p = Store::new(Clipboard { persist: true, ..cfg() });
        p.add_text("x".into(), 1);
        p.set_config(Clipboard { persist: true, enabled: false, ..cfg() });
        assert_eq!(p.len(), 1, "retained because the user chose to persist");
    }

    #[test]
    fn turning_persistence_off_asks_for_the_files_to_be_deleted() {
        let mut s = Store::new(Clipboard { persist: true, ..cfg() });
        assert!(s.set_config(Clipboard { persist: false, ..cfg() }));
        assert!(!s.set_config(Clipboard { persist: false, ..cfg() }));
        s.dirty = false;
        s.set_config(Clipboard { persist: true, ..cfg() });
        assert!(s.dirty, "turning it on schedules a first save");
    }

    #[test]
    fn the_image_byte_budget_evicts_the_oldest_unpinned_images() {
        let mut s = Store::new(Clipboard { max_items: 200, ..cfg() });
        let big = ImageData::from_parts(10, 10, vec![0; MAX_IMAGE_BYTES / 3], vec![0; 10]);
        for i in 0..4u64 {
            s.add_image(1000 + i, big.clone(), i);
        }
        assert!(s.status(true).image_bytes <= MAX_IMAGE_BYTES);
        assert_eq!(s.len(), 2, "three would exceed the budget, so only two fit");
        let huge = ImageData::from_parts(10, 10, vec![0; MAX_IMAGE_BYTES], vec![0; 10]);
        assert!(matches!(s.add_image(9, huge, 9), AddOutcome::Skipped(_)));
        assert!(s.status(true).note.is_some());
    }

    #[test]
    fn touch_moves_an_entry_to_the_top() {
        let mut s = Store::new(cfg());
        let AddOutcome::Added(a) = s.add_text("a".into(), 1) else { panic!() };
        s.add_text("b".into(), 2);
        s.touch(a, 3);
        assert_eq!(s.list("")[0].id, a);
        assert_eq!(s.items().last().unwrap().ts_ms, 3);
    }

    #[test]
    fn load_restores_ids_and_counts_image_bytes() {
        let mut s = Store::new(cfg());
        let (h, d) = img(9, 12, 12);
        let items = vec![
            Item { id: 7, ts_ms: 20, pinned: false, hash: 1, content: Content::Text("later".into()) },
            Item { id: 3, ts_ms: 10, pinned: true, hash: h, content: Content::Image(d.clone()) },
        ];
        s.load(items);
        assert_eq!(s.items()[0].id, 3); // sorted by time
        assert_eq!(s.status(false).image_bytes, d.png.len() + d.thumb_png.len());
        let AddOutcome::Added(id) = s.add_text("new".into(), 30) else { panic!() };
        assert_eq!(id, 8);
    }
}
