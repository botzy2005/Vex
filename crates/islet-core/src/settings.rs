//! Settings schema. The Rust types are the source of truth; the TypeScript
//! mirror lives in `src/shared/types.ts` and `src/shared/defaults.json` is
//! checked against `Settings::default()` by a unit test so they cannot drift.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SETTINGS_VERSION: u32 = 1;
pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+I";
pub const DEFAULT_ACCENT: &str = "#0a84ff";
pub const DEFAULT_LYRICS_FONT: &str = "Segoe UI Variable, Segoe UI, system-ui, sans-serif";

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("invalid settings value: {0}")]
    Invalid(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// Floating rounded pill below the top edge.
    Pill,
    /// Hangs from the top edge with a flat top, like a hardware notch.
    Notch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccentSource {
    Artwork,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    Off,
    Subtle,
    Normal,
    Bouncy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiProvider {
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
}

/// How the active lyric line animates in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsStyle {
    /// Each letter rises in with a staggered delay and a sweep follows the singing.
    Letters,
    /// The whole line fades in.
    Line,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    pub theme: Theme,
    pub shape: Shape,
    /// 0.4..=1.0 — opacity of the glass surface.
    pub glass_opacity: f64,
    /// Corner radius of the expanded island in logical px (8..=40).
    pub corner_radius: u32,
    pub accent_source: AccentSource,
    /// `#rrggbb`
    pub custom_accent: String,
    pub motion: Motion,
    /// 0.8..=1.4 — scales every island dimension.
    pub size_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Island {
    /// Index into the OS monitor list; -1 means the primary monitor.
    pub monitor_index: i32,
    /// Horizontal offset from screen centre in logical px.
    pub offset_x: i32,
    /// Gap between the top screen edge and the island in logical px.
    pub margin_top: i32,
    pub hover_to_expand: bool,
    pub hover_delay_ms: u32,
    pub collapse_delay_ms: u32,
    /// Global shortcut that expands/pins or collapses the island.
    pub hotkey: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Music {
    pub artwork: bool,
    pub visualizer: bool,
    pub lyrics_enabled: bool,
    /// Look lyrics up on LRCLIB automatically (sends artist, title and duration to lrclib.net).
    pub lyrics_online: bool,
    pub lyrics_style: LyricsStyle,
    pub lyrics_in_collapsed: bool,
    pub lyrics_font: String,
    pub lyrics_font_size: u32,
    /// 0.25..=3.0 — multiplier on lyric transition speed.
    pub lyrics_animation_speed: f64,
    pub karaoke: bool,
    /// Shifts lyric timing; positive shows lines later.
    pub lyrics_offset_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ai {
    pub provider: AiProvider,
    pub model: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub appearance: Appearance,
    pub island: Island,
    pub music: Music,
    pub ai: Ai,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            shape: Shape::Pill,
            glass_opacity: 0.92,
            corner_radius: 26,
            accent_source: AccentSource::Artwork,
            custom_accent: DEFAULT_ACCENT.into(),
            motion: Motion::Normal,
            size_scale: 1.0,
        }
    }
}

impl Default for Island {
    fn default() -> Self {
        Self {
            monitor_index: -1,
            offset_x: 0,
            margin_top: 8,
            hover_to_expand: true,
            hover_delay_ms: 120,
            collapse_delay_ms: 350,
            hotkey: DEFAULT_HOTKEY.into(),
        }
    }
}

impl Default for Music {
    fn default() -> Self {
        Self {
            artwork: true,
            visualizer: true,
            lyrics_enabled: true,
            lyrics_online: true,
            lyrics_style: LyricsStyle::Letters,
            lyrics_in_collapsed: true,
            lyrics_font: DEFAULT_LYRICS_FONT.into(),
            lyrics_font_size: 15,
            lyrics_animation_speed: 1.0,
            karaoke: true,
            lyrics_offset_ms: 0,
        }
    }
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            provider: AiProvider::Ollama,
            model: String::new(),
            system_prompt: "You are a helpful assistant built into a desktop Dynamic Island. Keep replies short.".into(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            appearance: Appearance::default(),
            island: Island::default(),
            music: Music::default(),
            ai: Ai::default(),
        }
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Font names end up inside a CSS `font-family` declaration, so only a safe
/// character set is allowed.
fn sanitize_font(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | ',' | '-' | '_' | '\''))
        .take(120)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        DEFAULT_LYRICS_FONT.into()
    } else {
        cleaned
    }
}

