//! Optional on-disk copy of the history (only used when the user turns persistence on).
//! Text lives inside `index.json`; each image is a PNG file next to a thumbnail. Everything is under
//! one folder so turning persistence off is a single delete. The files are NOT encrypted.

use super::store::{Content, ImageData, Item};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

pub trait Storage: Send {
    fn load(&mut self) -> Vec<Item>;
    fn save(&mut self, items: &[Item]) -> io::Result<()>;
    /// Delete everything this storage holds.
    fn clear(&mut self);
}

pub struct FileStorage {
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Index {
    version: u32,
    items: Vec<Row>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: u64,
    ts_ms: u64,
    pinned: bool,
    /// Hex, because a JSON number cannot hold every u64.
    hash: String,
    text: Option<String>,
    width: u32,
    height: u32,
}

impl FileStorage {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn index(&self) -> PathBuf {
        self.dir.join("index.json")
    }
    fn png(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.png"))
    }
    fn thumb(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.thumb.png"))
    }
}

impl Storage for FileStorage {
    fn load(&mut self) -> Vec<Item> {
        let Ok(text) = fs::read_to_string(self.index()) else { return vec![] };
        let Ok(index) = serde_json::from_str::<Index>(&text) else { return vec![] };
        index
            .items
            .into_iter()
            .filter_map(|r| {
                let hash = u64::from_str_radix(&r.hash, 16).ok()?;
                let content = match r.text {
                    Some(t) => Content::Text(t),
                    None => {
                        let png = fs::read(self.png(r.id)).ok()?;
                        let thumb = fs::read(self.thumb(r.id)).ok()?;
                        Content::Image(ImageData::from_parts(r.width, r.height, png, thumb))
                    }
                };
                Some(Item { id: r.id, ts_ms: r.ts_ms, pinned: r.pinned, hash, content })
            })
            .collect()
    }

    fn save(&mut self, items: &[Item]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let mut rows = Vec::new();
        let mut keep = std::collections::HashSet::new();
        for it in items {
            let mut row = Row { id: it.id, ts_ms: it.ts_ms, pinned: it.pinned, hash: format!("{:x}", it.hash), text: None, width: 0, height: 0 };
            match &it.content {
                Content::Text(t) => row.text = Some(t.clone()),
                Content::Image(d) => {
                    row.width = d.width;
                    row.height = d.height;
                    // Image bytes never change for an id, so write each file once.
                    if !self.png(it.id).exists() {
                        fs::write(self.png(it.id), &d.png)?;
                        fs::write(self.thumb(it.id), &d.thumb_png)?;
                    }
                    keep.insert(format!("{}.png", it.id));
                    keep.insert(format!("{}.thumb.png", it.id));
                }
            }
            rows.push(row);
        }
        // Remove files for entries that no longer exist.
        for e in fs::read_dir(&self.dir)?.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".png") && !keep.contains(&name) {
                let _ = fs::remove_file(e.path());
            }
        }
        let tmp = self.dir.join("index.json.tmp");
        fs::write(&tmp, serde_json::to_string(&Index { version: 1, items: rows }).map_err(io::Error::other)?)?;
        fs::rename(&tmp, self.index())
    }

    fn clear(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::super::image::RawImage;
    use super::*;

    fn image_item(id: u64) -> Item {
        let raw = RawImage::new(30, 20, (0..30 * 20 * 4).map(|i| (i % 251) as u8).collect()).unwrap();
        Item { id, ts_ms: id * 10, pinned: id == 2, hash: u64::MAX - id, content: Content::Image(ImageData::from_raw(&raw).unwrap()) }
    }

    #[test]
    fn items_roundtrip_through_disk_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = FileStorage::new(dir.path().join("clip"));
        let items = vec![
            Item { id: 1, ts_ms: 10, pinned: false, hash: 0xdead_beef_dead_beef, content: Content::Text("héllo\nwörld ✓".into()) },
            image_item(2),
        ];
        s.save(&items).unwrap();
        let mut back = s.load();
        back.sort_by_key(|i| i.id);
        assert_eq!(back, items);
    }

    #[test]
    fn removed_entries_lose_their_image_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip");
        let mut s = FileStorage::new(root.clone());
        s.save(&[image_item(2), image_item(3)]).unwrap();
        assert!(root.join("2.png").exists() && root.join("3.thumb.png").exists());
        s.save(&[image_item(3)]).unwrap();
        assert!(!root.join("2.png").exists() && !root.join("2.thumb.png").exists());
        assert!(root.join("3.png").exists());
    }

    #[test]
    fn clear_deletes_everything_and_load_tolerates_missing_or_damaged_data() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip");
        let mut s = FileStorage::new(root.clone());
        assert!(s.load().is_empty()); // nothing yet
        s.save(&[image_item(2)]).unwrap();
        fs::remove_file(root.join("2.png")).unwrap();
        assert!(s.load().is_empty(), "an entry whose image file is gone is dropped");
        fs::write(root.join("index.json"), "{ not json").unwrap();
        assert!(s.load().is_empty());
        s.clear();
        assert!(!root.exists());
        s.clear(); // clearing nothing is fine
    }
}
