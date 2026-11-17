//! Settings schema. The Rust types are the source of truth; the TypeScript
//! mirror lives in `src/shared/types.ts` and `src/shared/defaults.json` is
//! checked against `Settings::default()` by a unit test so they cannot drift.

use std::collections::BTreeMap;
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
    /// Chosen model per provider, keyed by provider id (e.g. "gemini"), so switching providers
    /// never carries one provider's model name into another.
    pub models: BTreeMap<String, String>,
    pub system_prompt: String,
    /// Where Ollama listens. Defaults to the local machine.
    pub ollama_endpoint: String,
    /// Base URL of an OpenAI-compatible server, including any `/v1` (empty until configured).
    pub compatible_base_url: String,
    /// Let the assistant see the current song / lyrics, but only for a message that asks about them.
    /// When off, nothing about your music is ever sent unless you attach it by hand.
    pub context_on_request: bool,
    /// Let reasoning ("thinking") Ollama models think before answering. Off by default: a thinking
    /// model can spend minutes on hidden reasoning tokens on a laptop GPU before the first visible word.
    pub ollama_think: bool,
    /// How long Ollama keeps the model in memory after a reply ("30m", "2h", "-1" = forever).
    /// Keeping it loaded avoids re-loading the model before every conversation.
    pub ollama_keep_alive: String,
    /// Context window (tokens) requested from Ollama. Smaller = less memory and faster prompt handling.
    pub ollama_num_ctx: u32,
}

pub const DEFAULT_OLLAMA_KEEP_ALIVE: &str = "30m";
pub const DEFAULT_OLLAMA_NUM_CTX: u32 = 4096;

/// What the assistant is told about itself. It asks for substance (the old default said only "Keep replies
/// short", which made small models vague), explains the app-context block so the model uses it instead of
/// claiming it cannot see the user's music, and says what to do when no context was shared.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are VEX, an assistant built into a Windows desktop Dynamic Island. \
Give accurate, genuinely useful answers. For explanations, cover the key ideas with the correct terms, define them in plain language and include a concrete example; for simple questions answer directly in a sentence or two. \
Match the length to the question: no filler, no restating the question, no needless warnings, but never leave out substance just to be brief. \
Use Markdown (lists, tables, short code blocks) when it makes an answer clearer. \
If you are not sure of something, say so instead of guessing, and never make up facts. \
Some messages include a block that starts with \"[Context from the VEX app\" holding real, current details from the user's PC, such as the song that is playing or its lyrics. Use it, and never claim you cannot see it. \
Without such a block you cannot see what they are listening to; if they ask about it, tell them to ask \"What song is playing?\" or press Share song in the chat.";

/// Earlier built-in prompts. A saved settings file that still holds one of these unchanged (the user never
/// edited it) is upgraded to the current default; a prompt the user wrote themselves is never touched.
pub const LEGACY_SYSTEM_PROMPTS: &[&str] = &["You are a helpful assistant built into a desktop Dynamic Island. Keep replies short."];

/// Accepts what Ollama accepts for `keep_alive`: a number with s/m/h, "0", or "-1".
pub fn valid_keep_alive(s: &str) -> bool {
    if s == "-1" || s == "0" {
        return true;
    }
    let Some(unit) = s.chars().last() else { return false };
    let digits = &s[..s.len() - unit.len_utf8()];
    matches!(unit, 's' | 'm' | 'h') && !digits.is_empty() && digits.len() <= 4 && digits.chars().all(|c| c.is_ascii_digit())
}

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://localhost:11434";

impl AiProvider {
    pub const ALL: [AiProvider; 6] = [
        AiProvider::Ollama,
        AiProvider::OpenAi,
        AiProvider::Gemini,
        AiProvider::Claude,
        AiProvider::OpenRouter,
        AiProvider::OpenAiCompatible,
    ];

