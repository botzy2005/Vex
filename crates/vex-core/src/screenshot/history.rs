//! The capture history. The latest capture is always available (the island offers to edit, copy or
//! save it); how many earlier ones are kept, and where, follows the user's preference.

use crate::settings::ShotHistory;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

pub const SESSION_LIMIT: usize = 10;
pub const KEEP_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotView {
    pub id: u64,
    pub ts_ms: u64,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    /// `data:image/png;base64,…`
    pub thumb: String,
}

struct Shot {
    view: ShotView,
    /// Held in memory (modes Off and Session), or `None` when the PNG lives only in the data folder.
    png: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: u64,
    ts_ms: u64,
    width: u32,
    height: u32,
    bytes: usize,
    thumb: String,
}

#[derive(Serialize, Deserialize)]
struct Index {
    version: u32,
    items: Vec<Row>,
}

pub struct History {
    mode: ShotHistory,
    dir: PathBuf,
    /// Oldest first.
    items: Vec<Shot>,
    next_id: u64,
}

pub fn limit(mode: ShotHistory) -> usize {
    match mode {
        ShotHistory::Off => 1,
        ShotHistory::Session => SESSION_LIMIT,
        ShotHistory::Keep => KEEP_LIMIT,
    }
}

impl History {
    /// Starts empty. `load` reads the data folder when the mode is `Keep`, and clears it otherwise.
    pub fn new(mode: ShotHistory, dir: PathBuf) -> Self {
        Self { mode, dir, items: Vec::new(), next_id: 1 }
    }

