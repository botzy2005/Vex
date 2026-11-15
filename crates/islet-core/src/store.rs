//! Settings persistence: atomic writes and recovery from a corrupt file.

use crate::settings::Settings;
use std::{fs, io, path::PathBuf};

pub struct SettingsStore {
    path: PathBuf,
}

#[derive(Debug)]
pub struct LoadOutcome {
    pub settings: Settings,
    /// True when an existing file could not be parsed and was moved aside.
    pub recovered_from_corrupt: bool,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn load(&self) -> LoadOutcome {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return LoadOutcome {
                    settings: Settings::default(),
                    recovered_from_corrupt: false,
                }
            }
            Err(_) => return self.recover(),
        };
        match serde_json::from_str::<Settings>(&text) {
            Ok(mut s) => {
                s.sanitize();
                LoadOutcome {
                    settings: s,
                    recovered_from_corrupt: false,
                }
            }
            Err(_) => self.recover(),
        }
    }

    fn recover(&self) -> LoadOutcome {
        let mut aside = self.path.clone().into_os_string();
        aside.push(".corrupt");
        let _ = fs::rename(&self.path, PathBuf::from(aside));
        LoadOutcome {
            settings: Settings::default(),
            recovered_from_corrupt: true,
        }
    }

    /// Write to a temp file then rename, so a crash mid-write can never leave
    /// a half-written settings file behind.
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let out = SettingsStore::new(dir.path().join("settings.json")).load();
        assert_eq!(out.settings, Settings::default());
        assert!(!out.recovered_from_corrupt);
    }

    #[test]
    fn settings_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let first = SettingsStore::new(path.clone());
        let changed = Settings::default()
            .with_patch(&json!({"appearance": {"theme": "light"}, "island": {"offsetX": -120}, "music": {"lyricsFontSize": 22}}))
            .unwrap();
        first.save(&changed).unwrap();

        // A brand new store instance stands in for the next app launch.
        let second = SettingsStore::new(path);
        assert_eq!(second.load().settings, changed);
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_defaults_returned() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let out = SettingsStore::new(path.clone()).load();
        assert!(out.recovered_from_corrupt);
        assert_eq!(out.settings, Settings::default());
        assert!(dir.path().join("settings.json.corrupt").exists());
        assert!(!path.exists());
    }

    #[test]
    fn out_of_range_values_on_disk_are_sanitised_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"island":{"offsetX":999999},"appearance":{"glassOpacity":-3.0}}"#,
        )
        .unwrap();
        let s = SettingsStore::new(path).load().settings;
        assert_eq!(s.island.offset_x, 2000);
        assert_eq!(s.appearance.glass_opacity, 0.4);
    }

    #[test]
    fn save_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        store.save(&Settings::default()).unwrap();
        assert!(!dir.path().join("settings.json.tmp").exists());
    }
}
