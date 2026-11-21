//! Widgets Hub core: the widget catalog, the saved dashboard layout, per-widget data documents and the
//! short status line a widget may show on the island. Pure logic; the shell (`src-tauri/src/hub.rs`) owns
//! the window, events and background scheduler.

pub mod timers;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::jsonfile;

/// The catalog, shared with the TypeScript side (it imports the same file).
pub const CATALOG_JSON: &str = include_str!("../../../../src/shared/widgets.json");

pub const MAX_PINNED: usize = 4;
pub const MAX_WIDGETS: usize = 24;
/// Largest widget data document the Hub will store.
pub const MAX_DATA_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    /// 1 x 1
    S,
    /// 2 x 1
    M,
    /// 1 x 2
    T,
    /// 2 x 2
    L,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub sizes: Vec<Size>,
    pub default_size: Size,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Catalog {
    pub widgets: Vec<WidgetDef>,
}

impl Catalog {
    pub fn parse(json: &str) -> Result<Catalog, String> {
        let c: Catalog = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut seen = std::collections::BTreeSet::new();
        for w in &c.widgets {
            if !valid_kind(&w.id) {
                return Err(format!("bad widget id {:?}", w.id));
            }
            if !seen.insert(&w.id) {
                return Err(format!("duplicate widget id {:?}", w.id));
            }
            if w.sizes.is_empty() || !w.sizes.contains(&w.default_size) {
                return Err(format!("widget {:?}: default size must be one of its sizes", w.id));
            }
        }
        Ok(c)
    }

    /// The catalog this build ships. The file is embedded at compile time and checked by a test.
    pub fn builtin() -> Catalog {
        Catalog::parse(CATALOG_JSON).expect("src/shared/widgets.json is valid")
    }

    pub fn get(&self, id: &str) -> Option<&WidgetDef> {
        self.widgets.iter().find(|w| w.id == id)
    }
}

/// Widget ids become file names and event payloads, so keep them boring.
pub fn valid_kind(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// ---------------------------------------------------------------- layout

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub kind: String,
    pub size: Size,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub version: u32,
    pub widgets: Vec<Placed>,
}

impl Layout {
    /// What a first run shows: just the "Getting started" card, when the catalog has it.
    pub fn initial(cat: &Catalog) -> Layout {
        let widgets = cat
            .get("welcome")
            .map(|w| vec![Placed { kind: w.id.clone(), size: w.default_size, pinned: false }])
            .unwrap_or_default();
        Layout { version: 1, widgets }
    }

    /// Make any layout safe to show: unknown or duplicate kinds go, a size the widget does not offer
    /// becomes its default, the number of cards and of pinned cards is capped.
    pub fn sanitize(&mut self, cat: &Catalog) {
        self.version = 1;
        let mut seen = std::collections::BTreeSet::new();
        self.widgets.retain(|p| cat.get(&p.kind).is_some() && seen.insert(p.kind.clone()));
        for p in &mut self.widgets {
            if let Some(def) = cat.get(&p.kind) {
                if !def.sizes.contains(&p.size) {
                    p.size = def.default_size;
                }
            }
        }
        self.widgets.truncate(MAX_WIDGETS);
        let mut pinned = 0;
        for p in &mut self.widgets {
            if p.pinned {
                if pinned >= MAX_PINNED {
                    p.pinned = false;
                } else {
                    pinned += 1;
                }
            }
        }
    }
}

/// Load the saved layout. Missing file: the first-run layout. Unreadable file: moved aside, first-run
/// layout. An empty saved layout stays empty (the user removed everything on purpose).
pub fn load_layout(path: &Path, cat: &Catalog) -> (Layout, bool) {
    let (loaded, recovered) = jsonfile::load::<Layout>(path);
    let mut l = loaded.unwrap_or_else(|| Layout::initial(cat));
    l.sanitize(cat);
    (l, recovered)
}

// ---------------------------------------------------------------- per-widget data

pub fn data_path(dir: &Path, kind: &str) -> PathBuf {
    dir.join(format!("{kind}.json"))
}