    /// The id used in settings JSON and as the credential name.
    pub fn id(self) -> &'static str {
        match self {
            AiProvider::Ollama => "ollama",
            AiProvider::OpenAi => "openai",
            AiProvider::Gemini => "gemini",
            AiProvider::Claude => "claude",
            AiProvider::OpenRouter => "openrouter",
            AiProvider::OpenAiCompatible => "openai-compatible",
        }
    }

    pub fn from_id(id: &str) -> Option<AiProvider> {
        AiProvider::ALL.into_iter().find(|p| p.id() == id)
    }
}

impl Ai {
    /// The model chosen for `provider`, if any (never a default guess).
    pub fn model_for(&self, provider: AiProvider) -> Option<&str> {
        self.models.get(provider.id()).map(String::as_str).filter(|m| !m.is_empty())
    }
}

/// Trim, drop a trailing slash, and accept only http(s) URLs.
pub fn normalize_endpoint(raw: &str) -> Option<String> {
    let t = raw.trim().trim_end_matches('/');
    let lower = t.to_ascii_lowercase();
    let rest = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://"))?;
    if rest.is_empty() || rest.contains(char::is_whitespace) || t.len() > 300 {
        return None;
    }
    Some(t.to_string())
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
            models: BTreeMap::new(),
            ollama_endpoint: DEFAULT_OLLAMA_ENDPOINT.into(),
            compatible_base_url: String::new(),
            context_on_request: true,
            ollama_think: false,
            ollama_keep_alive: DEFAULT_OLLAMA_KEEP_ALIVE.into(),
            ollama_num_ctx: DEFAULT_OLLAMA_NUM_CTX,
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
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
        ai.models = std::mem::take(&mut ai.models)
            .into_iter()
            .filter(|(k, _)| AiProvider::from_id(k).is_some())
            .map(|(k, v)| (k, v.trim().chars().take(200).collect::<String>()))
            .filter(|(_, v)| !v.is_empty())
            .collect();
        ai.ollama_endpoint = normalize_endpoint(&ai.ollama_endpoint).unwrap_or_else(|| DEFAULT_OLLAMA_ENDPOINT.into());
        ai.compatible_base_url = normalize_endpoint(&ai.compatible_base_url).unwrap_or_default();
        ai.system_prompt = ai.system_prompt.chars().take(4000).collect();
        if LEGACY_SYSTEM_PROMPTS.contains(&ai.system_prompt.trim()) {
            ai.system_prompt = DEFAULT_SYSTEM_PROMPT.into();
        }
        ai.ollama_keep_alive = ai.ollama_keep_alive.trim().to_string();
        if !valid_keep_alive(&ai.ollama_keep_alive) {
            ai.ollama_keep_alive = DEFAULT_OLLAMA_KEEP_ALIVE.into();
        }
        ai.ollama_num_ctx = ai.ollama_num_ctx.clamp(1024, 32768);
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

    #[test]
    fn per_provider_models_merge_and_reset() {
        let d = Settings::default();
        let a = d.with_patch(&json!({"ai": {"models": {"gemini": "gem-x", "ollama": "llama3.2"}}})).unwrap();
        let b = a.with_patch(&json!({"ai": {"models": {"gemini": "gem-y"}}})).unwrap();
        assert_eq!(b.ai.model_for(AiProvider::Gemini), Some("gem-y"));
        assert_eq!(b.ai.model_for(AiProvider::Ollama), Some("llama3.2"), "other providers keep their model");
        let c = b.with_patch(&json!({"ai": {"models": {"gemini": null}}})).unwrap();
        assert_eq!(c.ai.model_for(AiProvider::Gemini), None);
        assert_eq!(Settings::default().ai.model_for(AiProvider::Ollama), None, "no model is ever assumed");
    }

    #[test]
    fn ai_settings_are_sanitised() {
        let s = Settings::default()
            .with_patch(&json!({"ai": {
                "models": {"nonsense": "x", "openai": "  gpt  ", "claude": "   "},
                "ollamaEndpoint": "javascript:alert(1)",
                "compatibleBaseUrl": "http://my server/v1"
            }}))
            .unwrap();
        assert_eq!(s.ai.models.len(), 1);
        assert_eq!(s.ai.model_for(AiProvider::OpenAi), Some("gpt"));
        assert_eq!(s.ai.ollama_endpoint, DEFAULT_OLLAMA_ENDPOINT);
        assert_eq!(s.ai.compatible_base_url, "");
        let t = Settings::default()
            .with_patch(&json!({"ai": {"ollamaEndpoint": " http://127.0.0.1:11434/ ", "compatibleBaseUrl": "https://api.example.com/v1/"}}))
            .unwrap();
        assert_eq!(t.ai.ollama_endpoint, "http://127.0.0.1:11434");
        assert_eq!(t.ai.compatible_base_url, "https://api.example.com/v1");
    }

    #[test]
    fn provider_ids_round_trip() {
        for p in AiProvider::ALL {
            assert_eq!(AiProvider::from_id(p.id()), Some(p));
            assert_eq!(serde_json::to_value(p).unwrap(), json!(p.id()));
        }
    }

    #[test]
    fn ollama_tuning_defaults_and_sanitising() {
        let d = Settings::default();
        assert!(!d.ai.ollama_think);
        assert_eq!((d.ai.ollama_keep_alive.as_str(), d.ai.ollama_num_ctx), ("30m", 4096));
        let s = d.with_patch(&serde_json::json!({"ai": {"ollamaKeepAlive": "banana", "ollamaNumCtx": 99999999}})).unwrap();
        assert_eq!((s.ai.ollama_keep_alive.as_str(), s.ai.ollama_num_ctx), ("30m", 32768));
        let s = d.with_patch(&serde_json::json!({"ai": {"ollamaKeepAlive": "-1", "ollamaNumCtx": 10}})).unwrap();
        assert_eq!((s.ai.ollama_keep_alive.as_str(), s.ai.ollama_num_ctx), ("-1", 1024));
        for ok in ["5m", "30m", "2h", "90s", "0", "-1"] {
            assert!(valid_keep_alive(ok), "{ok}");
        }
        for bad in ["", "m", "5", "5d", "1.5h", "-5m", "5 m", "99999m"] {
            assert!(!valid_keep_alive(bad), "{bad}");
        }
    }

    #[test]
    fn default_prompt_asks_for_substance_and_explains_app_context() {
        let p = DEFAULT_SYSTEM_PROMPT;
        assert!(!p.to_lowercase().contains("keep replies short"), "the old blanket brevity rule must be gone");
        assert!(p.contains("[Context from the VEX app"), "must match the marker compose_user_content adds");
        assert!(p.contains("never claim you cannot see it"));
        assert!(p.chars().count() < 1500, "stays small so it costs little on a small local model");
    }

    #[test]
    fn an_untouched_legacy_prompt_is_upgraded_but_a_custom_one_is_kept() {
        let legacy = LEGACY_SYSTEM_PROMPTS[0];
        let s = Settings::default().with_patch(&serde_json::json!({"ai": {"systemPrompt": legacy}})).unwrap();
        assert_eq!(s.ai.system_prompt, DEFAULT_SYSTEM_PROMPT);
        let s = Settings::default().with_patch(&serde_json::json!({"ai": {"systemPrompt": "  ".to_string() + legacy + "\n"}})).unwrap();
        assert_eq!(s.ai.system_prompt, DEFAULT_SYSTEM_PROMPT, "whitespace around the old default does not make it custom");
        let s = Settings::default().with_patch(&serde_json::json!({"ai": {"systemPrompt": "Answer like a pirate."}})).unwrap();
        assert_eq!(s.ai.system_prompt, "Answer like a pirate.");
        let s = Settings::default().with_patch(&serde_json::json!({"ai": {"systemPrompt": ""}})).unwrap();
        assert_eq!(s.ai.system_prompt, "", "clearing it on purpose still means no system prompt");
    }
}
