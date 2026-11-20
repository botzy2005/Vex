//! File and folder search over a bounded, cached name index.
//!
//! The index is built by walking only the folders the user chose (by default Desktop, Documents,
//! Downloads, Pictures, Videos and Music) with hard caps on depth and entry count. Searching never
//! touches the disk: it filters the in-memory list. Symbolic links and Windows reparse points are not
//! followed, hidden and system entries are skipped, and a directory that cannot be read is counted
//! and skipped rather than failing the scan.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use super::rank;

pub const DEFAULT_SCOPES: [&str; 6] = ["Desktop", "Documents", "Downloads", "Pictures", "Videos", "Music"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Folder,
    Document,
    Image,
    Video,
    Audio,
    Archive,
    Code,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub kind: FileKind,
    /// Unix seconds, 0 when unknown.
    pub modified: u64,
}

pub fn kind_of(name: &str, is_dir: bool) -> FileKind {
    if is_dir {
        return FileKind::Folder;
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "doc" | "docx" | "pdf" | "txt" | "md" | "rtf" | "odt" | "xls" | "xlsx" | "csv" | "ppt" | "pptx" | "odp" | "ods" | "epub" => FileKind::Document,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "heic" | "tif" | "tiff" => FileKind::Image,
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "m4v" => FileKind::Video,
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac" | "wma" | "opus" => FileKind::Audio,
        "zip" | "rar" | "7z" | "tar" | "gz" | "iso" => FileKind::Archive,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "json" | "html" | "css" | "toml" | "yml" | "yaml" | "c" | "cpp" | "h" | "cs" | "java" | "go" => FileKind::Code,
        _ => FileKind::Other,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScanLimits {
    pub max_depth: usize,
    pub max_entries: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self { max_depth: 6, max_entries: 60_000 }
    }
}

#[derive(Debug, Default)]
pub struct ScanReport {
    pub entries: Vec<FileEntry>,
    /// Directories that could not be read (permissions, vanished mid-scan).
    pub unreadable: usize,
    /// True when the entry cap stopped the scan early.
    pub truncated: bool,
}

/// Folder names that are never worth indexing.
fn skipped_dir(name: &str) -> bool {
    matches!(name, "node_modules" | ".git" | "target" | "$RECYCLE.BIN" | "System Volume Information" | "__pycache__" | ".venv")
}

#[cfg(windows)]
fn hidden_or_special(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // hidden 0x2, system 0x4, reparse point (junctions, links) 0x400
    meta.file_attributes() & (0x2 | 0x4 | 0x400) != 0
}

#[cfg(not(windows))]
fn hidden_or_special(_meta: &fs::Metadata) -> bool {
    false
}

/// Walk the roots breadth-first within the limits.
pub fn scan(roots: &[PathBuf], limits: ScanLimits) -> ScanReport {
    let mut report = ScanReport::default();
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> = roots.iter().filter(|r| r.is_dir()).map(|r| (r.clone(), 0)).collect();
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(read) = fs::read_dir(&dir) else {
            report.unreadable += 1;
            continue;
        };
        for entry in read.flatten() {
            if report.entries.len() >= limits.max_entries {
                report.truncated = true;
                return report;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if hidden_or_special(&meta) {
                continue;
            }
            let is_dir = ft.is_dir();
            if is_dir && skipped_dir(&name) {
                continue;
            }
            let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            let path = entry.path();
            report.entries.push(FileEntry { path: path.to_string_lossy().into_owned(), kind: kind_of(&name, is_dir), name, modified });
            if is_dir && depth + 1 < limits.max_depth {
                queue.push_back((path, depth + 1));
            }
        }
    }
    report
}

/// Filter by name (and optionally kind), best match first; ties go to the most recently modified.
pub fn search<'a>(index: &'a [FileEntry], query: &str, kind: Option<FileKind>, limit: usize) -> Vec<&'a FileEntry> {
    let mut scored: Vec<(u32, &FileEntry)> = index
        .iter()
        .filter(|e| kind.is_none_or(|k| e.kind == k))
        .filter_map(|e| rank::score(query, &e.name).map(|s| (s, e)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.modified.cmp(&a.1.modified)).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    scored.truncate(limit);
    scored.into_iter().map(|(_, e)| e).collect()
}