fn known(cat: &Catalog, kind: &str) -> Result<(), String> {
    if valid_kind(kind) && cat.get(kind).is_some() {
        Ok(())
    } else {
        Err("That widget does not exist.".into())
    }
}

/// One JSON document per widget kind. A missing or unreadable document reads as `null`.
pub fn read_data(dir: &Path, cat: &Catalog, kind: &str) -> Result<Value, String> {
    known(cat, kind)?;
    Ok(jsonfile::load::<Value>(&data_path(dir, kind)).0.unwrap_or(Value::Null))
}

pub fn write_data(dir: &Path, cat: &Catalog, kind: &str, value: &Value) -> Result<(), String> {
    known(cat, kind)?;
    let size = serde_json::to_vec(value).map_err(|e| e.to_string())?.len();
    if size > MAX_DATA_BYTES {
        return Err(format!("That is too much data for one widget (limit {} KB).", MAX_DATA_BYTES / 1024));
    }
    jsonfile::save(&data_path(dir, kind), value).map_err(|_| "Could not save the widget's data.".to_string())
}

// ---------------------------------------------------------------- island status

pub const STATUS_LABEL_MAX: usize = 40;
pub const STATUS_DETAIL_MAX: usize = 80;

/// One short line a widget may ask the island to show. In memory only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub label: String,
    #[serde(default)]
    pub detail: Option<String>,
    /// 0 (ambient) to 3 (urgent). Higher wins when several widgets publish.
    #[serde(default)]
    pub priority: u8,
}

fn clean_line(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(max).collect()
}

