//! File names and save locations.

use crate::settings::ImageFormat;
use std::path::{Path, PathBuf};

/// A local date and time, supplied by the shell (this crate has no time-zone database).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Stamp {
    /// A UTC stamp from Unix milliseconds (used where the OS offers no local time).
    pub fn from_epoch_ms_utc(ms: u64) -> Stamp {
        let secs = (ms / 1000) as i64;
        let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        // Howard Hinnant's civil-from-days algorithm.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Stamp { year, month, day, hour: (rem / 3600) as u32, minute: (rem % 3600 / 60) as u32, second: (rem % 60) as u32 }
    }
}

pub fn extension(f: ImageFormat) -> &'static str {
    match f {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
    }
}

/// `VEX Screenshot 2026-11-18 16.28.31.png` (no colons: Windows file names cannot contain them).
pub fn file_name(s: &Stamp, f: ImageFormat) -> String {
    format!("VEX Screenshot {:04}-{:02}-{:02} {:02}.{:02}.{:02}.{}", s.year, s.month, s.day, s.hour, s.minute, s.second, extension(f))
}

/// `dir/name`, or `dir/name (2)`, `(3)`… when that file already exists. A screenshot never overwrites another file.
pub fn unique_path(dir: &Path, file_name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(file_name);
    if !exists(&first) {
        return first;
    }
    let p = Path::new(file_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("VEX Screenshot");
    let ext = p.extension().and_then(|s| s.to_str());
    for n in 2..10_000u32 {
        let name = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = dir.join(name);
        if !exists(&candidate) {
            return candidate;
        }
    }
    first
}

/// The folder screenshots are saved to: the user's choice, else `<Pictures>/VEX Screenshots`.
/// `pictures` is the OS Pictures folder (or the home folder when the OS has none).
pub fn default_dir(setting: &str, pictures: Option<PathBuf>) -> Option<PathBuf> {
    let s = setting.trim();
    if !s.is_empty() {
        let p = PathBuf::from(s);
        return p.is_absolute().then_some(p);
    }
    pictures.map(|p| p.join("VEX Screenshots"))
}

/// The format a chosen file name implies (`.png`, `.jpg`, `.jpeg`), else `fallback`.
pub fn format_for_path(path: &Path, fallback: ImageFormat) -> ImageFormat {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => ImageFormat::Png,
        Some("jpg" | "jpeg") => ImageFormat::Jpeg,
        _ => fallback,
    }
}

/// Make sure the file name ends in an extension that matches the data being written.
/// `.jpeg` is accepted for JPEG; anything else (or nothing) gets the format's own extension appended.
pub fn with_format_extension(path: &Path, f: ImageFormat) -> PathBuf {
    if format_for_path(path, f) == f && path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg")) {
        return path.to_path_buf();
    }
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".");
    name.push(extension(f));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_from_epoch_is_correct_across_leap_days_and_eras() {
        // 2026-11-18 10:58:31 UTC
        assert_eq!(Stamp::from_epoch_ms_utc(1_794_999_511_000), Stamp { year: 2026, month: 11, day: 18, hour: 10, minute: 58, second: 31 });
        assert_eq!(Stamp::from_epoch_ms_utc(0), Stamp { year: 1970, month: 1, day: 1, hour: 0, minute: 0, second: 0 });
        // 2024-02-29 23:59:59 UTC
        assert_eq!(Stamp::from_epoch_ms_utc(1_709_251_199_000), Stamp { year: 2024, month: 2, day: 29, hour: 23, minute: 59, second: 59 });
        // 2000-03-01 00:00:00 UTC
        assert_eq!(Stamp::from_epoch_ms_utc(951_868_800_000), Stamp { year: 2000, month: 3, day: 1, hour: 0, minute: 0, second: 0 });
    }

    #[test]
    fn file_names_are_windows_safe() {
        let s = Stamp { year: 2026, month: 3, day: 9, hour: 4, minute: 5, second: 6 };
        assert_eq!(file_name(&s, ImageFormat::Png), "VEX Screenshot 2026-03-09 04.05.06.png");
        assert_eq!(file_name(&s, ImageFormat::Jpeg), "VEX Screenshot 2026-03-09 04.05.06.jpg");
        assert!(!file_name(&s, ImageFormat::Png).contains([':', '/', '\\', '*', '?', '"', '<', '>', '|']));
    }

    #[test]
    fn an_existing_file_is_never_overwritten() {
        let dir = Path::new("C:/shots");
        let taken = [PathBuf::from("C:/shots/a.png"), PathBuf::from("C:/shots/a (2).png")];
        assert_eq!(unique_path(dir, "a.png", |p| taken.contains(&p.to_path_buf())), PathBuf::from("C:/shots/a (3).png"));
        assert_eq!(unique_path(dir, "b.png", |_| false), PathBuf::from("C:/shots/b.png"));
        assert_eq!(unique_path(dir, "noext", |p| p.ends_with("noext")), PathBuf::from("C:/shots/noext (2)"));
    }

    #[test]
    fn default_folder_prefers_the_setting_but_only_if_absolute() {
        let pics = Some(PathBuf::from("/home/me/Pictures"));
        assert_eq!(default_dir("", pics.clone()), Some(PathBuf::from("/home/me/Pictures/VEX Screenshots")));
        let abs = if cfg!(windows) { "C:\\data\\shots" } else { "/data/shots" };
        assert_eq!(default_dir(&format!("  {abs} "), pics.clone()), Some(PathBuf::from(abs)));
        assert_eq!(default_dir("relative/dir", pics), None, "a relative folder would depend on the working directory");
        assert_eq!(default_dir("", None), None);
    }

    #[test]
    fn extension_rules() {
        assert_eq!(format_for_path(Path::new("x.PNG"), ImageFormat::Jpeg), ImageFormat::Png);
        assert_eq!(format_for_path(Path::new("x.jpeg"), ImageFormat::Png), ImageFormat::Jpeg);
        assert_eq!(format_for_path(Path::new("x.txt"), ImageFormat::Png), ImageFormat::Png);
        assert_eq!(with_format_extension(Path::new("/a/b.png"), ImageFormat::Png), PathBuf::from("/a/b.png"));
        assert_eq!(with_format_extension(Path::new("/a/b.JPEG"), ImageFormat::Jpeg), PathBuf::from("/a/b.JPEG"));
        assert_eq!(with_format_extension(Path::new("/a/b"), ImageFormat::Jpeg), PathBuf::from("/a/b.jpg"));
        assert_eq!(with_format_extension(Path::new("/a/b.txt"), ImageFormat::Png), PathBuf::from("/a/b.txt.png"));
        // Data is PNG but the name says jpg: the name is corrected, never lying about the content.
        assert_eq!(with_format_extension(Path::new("/a/b.jpg"), ImageFormat::Png), PathBuf::from("/a/b.jpg.png"));
    }
}