fn clamp_f(v: f64, lo: f64, hi: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

impl Settings {
    /// Clamp every value into its valid range and repair unusable strings.
    pub fn sanitize(&mut self) {
        self.version = SETTINGS_VERSION;

        let a = &mut self.appearance;
        a.glass_opacity = clamp_f(a.glass_opacity, 0.4, 1.0, 0.92);
        a.corner_radius = a.corner_radius.clamp(8, 40);
        a.size_scale = clamp_f(a.size_scale, 0.8, 1.4, 1.0);
        if !is_hex_color(&a.custom_accent) {
            a.custom_accent = DEFAULT_ACCENT.into();
        }
        a.custom_accent.make_ascii_lowercase();

        let i = &mut self.island;
        i.monitor_index = i.monitor_index.clamp(-1, 16);
        i.offset_x = i.offset_x.clamp(-2000, 2000);
        i.margin_top = i.margin_top.clamp(0, 200);
        i.hover_delay_ms = i.hover_delay_ms.min(2000);
        i.collapse_delay_ms = i.collapse_delay_ms.min(5000);
        i.hotkey = i.hotkey.trim().chars().take(40).collect();
        if i.hotkey.is_empty() {
            i.hotkey = DEFAULT_HOTKEY.into();
        }

        let m = &mut self.music;
        m.lyrics_font = sanitize_font(&m.lyrics_font);
        m.lyrics_font_size = m.lyrics_font_size.clamp(12, 32);
        m.lyrics_animation_speed = clamp_f(m.lyrics_animation_speed, 0.25, 3.0, 1.0);
        m.lyrics_offset_ms = m.lyrics_offset_ms.clamp(-5000, 5000);

        let ai = &mut self.ai;
        ai.model = ai.model.trim().chars().take(200).collect();
        ai.system_prompt = ai.system_prompt.chars().take(4000).collect();
    }

    /// Apply an RFC 7386 merge patch (objects merge, `null` resets a field to
    /// its default) and return the sanitised result. The receiver is untouched
    /// on error.
    pub fn with_patch(&self, patch: &Value) -> Result<Settings, SettingsError> {
        let mut doc = serde_json::to_value(self)?;
        merge_patch(&mut doc, patch);
        let mut next: Settings = serde_json::from_value(doc)?;
        next.sanitize();
        Ok(next)
    }
}

/// RFC 7386 JSON merge patch.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(p) => {
            if !target.is_object() {
                *target = Value::Object(Default::default());
            }
            let t = target.as_object_mut().expect("just made an object");
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        other => *target = other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_already_sane() {
        let mut s = Settings::default();
        let before = s.clone();
        s.sanitize();
        assert_eq!(s, before);
    }

    #[test]
    fn typescript_defaults_match_rust_defaults() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/shared/defaults.json"
        );
        let text = std::fs::read_to_string(path).expect("src/shared/defaults.json must exist");
        let ts: Value = serde_json::from_str(&text).unwrap();
        let rust = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            ts, rust,
            "src/shared/defaults.json drifted from Settings::default()"
        );
    }

    #[test]
    fn patch_merges_nested_fields_only() {
        let s = Settings::default()
            .with_patch(&json!({"appearance": {"theme": "light"}, "island": {"offsetX": 40}}))
            .unwrap();
        assert_eq!(s.appearance.theme, Theme::Light);
        assert_eq!(s.island.offset_x, 40);
        assert_eq!(s.appearance.shape, Shape::Pill); // untouched sibling
        assert_eq!(s.music, Music::default());
    }

    #[test]
    fn null_resets_field_to_default() {
        let s = Settings::default()
            .with_patch(&json!({"island": {"marginTop": 50}}))
            .unwrap();
        assert_eq!(s.island.margin_top, 50);
        let s = s
            .with_patch(&json!({"island": {"marginTop": null}}))
            .unwrap();
        assert_eq!(s.island.margin_top, Island::default().margin_top);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let s = Settings::default()
            .with_patch(&json!({
                "appearance": {"glassOpacity": 7.0, "sizeScale": 0.01, "cornerRadius": 999},
                "island": {"offsetX": 99999, "marginTop": -5, "hoverDelayMs": 999999},
                "music": {"lyricsFontSize": 2, "lyricsAnimationSpeed": 100.0, "lyricsOffsetMs": -99999}
            }))
            .unwrap();
        assert_eq!(s.appearance.glass_opacity, 1.0);
        assert_eq!(s.appearance.size_scale, 0.8);
        assert_eq!(s.appearance.corner_radius, 40);
        assert_eq!(s.island.offset_x, 2000);
        assert_eq!(s.island.margin_top, 0);
        assert_eq!(s.island.hover_delay_ms, 2000);
        assert_eq!(s.music.lyrics_font_size, 12);
        assert_eq!(s.music.lyrics_animation_speed, 3.0);
        assert_eq!(s.music.lyrics_offset_ms, -5000);
    }

    #[test]
    fn wrong_types_are_rejected_without_touching_current() {
        let s = Settings::default();
        assert!(s
            .with_patch(&json!({"appearance": {"theme": "neon"}}))
            .is_err());
        assert!(s
            .with_patch(&json!({"island": {"offsetX": "left"}}))
            .is_err());
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn bad_strings_are_repaired() {
        let s = Settings::default()
            .with_patch(&json!({
                "appearance": {"customAccent": "red; background:url(x)"},
                "island": {"hotkey": "   "},
                "music": {"lyricsFont": "Arial;} body{display:none"}
            }))
            .unwrap();
        assert_eq!(s.appearance.custom_accent, DEFAULT_ACCENT);
        assert_eq!(s.island.hotkey, DEFAULT_HOTKEY);
        assert!(!s.music.lyrics_font.contains(';'));
        assert!(!s.music.lyrics_font.contains('{'));
    }

    #[test]
    fn accent_hex_is_lowercased() {
        let s = Settings::default()
            .with_patch(&json!({"appearance": {"customAccent": "#FFAA00"}}))
            .unwrap();
        assert_eq!(s.appearance.custom_accent, "#ffaa00");
    }

    #[test]
    fn partial_documents_fill_in_defaults() {
        let s: Settings = serde_json::from_str(r#"{"appearance":{"theme":"light"}}"#).unwrap();
        assert_eq!(s.appearance.theme, Theme::Light);
        assert_eq!(s.island, Island::default());
    }
}
