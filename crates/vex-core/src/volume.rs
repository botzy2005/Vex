//! Pure volume logic: what the page is told, and the arithmetic for setting and stepping the level.
//! The Windows side (`volume_windows.rs` in the shell) only reads and writes a 0.0..=1.0 scalar and a mute flag.

use serde::Serialize;

/// Largest step a command may ask for (matches the Settings range).
pub const MAX_STEP: i32 = 20;

/// What the page sees. `percent` is the OS scalar rounded to whole percent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeState {
    pub available: bool,
    pub percent: u8,
    pub muted: bool,
    /// The device's friendly name, when Windows gives one.
    pub device: Option<String>,
    /// A short, user-facing message when something failed or there is no device.
    pub error: Option<String>,
}

impl Default for VolumeState {
    fn default() -> Self {
        Self::unavailable("No audio output device")
    }
}

impl VolumeState {
    pub fn unavailable(msg: &str) -> Self {
        Self { available: false, percent: 0, muted: false, device: None, error: Some(msg.to_string()) }
    }

    pub fn from_scalar(scalar: f32, muted: bool, device: Option<String>) -> Self {
        Self { available: true, percent: scalar_to_percent(scalar), muted, device, error: None }
    }
}

/// OS scalar (0.0..=1.0, may be NaN from a broken driver) to whole percent.
pub fn scalar_to_percent(scalar: f32) -> u8 {
    if !scalar.is_finite() {
        return 0;
    }
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Percent from the page (any integer, even nonsense) to the scalar sent to Windows.
pub fn percent_to_scalar(percent: i64) -> f32 {
    percent.clamp(0, 100) as f32 / 100.0
}

/// The new percent after a relative step. The step itself is bounded so a bad value cannot jump the level.
pub fn stepped(current: u8, delta: i64) -> u8 {
    let d = delta.clamp(-(MAX_STEP as i64), MAX_STEP as i64);
    (current as i64 + d).clamp(0, 100) as u8
}

/// Whether setting `percent` should also unmute: Windows unmutes when you raise the level from the slider
/// or keys, and so do we. Setting 0 does not unmute.
pub fn unmutes_on_set(percent: u8) -> bool {
    percent > 0
}

/// Icon level for the speaker button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Muted,
    Low,
    Medium,
    High,
}

pub fn level(percent: u8, muted: bool) -> Level {
    if muted || percent == 0 {
        Level::Muted
    } else if percent < 34 {
        Level::Low
    } else if percent < 67 {
        Level::Medium
    } else {
        Level::High
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_rounds_to_whole_percent_and_survives_garbage() {
        assert_eq!(scalar_to_percent(0.0), 0);
        assert_eq!(scalar_to_percent(0.504), 50);
        assert_eq!(scalar_to_percent(0.505), 51);
        assert_eq!(scalar_to_percent(1.0), 100);
        assert_eq!(scalar_to_percent(1.7), 100);
        assert_eq!(scalar_to_percent(-0.3), 0);
        assert_eq!(scalar_to_percent(f32::NAN), 0);
        assert_eq!(scalar_to_percent(f32::INFINITY), 0);
    }

    #[test]
    fn percent_is_clamped_before_it_reaches_windows() {
        assert_eq!(percent_to_scalar(-5), 0.0);
        assert_eq!(percent_to_scalar(50), 0.5);
        assert_eq!(percent_to_scalar(101), 1.0);
        assert_eq!(percent_to_scalar(i64::MAX), 1.0);
        assert_eq!(percent_to_scalar(i64::MIN), 0.0);
    }

    #[test]
    fn stepping_stops_at_both_ends_and_bounds_the_step() {
        assert_eq!(stepped(50, 5), 55);
        assert_eq!(stepped(50, -5), 45);
        assert_eq!(stepped(98, 5), 100);
        assert_eq!(stepped(100, 5), 100);
        assert_eq!(stepped(3, -5), 0);
        assert_eq!(stepped(0, -5), 0);
        assert_eq!(stepped(50, 1000), 70);
        assert_eq!(stepped(50, -1000), 30);
    }

    #[test]
    fn raising_the_level_unmutes_but_zero_does_not() {
        assert!(unmutes_on_set(1));
        assert!(unmutes_on_set(100));
        assert!(!unmutes_on_set(0));
    }

    #[test]
    fn icon_level_follows_mute_and_percent() {
        assert_eq!(level(80, true), Level::Muted);
        assert_eq!(level(0, false), Level::Muted);
        assert_eq!(level(1, false), Level::Low);
        assert_eq!(level(33, false), Level::Low);
        assert_eq!(level(34, false), Level::Medium);
        assert_eq!(level(66, false), Level::Medium);
        assert_eq!(level(67, false), Level::High);
        assert_eq!(level(100, false), Level::High);
    }

    #[test]
    fn states_report_availability_and_messages() {
        let s = VolumeState::from_scalar(0.42, false, Some("Speakers".into()));
        assert!(s.available && s.percent == 42 && s.error.is_none());
        let u = VolumeState::default();
        assert!(!u.available && u.error.as_deref() == Some("No audio output device"));
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["percent"], 42);
        assert_eq!(j["muted"], false);
    }
}
