//! Launcher state: pinned apps, recent launches and search history. Local JSON, written atomically,
//! recovered (not crashed on) when the file is corrupt. Search history is never sent anywhere.

use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

use super::AppKind;

pub const MAX_PINS: usize = 30;
pub const MAX_RECENTS: usize = 20;
pub const MAX_HISTORY: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Remembered {
    pub id: String,
    pub name: String,
    pub kind: AppKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub id: String,
    pub name: String,
    pub kind: AppKind,
    /// Unix milliseconds of the last launch.
    pub at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherState {
    pub pins: Vec<Remembered>,
    pub recents: Vec<Recent>,
    pub history: Vec<String>,
}

impl LauncherState {
    pub fn is_pinned(&self, id: &str) -> bool {
        self.pins.iter().any(|p| p.id == id)
    }

    /// Returns false when the pin list is full.
    pub fn pin(&mut self, app: Remembered) -> bool {
        if self.is_pinned(&app.id) {
            return true;
        }
        if self.pins.len() >= MAX_PINS {
            return false;
        }
        self.pins.push(app);
        true
    }

    pub fn unpin(&mut self, id: &str) {
        self.pins.retain(|p| p.id != id);
    }

    /// Newest first, one entry per app.
    pub fn record_launch(&mut self, app: Remembered, now_ms: u64) {
        self.recents.retain(|r| r.id != app.id);
        self.recents.insert(0, Recent { id: app.id, name: app.name, kind: app.kind, at: now_ms });
        self.recents.truncate(MAX_RECENTS);
    }

    pub fn forget_app(&mut self, id: &str) {
        self.recents.retain(|r| r.id != id);
        self.pins.retain(|p| p.id != id);
    }

    /// Newest first, no duplicates, ignores blanks.
    pub fn record_search(&mut self, text: &str) {
        let t: String = text.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(120).collect();
        if t.chars().count() < 2 {
            return;
        }
        self.history.retain(|h| !h.eq_ignore_ascii_case(&t));
        self.history.insert(0, t);
        self.history.truncate(MAX_HISTORY);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.recents.clear();
    }

    pub fn sanitize(&mut self) {
        let mut seen = std::collections::BTreeSet::new();
        self.pins.retain(|p| !p.id.is_empty() && seen.insert(p.id.clone()));
        self.pins.truncate(MAX_PINS);
        let mut seen = std::collections::BTreeSet::new();
        self.recents.retain(|r| !r.id.is_empty() && seen.insert(r.id.clone()));
        self.recents.truncate(MAX_RECENTS);
        self.history.retain(|h| !h.trim().is_empty());
        self.history.truncate(MAX_HISTORY);
    }
}

/// Load the state. A missing file is an empty state; an unreadable one is moved aside.
pub fn load(path: &Path) -> LauncherState {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<LauncherState>(&text) {
            Ok(mut s) => {
                s.sanitize();
                s
            }
            Err(_) => {
                let mut aside = path.as_os_str().to_owned();
                aside.push(".corrupt");
                let _ = fs::rename(path, aside);
                LauncherState::default()
            }
        },
        Err(_) => LauncherState::default(),
    }
}

pub fn save(path: &Path, state: &LauncherState) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(state).map_err(io::Error::other)?)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> Remembered {
        Remembered { id: id.into(), name: id.to_uppercase(), kind: AppKind::Desktop }
    }

    #[test]
    fn pin_and_unpin_without_duplicates() {
        let mut s = LauncherState::default();
        assert!(s.pin(app("a")));
        assert!(s.pin(app("a")));
        assert_eq!(s.pins.len(), 1);
        s.unpin("a");
        assert!(s.pins.is_empty());
        s.unpin("missing");
    }

    #[test]
    fn pin_list_is_capped() {
        let mut s = LauncherState::default();
        for i in 0..MAX_PINS {
            assert!(s.pin(app(&format!("a{i}"))));
        }
        assert!(!s.pin(app("one-too-many")));
        assert_eq!(s.pins.len(), MAX_PINS);
    }

    #[test]
    fn recents_are_newest_first_unique_and_capped() {
        let mut s = LauncherState::default();
        for i in 0..30 {
            s.record_launch(app(&format!("a{i}")), i);
        }
        assert_eq!(s.recents.len(), MAX_RECENTS);
        s.record_launch(app("a25"), 99);
        assert_eq!(s.recents[0].id, "a25");
        assert_eq!(s.recents.iter().filter(|r| r.id == "a25").count(), 1);
    }

    #[test]
    fn search_history_rules() {
        let mut s = LauncherState::default();
        s.record_search("  ");
        s.record_search("a");
        assert!(s.history.is_empty(), "blank and one-letter searches are not kept");
        s.record_search("Report");
        s.record_search("budget");
        s.record_search("report");
        assert_eq!(s.history, vec!["report", "budget"]);
        s.record_search("line\nbreak");
        assert_eq!(s.history[0], "linebreak");
    }

    #[test]
    fn clearing_history_also_clears_recents_but_not_pins() {
        let mut s = LauncherState::default();
        s.pin(app("p"));
        s.record_launch(app("r"), 1);
        s.record_search("hello");
        s.clear_history();
        assert!(s.history.is_empty() && s.recents.is_empty());
        assert_eq!(s.pins.len(), 1);
    }

    #[test]
    fn forgetting_an_uninstalled_app_removes_it_everywhere() {
        let mut s = LauncherState::default();
        s.pin(app("x"));
        s.record_launch(app("x"), 1);
        s.forget_app("x");
        assert!(s.pins.is_empty() && s.recents.is_empty());
    }

    #[test]
    fn round_trips_through_disk_and_survives_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("launcher.json");
        assert_eq!(load(&path), LauncherState::default());
        let mut s = LauncherState::default();
        s.pin(app("a"));
        s.record_search("hello");
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), LauncherState::default());
        assert!(dir.path().join("sub").join("launcher.json.corrupt").exists());
    }

    #[test]
    fn unknown_fields_and_duplicates_in_a_file_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("l.json");
        std::fs::write(&path, r#"{"pins":[{"id":"a","name":"A","kind":"desktop"},{"id":"a","name":"A","kind":"desktop"},{"id":"","name":"x","kind":"store"}],"future":1}"#).unwrap();
        let s = load(&path);
        assert_eq!(s.pins.len(), 1);
    }
}