    pub fn mode(&self) -> ShotHistory {
        self.mode
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }
    fn png_path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.png"))
    }

    pub fn load(&mut self) {
        if self.mode != ShotHistory::Keep {
            // The user no longer wants anything kept: delete whatever an earlier run left behind.
            self.wipe_disk();
            return;
        }
        let Ok(text) = fs::read_to_string(self.index_path()) else { return };
        let Ok(index) = serde_json::from_str::<Index>(&text) else { return };
        for r in index.items {
            if self.png_path(r.id).is_file() {
                self.next_id = self.next_id.max(r.id + 1);
                self.items.push(Shot { view: ShotView { id: r.id, ts_ms: r.ts_ms, width: r.width, height: r.height, bytes: r.bytes, thumb: r.thumb }, png: None });
            }
        }
        self.enforce_limit();
    }

    fn wipe_disk(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    fn save_index(&self) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let rows = self.items.iter().map(|s| Row { id: s.view.id, ts_ms: s.view.ts_ms, width: s.view.width, height: s.view.height, bytes: s.view.bytes, thumb: s.view.thumb.clone() }).collect();
        let json = serde_json::to_string(&Index { version: 1, items: rows }).map_err(io::Error::other)?;
        let tmp = self.dir.join("index.json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, self.index_path())
    }

    fn forget_file(&self, id: u64) {
        let _ = fs::remove_file(self.png_path(id));
    }

    fn enforce_limit(&mut self) {
        let max = limit(self.mode);
        while self.items.len() > max {
            let old = self.items.remove(0);
            if self.mode == ShotHistory::Keep {
                self.forget_file(old.view.id);
            }
        }
    }

    /// Record a capture. `png` is the lossless encoding; `thumb_png` its small preview.
    pub fn add(&mut self, png: Vec<u8>, width: u32, height: u32, thumb_png: &[u8], ts_ms: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let view = ShotView { id, ts_ms, width, height, bytes: png.len(), thumb: format!("data:image/png;base64,{}", STANDARD.encode(thumb_png)) };
        let mut shot = Shot { view, png: Some(png) };
        if self.mode == ShotHistory::Keep {
            // If the data folder cannot be written the capture stays in memory: it must never be lost
            // just because the history could not be saved.
            let wrote = fs::create_dir_all(&self.dir).and_then(|_| fs::write(self.png_path(id), shot.png.as_deref().unwrap_or_default()));
            if wrote.is_ok() {
                shot.png = None; // the file is the copy
            }
        }
        self.items.push(shot);
        self.enforce_limit();
        if self.mode == ShotHistory::Keep {
            let _ = self.save_index();
        }
        id
    }

    /// Newest first.
    pub fn list(&self) -> Vec<ShotView> {
        self.items.iter().rev().map(|s| s.view.clone()).collect()
    }

    pub fn latest_id(&self) -> Option<u64> {
        self.items.last().map(|s| s.view.id)
    }

    pub fn view(&self, id: u64) -> Option<ShotView> {
        self.items.iter().find(|s| s.view.id == id).map(|s| s.view.clone())
    }

    pub fn png(&self, id: u64) -> Option<Vec<u8>> {
        let s = self.items.iter().find(|s| s.view.id == id)?;
        match &s.png {
            Some(b) => Some(b.clone()),
            None => fs::read(self.png_path(id)).ok(),
        }
    }

    pub fn delete(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|s| s.view.id != id);
        let removed = self.items.len() != before;
        if removed && self.mode == ShotHistory::Keep {
            self.forget_file(id);
            let _ = self.save_index();
        }
        removed
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.wipe_disk();
    }

    /// Apply a new preference. Turning `Keep` off deletes the saved files; turning it on saves what is
    /// held in memory; `Off` keeps only the newest capture.
    pub fn set_mode(&mut self, mode: ShotHistory) {
        if mode == self.mode {
            return;
        }
        let old = self.mode;
        self.mode = mode;
        if old == ShotHistory::Keep {
            // Bring the files back into memory (the newest few), then delete them from disk.
            let keep = limit(mode);
            let skip = self.items.len().saturating_sub(keep);
            let mut kept = Vec::new();
            for (i, s) in std::mem::take(&mut self.items).into_iter().enumerate() {
                if i < skip {
                    continue;
                }
                let png = s.png.or_else(|| fs::read(self.png_path(s.view.id)).ok());
                kept.push(Shot { view: s.view, png });
            }
            self.items = kept;
            self.wipe_disk();
            self.enforce_limit();
        } else if mode == ShotHistory::Keep {
            let dir = self.dir.clone();
            for s in &mut self.items {
                if let Some(bytes) = s.png.take() {
                    if fs::create_dir_all(&dir).and_then(|_| fs::write(dir.join(format!("{}.png", s.view.id)), &bytes)).is_err() {
                        s.png = Some(bytes); // could not write: keep it in memory rather than lose it
                    }
                }
            }
            let _ = self.save_index();
            self.enforce_limit();
        } else {
            self.enforce_limit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(mode: ShotHistory) -> (History, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (History::new(mode, dir.path().join("shots")), dir)
    }
    fn add(h: &mut History, n: u8) -> u64 {
        h.add(vec![n; 10 + n as usize], 4, 3, &[n], u64::from(n))
    }

    #[test]
    fn off_keeps_only_the_latest_and_never_touches_disk() {
        let (mut h, dir) = h(ShotHistory::Off);
        add(&mut h, 1);
        let b = add(&mut h, 2);
        assert_eq!(h.list().len(), 1);
        assert_eq!(h.latest_id(), Some(b));
        assert_eq!(h.png(b).unwrap().len(), 12);
        assert!(!dir.path().join("shots").exists());
    }

    #[test]
    fn session_keeps_a_bounded_number_in_memory_newest_first() {
        let (mut h, dir) = h(ShotHistory::Session);
        for n in 0..15 {
            add(&mut h, n);
        }
        let l = h.list();
        assert_eq!(l.len(), SESSION_LIMIT);
        assert_eq!(l[0].ts_ms, 14);
        assert_eq!(l.last().unwrap().ts_ms, 5);
        assert!(!dir.path().join("shots").exists());
        assert!(l[0].thumb.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn delete_and_clear() {
        let (mut h, _d) = h(ShotHistory::Session);
        let a = add(&mut h, 1);
        let b = add(&mut h, 2);
        assert!(h.delete(a) && !h.delete(a));
        assert!(h.png(a).is_none() && h.png(b).is_some());
        h.clear();
        assert!(h.list().is_empty() && h.latest_id().is_none());
    }

    #[test]
    fn keep_writes_files_survives_a_restart_and_trims_files_with_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shots");
        let mut a = History::new(ShotHistory::Keep, path.clone());
        let first = add(&mut a, 1);
        for n in 2..(KEEP_LIMIT as u8 + 5) {
            add(&mut a, n);
        }
        assert_eq!(a.list().len(), KEEP_LIMIT);
        assert!(!path.join(format!("{first}.png")).exists(), "trimmed entries lose their files");
        let files = fs::read_dir(&path).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x == "png")).count();
        assert_eq!(files, KEEP_LIMIT);

        let mut b = History::new(ShotHistory::Keep, path.clone());
        b.load();
        assert_eq!(b.list().len(), KEEP_LIMIT);
        let newest = b.list()[0].clone();
        assert_eq!(b.png(newest.id).unwrap().len(), newest.bytes);
        // New ids continue after the loaded ones.
        assert!(add(&mut b, 99) > newest.id);
    }

    #[test]
    fn starting_with_keep_off_deletes_what_an_earlier_run_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shots");
        let mut a = History::new(ShotHistory::Keep, path.clone());
        add(&mut a, 1);
        assert!(path.exists());
        let mut b = History::new(ShotHistory::Session, path.clone());
        b.load();
        assert!(!path.exists());
        assert!(b.list().is_empty());
    }

    #[test]
    fn switching_modes_moves_data_without_losing_the_newest_and_deletes_files_when_leaving_keep() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shots");
        let mut h = History::new(ShotHistory::Session, path.clone());
        let a = add(&mut h, 1);
        let b = add(&mut h, 2);
        h.set_mode(ShotHistory::Keep);
        assert!(path.join(format!("{a}.png")).exists() && path.join("index.json").exists());
        assert_eq!(h.png(b).unwrap().len(), 12);
        h.set_mode(ShotHistory::Off);
        assert!(!path.exists(), "turning Keep off removes the saved files");
        assert_eq!(h.list().len(), 1);
        assert_eq!(h.list()[0].id, b);
        assert_eq!(h.png(b).unwrap().len(), 12, "the latest capture is still usable");
    }

    #[test]
    fn a_corrupt_index_is_ignored_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shots");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("index.json"), "{not json").unwrap();
        let mut h = History::new(ShotHistory::Keep, path);
        h.load();
        assert!(h.list().is_empty());
        add(&mut h, 1);
        assert_eq!(h.list().len(), 1);
    }
}
