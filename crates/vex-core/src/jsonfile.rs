//! Small helpers for local JSON documents: atomic writes and "move the unreadable file aside".

use serde::{de::DeserializeOwned, Serialize};
use std::{fs, io, path::Path};

/// Write `value` next to `path` first and rename it into place, so a crash never leaves half a file.
pub fn save<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(io::Error::other)?)?;
    fs::rename(&tmp, path)
}

/// `(value, recovered)`. A missing file is `(None, false)`. A file that cannot be parsed is renamed to
/// `<name>.corrupt` and reported as `(None, true)`, so the caller starts fresh without losing the evidence.
pub fn load<T: DeserializeOwned>(path: &Path) -> (Option<T>, bool) {
    let Ok(text) = fs::read_to_string(path) else { return (None, false) };
    match serde_json::from_str::<T>(&text) {
        Ok(v) => (Some(v), false),
        Err(_) => {
            let mut aside = path.as_os_str().to_owned();
            aside.push(".corrupt");
            let _ = fs::rename(path, aside);
            (None, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_missing_and_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("a.json");
        assert_eq!(load::<Vec<u32>>(&p), (None, false));
        save(&p, &vec![1u32, 2]).unwrap();
        assert_eq!(load::<Vec<u32>>(&p), (Some(vec![1, 2]), false));
        assert!(!p.with_extension("json.tmp").exists());
        fs::write(&p, "{nope").unwrap();
        assert_eq!(load::<Vec<u32>>(&p), (None, true));
        assert!(p.with_extension("json.corrupt").exists());
        assert!(!p.exists());
    }
}