/// The folder that contains a path, for "open containing folder". `None` for a root.
pub fn parent_of(path: &str) -> Option<PathBuf> {
    Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: FileKind, modified: u64) -> FileEntry {
        FileEntry { path: format!("C:\\x\\{name}"), name: name.into(), kind, modified }
    }

    #[test]
    fn kinds_by_extension() {
        assert_eq!(kind_of("report.PDF", false), FileKind::Document);
        assert_eq!(kind_of("cat.jpeg", false), FileKind::Image);
        assert_eq!(kind_of("film.mkv", false), FileKind::Video);
        assert_eq!(kind_of("song.flac", false), FileKind::Audio);
        assert_eq!(kind_of("a.zip", false), FileKind::Archive);
        assert_eq!(kind_of("main.rs", false), FileKind::Code);
        assert_eq!(kind_of("noext", false), FileKind::Other);
        assert_eq!(kind_of("photos.jpg", true), FileKind::Folder);
    }

    #[test]
    fn search_filters_and_orders() {
        let idx = vec![
            entry("budget 2025.xlsx", FileKind::Document, 10),
            entry("Budget", FileKind::Folder, 5),
            entry("my budget notes.txt", FileKind::Document, 50),
            entry("holiday.jpg", FileKind::Image, 1),
        ];
        let r = search(&idx, "budget", None, 10);
        assert_eq!(r[0].name, "Budget", "exact name first");
        assert_eq!(r.len(), 3);
        let docs = search(&idx, "budget", Some(FileKind::Document), 10);
        assert_eq!(docs.len(), 2);
        assert_eq!(search(&idx, "budget", None, 1).len(), 1);
        assert!(search(&idx, "zzz", None, 10).is_empty());
        assert!(search(&idx, "", None, 10).is_empty());
    }

    #[test]
    fn equal_scores_prefer_recently_modified() {
        let idx = vec![entry("notes a.txt", FileKind::Document, 1), entry("notes b.txt", FileKind::Document, 99)];
        let r = search(&idx, "notes", None, 10);
        assert_eq!(r[0].name, "notes b.txt");
    }

    #[test]
    fn scan_respects_depth_limits_skips_and_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a/b/c")).unwrap();
        fs::create_dir_all(root.join("node_modules/x")).unwrap();
        fs::create_dir_all(root.join(".secret")).unwrap();
        fs::write(root.join("top.txt"), "x").unwrap();
        fs::write(root.join("a/mid.pdf"), "x").unwrap();
        fs::write(root.join("a/b/c/deep.png"), "x").unwrap();
        fs::write(root.join("node_modules/x/lib.js"), "x").unwrap();
        fs::write(root.join(".secret/hidden.txt"), "x").unwrap();

        let shallow = scan(&[root.to_path_buf()], ScanLimits { max_depth: 2, max_entries: 1000 });
        let names: Vec<_> = shallow.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"top.txt") && names.contains(&"mid.pdf") && names.contains(&"b"));
        assert!(!names.contains(&"deep.png"), "beyond max depth");
        assert!(!names.contains(&"lib.js") && !names.contains(&"hidden.txt"), "skipped folders are not entered");
        assert!(!names.contains(&"node_modules") && !names.contains(&".secret"));

        let deep = scan(&[root.to_path_buf()], ScanLimits::default());
        assert!(deep.entries.iter().any(|e| e.name == "deep.png"));
        assert!(!deep.truncated && deep.unreadable == 0);
    }

    #[test]
    fn scan_stops_at_the_entry_cap() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
        }
        let r = scan(&[dir.path().to_path_buf()], ScanLimits { max_depth: 3, max_entries: 5 });
        assert_eq!(r.entries.len(), 5);
        assert!(r.truncated);
    }

    #[test]
    fn missing_roots_are_ignored_and_unreadable_dirs_counted() {
        let r = scan(&[PathBuf::from("/definitely/not/here")], ScanLimits::default());
        assert!(r.entries.is_empty() && !r.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("private.txt"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let r = scan(&[dir.path().to_path_buf()], ScanLimits::default());
        assert!(r.entries.is_empty());
    }

    #[test]
    fn parent_of_gives_the_containing_folder() {
        assert_eq!(parent_of("/a/b/c.txt"), Some(PathBuf::from("/a/b")));
        assert_eq!(parent_of("c.txt"), None);
    }
}