pub fn clean_status(s: Status) -> Result<Status, String> {
    let label = clean_line(&s.label, STATUS_LABEL_MAX);
    if label.is_empty() {
        return Err("A status needs a label.".into());
    }
    let detail = s.detail.map(|d| clean_line(&d, STATUS_DETAIL_MAX)).filter(|d| !d.is_empty());
    Ok(Status { label, detail, priority: s.priority.min(3) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cat() -> Catalog {
        Catalog::parse(
            r#"{"widgets":[
              {"id":"a","title":"A","description":"","sizes":["s","m"],"defaultSize":"s"},
              {"id":"b","title":"B","description":"","sizes":["m","l"],"defaultSize":"m"},
              {"id":"c","title":"C","description":"","sizes":["s"],"defaultSize":"s"},
              {"id":"d","title":"D","description":"","sizes":["s"],"defaultSize":"s"},
              {"id":"e","title":"E","description":"","sizes":["s"],"defaultSize":"s"},
              {"id":"f","title":"F","description":"","sizes":["s"],"defaultSize":"s"}
            ]}"#,
        )
        .unwrap()
    }

    fn placed(kind: &str, size: Size, pinned: bool) -> Placed {
        Placed { kind: kind.into(), size, pinned }
    }

    #[test]
    fn shipped_catalog_is_valid_and_has_the_starter_widget() {
        let c = Catalog::builtin();
        assert!(c.get("welcome").is_some());
        assert_eq!(Layout::initial(&c).widgets.len(), 1);
    }

    #[test]
    fn catalog_rejects_bad_entries() {
        assert!(Catalog::parse(r#"{"widgets":[{"id":"Bad Id","title":"","description":"","sizes":["s"],"defaultSize":"s"}]}"#).is_err());
        assert!(Catalog::parse(r#"{"widgets":[{"id":"a","title":"","description":"","sizes":["s"],"defaultSize":"m"}]}"#).is_err());
        assert!(Catalog::parse(r#"{"widgets":[{"id":"a","title":"","description":"","sizes":[],"defaultSize":"s"}]}"#).is_err());
        let dup = r#"{"id":"a","title":"","description":"","sizes":["s"],"defaultSize":"s"}"#;
        assert!(Catalog::parse(&format!(r#"{{"widgets":[{dup},{dup}]}}"#)).is_err());
    }

    #[test]
    fn sanitize_drops_unknown_and_duplicates_and_fixes_sizes() {
        let mut l = Layout {
            version: 9,
            widgets: vec![placed("a", Size::L, false), placed("zzz", Size::S, false), placed("a", Size::S, false), placed("b", Size::S, true)],
        };
        l.sanitize(&cat());
        assert_eq!(l.version, 1);
        assert_eq!(l.widgets, vec![placed("a", Size::S, false), placed("b", Size::M, true)]);
    }

    #[test]
    fn at_most_four_pins_survive_in_order() {
        let mut l = Layout { version: 1, widgets: ["a", "b", "c", "d", "e", "f"].iter().map(|k| placed(k, Size::S, true)).collect() };
        l.widgets[1].size = Size::M;
        l.sanitize(&cat());
        let pinned: Vec<_> = l.widgets.iter().filter(|p| p.pinned).map(|p| p.kind.as_str()).collect();
        assert_eq!(pinned, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn load_layout_handles_missing_empty_and_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("hub.json");
        let c = Catalog::builtin();
        let (l, rec) = load_layout(&p, &c);
        assert_eq!((l, rec), (Layout::initial(&c), false));

        jsonfile::save(&p, &Layout { version: 1, widgets: vec![] }).unwrap();
        let (l, _) = load_layout(&p, &c);
        assert!(l.widgets.is_empty(), "removing everything is remembered");

        std::fs::write(&p, "not json").unwrap();
        let (l, rec) = load_layout(&p, &c);
        assert_eq!((l, rec), (Layout::initial(&c), true));
        assert!(dir.path().join("hub.json.corrupt").exists());
    }

    #[test]
    fn layout_json_shape_is_what_the_ui_sends() {
        let l: Layout = serde_json::from_value(json!({"version":1,"widgets":[{"kind":"a","size":"m","pinned":true},{"kind":"b","size":"l"}]})).unwrap();
        assert_eq!(l.widgets, vec![placed("a", Size::M, true), placed("b", Size::L, false)]);
        assert_eq!(serde_json::to_value(&l.widgets[0]).unwrap(), json!({"kind":"a","size":"m","pinned":true}));
    }

    #[test]
    fn widget_data_round_trips_and_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let c = cat();
        assert_eq!(read_data(dir.path(), &c, "a").unwrap(), Value::Null);
        write_data(dir.path(), &c, "a", &json!({"notes":["x"]})).unwrap();
        assert_eq!(read_data(dir.path(), &c, "a").unwrap(), json!({"notes":["x"]}));
        let big = json!({"x": "y".repeat(MAX_DATA_BYTES)});
        assert!(write_data(dir.path(), &c, "a", &big).unwrap_err().contains("too much"));
        assert_eq!(read_data(dir.path(), &c, "a").unwrap(), json!({"notes":["x"]}), "a refused write changes nothing");
    }

    #[test]
    fn widget_data_only_for_known_kinds_and_never_escapes_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let c = cat();
        for bad in ["nope", "../a", "a/../a", "", "A", "a.json", "a\\b"] {
            assert!(read_data(dir.path(), &c, bad).is_err(), "{bad}");
            assert!(write_data(dir.path(), &c, bad, &json!(1)).is_err(), "{bad}");
        }
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn corrupt_widget_data_reads_as_null() {
        let dir = tempfile::tempdir().unwrap();
        let c = cat();
        std::fs::write(data_path(dir.path(), "a"), "{oops").unwrap();
        assert_eq!(read_data(dir.path(), &c, "a").unwrap(), Value::Null);
    }

    #[test]
    fn status_is_cleaned_and_limited() {
        let s = clean_status(Status { label: "  Focus\n 12:00 ".into(), detail: Some("x".repeat(500)), priority: 99 }).unwrap();
        assert_eq!(s.label, "Focus 12:00");
        assert_eq!(s.detail.unwrap().chars().count(), STATUS_DETAIL_MAX);
        assert_eq!(s.priority, 3);
        assert!(clean_status(Status { label: " \n".into(), detail: None, priority: 0 }).is_err());
        assert_eq!(clean_status(Status { label: "a".into(), detail: Some("  ".into()), priority: 0 }).unwrap().detail, None);
    }
}
