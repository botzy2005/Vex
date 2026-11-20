//! The launcher's command registry.
//!
//! Every command is an entry in `REGISTRY` (name, aliases, description, usage) and resolves to an
//! `Action`: a closed enum, never a shell string. Arguments are validated here (length, control
//! characters, durations, allowed targets) so the shell crate only ever receives a well-formed action.
//! A command that is not available yet (timers and notes arrive with the Widgets Hub) still parses
//! and is shown, but is marked unavailable with the reason, so it never pretends to work.

use serde::{Deserialize, Serialize};

use super::calc;
use crate::settings::WebEngine;

pub const MAX_TEXT_CHARS: usize = 4000;
pub const MAX_TIMER_SECONDS: u64 = 24 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownFolder {
    Downloads,
    Documents,
    Desktop,
    Pictures,
    Videos,
    Music,
}

pub const FOLDERS: [(KnownFolder, &str); 6] = [
    (KnownFolder::Downloads, "downloads"),
    (KnownFolder::Documents, "documents"),
    (KnownFolder::Desktop, "desktop"),
    (KnownFolder::Pictures, "pictures"),
    (KnownFolder::Videos, "videos"),
    (KnownFolder::Music, "music"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiKind {
    Ask,
    Explain,
    Summarize,
    Rewrite,
    Translate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Calc { expression: String, result: String },
    Time,
    Date,
    OpenFolder { folder: KnownFolder },
    OpenVexSettings,
    OpenWindowsSettings,
    WebSearch { query: String },
    Timer { seconds: u64, label: String },
    Note { text: String },
    Ai { kind: AiKind, text: String, language: Option<String> },
    /// Not runnable: replaces the input text (used for command suggestions).
    Fill { text: String },
    /// Not runnable: explains why the input could not be used.
    Invalid,
}

/// Which optional groups the user has switched on, and which later modules exist yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features {
    pub web: bool,
    pub ai: bool,
    pub timers: bool,
    pub notes: bool,
}

pub struct CommandDef {
    pub id: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage: &'static str,
}

pub static REGISTRY: &[CommandDef] = &[
    CommandDef { id: "calc", name: "calc", aliases: &["calculate", "="], description: "Calculate an expression", usage: "calc 245 * 18" },
    CommandDef { id: "time", name: "time", aliases: &["clock"], description: "Show the current time", usage: "time" },
    CommandDef { id: "date", name: "date", aliases: &["today"], description: "Show today's date", usage: "date" },
    CommandDef { id: "open", name: "open", aliases: &["goto"], description: "Open a folder or VEX settings", usage: "open downloads" },
    CommandDef { id: "search", name: "search", aliases: &["web", "google"], description: "Search the web in your browser", usage: "search cybersecurity" },
    CommandDef { id: "timer", name: "timer", aliases: &["countdown"], description: "Start a countdown timer", usage: "timer 10 minutes" },
    CommandDef { id: "note", name: "note", aliases: &["remember"], description: "Save a quick note", usage: "note buy groceries" },
    CommandDef { id: "ask", name: "ask", aliases: &["ai"], description: "Ask your AI provider a question", usage: "ask what is a firewall?" },
    CommandDef { id: "explain", name: "explain", aliases: &[], description: "Explain text with your AI provider", usage: "explain <text>" },
    CommandDef { id: "summarize", name: "summarize", aliases: &["summarise", "tldr"], description: "Summarise text with your AI provider", usage: "summarize <text>" },
    CommandDef { id: "rewrite", name: "rewrite", aliases: &[], description: "Rewrite text with your AI provider", usage: "rewrite <text>" },
    CommandDef { id: "translate", name: "translate", aliases: &[], description: "Translate text with your AI provider", usage: "translate hello to French" },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandMatch {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub action: Action,
    /// False when the command cannot run (invalid input, or its module does not exist yet).
    pub available: bool,
    pub reason: Option<String>,
}

impl CommandMatch {
    fn ok(id: &str, title: impl Into<String>, subtitle: impl Into<String>, action: Action) -> Self {
        Self { id: id.into(), title: title.into(), subtitle: subtitle.into(), action, available: true, reason: None }
    }

    fn unavailable(id: &str, title: impl Into<String>, subtitle: impl Into<String>, action: Action, reason: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), subtitle: subtitle.into(), action, available: false, reason: Some(reason.into()) }
    }

    fn invalid(id: &str, def: &CommandDef, reason: impl Into<String>) -> Self {
        Self::unavailable(id, def.name, def.usage, Action::Invalid, reason)
    }
}

fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() || *c == '\n').collect::<String>().trim().to_string()
}

fn find(word: &str) -> Option<&'static CommandDef> {
    let w = word.to_lowercase();
    REGISTRY.iter().find(|d| d.name == w || d.aliases.iter().any(|a| *a == w))
}

// ------------------------------------------------------------------ durations

/// Parse "10", "10m", "10 minutes", "1h30m", "1 hour 30 minutes", "1.5h", "90s", "1:30" (m:ss).
fn parse_duration_text(text: &str) -> Option<u64> {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    if let Some((m, s)) = t.split_once(':') {
        let (m, s): (u64, u64) = (m.trim().parse().ok()?, s.trim().parse().ok()?);
        return (s < 60).then_some(m * 60 + s).filter(|v| *v > 0 && *v <= MAX_TIMER_SECONDS);
    }
    let chars: Vec<char> = t.chars().collect();
    let (mut i, mut total, mut parts) = (0, 0f64, 0);
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        if start == i {
            return None;
        }
        let n: f64 = chars[start..i].iter().collect::<String>().parse().ok()?;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let ustart = i;
        while i < chars.len() && chars[i].is_alphabetic() {
            i += 1;
        }
        let unit: String = chars[ustart..i].iter().collect();
        let mult = match unit.as_str() {
            "" | "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
            "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
            _ => return None,
        };
        total += n * mult;
        parts += 1;
    }
    let secs = total.round();
    (parts > 0 && secs >= 1.0 && secs <= MAX_TIMER_SECONDS as f64).then_some(secs as u64)
}

fn is_unit_word(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "s" | "sec" | "secs" | "second" | "seconds" | "m" | "min" | "mins" | "minute" | "minutes" | "h" | "hr" | "hrs" | "hour" | "hours"
    )
}

/// Split "10 minutes tea break" into (600, "tea break"): the longest leading run of words that reads
/// as a duration, then the rest as the label.
pub fn parse_timer(args: &str) -> Result<(u64, String), String> {
    let words: Vec<&str> = args.split_whitespace().collect();
    if words.is_empty() {
        return Err("Give a duration, e.g. timer 10 minutes".into());
    }
    for n in (1..=words.len().min(6)).rev() {
        if let Some(secs) = parse_duration_text(&words[..n].join(" ")) {
            // "25 hours" must not become 25 minutes labelled "hours": a leftover unit word is an error.
            if words.get(n).is_some_and(|w| is_unit_word(w)) {
                continue;
            }
            let label: String = words[n..].join(" ").chars().take(60).collect();
            return Ok((secs, label));
        }
    }
    Err("I could not read that duration. Try 10 minutes, 1h30m or 90s (up to 24 hours).".into())
}

pub fn format_duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    match (h, m, s) {
        (0, 0, s) => format!("{s} sec"),
        (0, m, 0) => format!("{m} min"),
        (0, m, s) => format!("{m} min {s} sec"),
        (h, 0, 0) => format!("{h} h"),
        (h, m, _) => format!("{h} h {m} min"),
    }
}

// ------------------------------------------------------------------ web

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The only URL the launcher ever builds for the web: a fixed https host plus the percent-encoded query.
pub fn web_search_url(engine: WebEngine, query: &str) -> Option<String> {
    let q = clean(query);
    if q.is_empty() || q.chars().count() > 300 {
        return None;
    }
    let base = match engine {
        WebEngine::Duckduckgo => "https://duckduckgo.com/?q=",
        WebEngine::Google => "https://www.google.com/search?q=",
        WebEngine::Bing => "https://www.bing.com/search?q=",
    };
    Some(format!("{base}{}", percent_encode(&q.replace('\n', " "))))
}

// ------------------------------------------------------------------ AI

/// The instruction sent to the user's chosen provider for a launcher AI command.
pub fn ai_prompt(kind: AiKind, text: &str, language: Option<&str>) -> String {
    match kind {
        AiKind::Ask => text.to_string(),
        AiKind::Explain => format!("Explain the following text clearly and briefly:\n\n{text}"),
        AiKind::Summarize => format!("Summarise the following text in a few short sentences:\n\n{text}"),
        AiKind::Rewrite => format!("Rewrite the following text so it is clearer and better written. Keep the meaning. Reply with only the rewritten text:\n\n{text}"),
        AiKind::Translate => format!("Translate the following text into {}. Reply with only the translation:\n\n{text}", language.unwrap_or("English")),
    }
}

/// "hello to French" -> ("hello", Some("French")); otherwise the whole text, no language.
fn split_language(text: &str) -> (String, Option<String>) {
    let lower = text.to_lowercase();
    if let Some(i) = lower.rfind(" to ").or_else(|| lower.rfind(" into ")) {
        let sep = if lower[i..].starts_with(" into ") { 6 } else { 4 };
        let lang = text[i + sep..].trim();
        let body = text[..i].trim();
        if !body.is_empty() && !lang.is_empty() && lang.chars().count() <= 24 && lang.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '-') {
            return (body.to_string(), Some(lang.to_string()));
        }
    }
    (text.to_string(), None)
}

// ------------------------------------------------------------------ resolving input

fn looks_like_math(s: &str) -> bool {
    let t = s.trim();
    let starts = t.chars().next().is_some_and(|c| c.is_ascii_digit() || matches!(c, '(' | '-' | '.' | '+')) || t.to_lowercase().starts_with("sqrt(") || t.to_lowercase().starts_with("pi");
    starts && t.chars().any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '×' | '÷' | '(')) && t.chars().any(|c| c.is_ascii_digit() || c.is_alphabetic())
}

fn calc_match(expr: &str) -> CommandMatch {
    match calc::evaluate(expr) {
        Ok(v) => {
            let result = calc::format_number(v);
            CommandMatch::ok("calc", result.clone(), format!("{} =", expr.trim()), Action::Calc { expression: expr.trim().to_string(), result })
        }
        Err(e) => CommandMatch::unavailable("calc", "Calculator", expr.trim().to_string(), Action::Invalid, e.to_string()),
    }
}

fn open_matches(arg: &str) -> Vec<CommandMatch> {
    let a = arg.trim().to_lowercase();
    let mut out = Vec::new();
    for (folder, name) in FOLDERS {
        if a.is_empty() || name.starts_with(&a) {
            out.push(CommandMatch::ok("open", format!("Open {name}"), "Folder", Action::OpenFolder { folder }));
        }
    }
    if a.is_empty() || "settings".starts_with(&a) || "vex settings".starts_with(&a) {
        out.push(CommandMatch::ok("open", "Open VEX settings", "VEX", Action::OpenVexSettings));
    }
    if a.is_empty() || "windows settings".starts_with(&a) || "system settings".starts_with(&a) {
        out.push(CommandMatch::ok("open", "Open Windows settings", "Windows", Action::OpenWindowsSettings));
    }
    if out.is_empty() {
        let def = find("open").expect("open is registered");
        out.push(CommandMatch::invalid("open", def, "Unknown place. Try downloads, documents, desktop, pictures, videos, music or settings."));
    }
    out
}

/// Turn the typed text into command results. Empty when the text is not a command at all.
pub fn resolve(input: &str, features: Features) -> Vec<CommandMatch> {
    let input = clean(input);
    if input.is_empty() {
        return Vec::new();
    }
    let (word, rest) = match input.split_once(char::is_whitespace) {
        Some((w, r)) => (w, r.trim()),
        None => (input.as_str(), ""),
    };
    let word_lc = word.to_lowercase();

    // "=5+5"
    if let Some(expr) = input.strip_prefix('=') {
        return vec![calc_match(expr)];
    }

    if let Some(def) = find(&word_lc) {
        return match def.id {
            "calc" => {
                if rest.is_empty() {
                    vec![CommandMatch::invalid("calc", def, "Type a calculation, e.g. calc 245 * 18")]
                } else {
                    vec![calc_match(rest)]
                }
            }
            "time" if rest.is_empty() => vec![CommandMatch::ok("time", "Current time", "Copy to clipboard", Action::Time)],
            "date" if rest.is_empty() => vec![CommandMatch::ok("date", "Today's date", "Copy to clipboard", Action::Date)],
            "time" | "date" => Vec::new(),
            "open" => open_matches(rest),
            "search" => {
                if !features.web {
                    return Vec::new();
                }
                if rest.is_empty() {
                    vec![CommandMatch::invalid("search", def, "Type what to search for, e.g. search cybersecurity")]
                } else if rest.chars().count() > 300 {
                    vec![CommandMatch::invalid("search", def, "That search is too long.")]
                } else {
                    vec![CommandMatch::ok("search", format!("Search the web for \"{rest}\""), "Opens your default browser", Action::WebSearch { query: rest.to_string() })]
                }
            }
            "timer" => match parse_timer(rest) {
                Ok((seconds, label)) => {
                    let title = if label.is_empty() { format!("Timer: {}", format_duration(seconds)) } else { format!("Timer: {} ({label})", format_duration(seconds)) };
                    let action = Action::Timer { seconds, label };
                    if features.timers {
                        vec![CommandMatch::ok("timer", title, "Start countdown", action)]
                    } else {
                        vec![CommandMatch::unavailable("timer", title, "Countdown timer", action, "Timers arrive with the Widgets Hub and are not available yet.")]
                    }
                }
                Err(e) => vec![CommandMatch::invalid("timer", def, e)],
            },
            "note" => {
                let text = rest.chars().take(MAX_TEXT_CHARS).collect::<String>();
                if text.is_empty() {
                    return vec![CommandMatch::invalid("note", def, "Type the note, e.g. note buy groceries")];
                }
                let action = Action::Note { text: text.clone() };
                if features.notes {
                    vec![CommandMatch::ok("note", "Save note", text, action)]
                } else {
                    vec![CommandMatch::unavailable("note", "Save note", text, action, "Notes arrive with the Widgets Hub and are not available yet.")]
                }
            }
            "ask" | "explain" | "summarize" | "rewrite" | "translate" => {
                if !features.ai {
                    return Vec::new();
                }
                let kind = match def.id {
                    "ask" => AiKind::Ask,
                    "explain" => AiKind::Explain,
                    "summarize" => AiKind::Summarize,
                    "rewrite" => AiKind::Rewrite,
                    _ => AiKind::Translate,
                };
                if rest.is_empty() {
                    return vec![CommandMatch::invalid(def.id, def, "Type or paste the text after the command. VEX never reads your clipboard on its own.")];
                }
                if rest.chars().count() > MAX_TEXT_CHARS {
                    return vec![CommandMatch::invalid(def.id, def, format!("That text is too long (limit {MAX_TEXT_CHARS} characters)."))];
                }
                let (text, language) = if kind == AiKind::Translate { split_language(rest) } else { (rest.to_string(), None) };
                let title = match kind {
                    AiKind::Translate => format!("Translate into {}", language.as_deref().unwrap_or("English")),
                    _ => def.description.to_string(),
                };
                vec![CommandMatch::ok(def.id, title, "Press Enter to send this text to your AI provider", Action::Ai { kind, text, language })]
            }
            _ => Vec::new(),
        };
    }

    // bare arithmetic: "245 * 18"
    if looks_like_math(&input) {
        if let Ok(v) = calc::evaluate(&input) {
            let result = calc::format_number(v);
            return vec![CommandMatch::ok("calc", result.clone(), format!("{input} ="), Action::Calc { expression: input.clone(), result })];
        }
    }

    // A partly typed command name suggests itself ("tim" -> time, timer).
    if rest.is_empty() && word_lc.chars().count() >= 2 {
        return REGISTRY
            .iter()
            .filter(|d| d.name.starts_with(&word_lc))
            .filter(|d| match d.id {
                "search" => features.web,
                "ask" | "explain" | "summarize" | "rewrite" | "translate" => features.ai,
                _ => true,
            })
            .map(|d| CommandMatch::ok(d.id, d.name, d.description, Action::Fill { text: format!("{} ", d.name) }))
            .collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Features = Features { web: true, ai: true, timers: true, notes: true };
    const NOW: Features = Features { web: true, ai: true, timers: false, notes: false };

    fn first(input: &str, f: Features) -> CommandMatch {
        resolve(input, f).into_iter().next().unwrap_or_else(|| panic!("no result for {input:?}"))
    }

    #[test]
    fn every_registered_command_is_unique_and_documented() {
        let mut names = std::collections::BTreeSet::new();
        for d in REGISTRY {
            assert!(!d.description.is_empty() && !d.usage.is_empty());
            assert!(names.insert(d.name), "duplicate command {}", d.name);
            for a in d.aliases {
                assert!(names.insert(a), "alias {a} collides");
            }
        }
    }

    #[test]
    fn calc_command_and_bare_expressions() {
        let m = first("calc 245 * 18", ALL);
        assert_eq!(m.action, Action::Calc { expression: "245 * 18".into(), result: "4410".into() });
        assert_eq!(first("245 * 18", ALL).title, "4410");
        assert_eq!(first("=2+2", ALL).title, "4");
        assert_eq!(first("(2+3)*4", ALL).title, "20");
        assert_eq!(first("= 10 / 4", ALL).title, "2.5");
    }

    #[test]
    fn calc_errors_are_shown_not_run() {
        let m = first("calc 1/0", ALL);
        assert!(!m.available);
        assert!(m.reason.unwrap().contains("divide"));
        assert!(!first("calc", ALL).available);
        assert!(!first("calc system('x')", ALL).available);
    }

    #[test]
    fn text_that_is_not_math_is_not_a_calculation() {
        assert!(resolve("2024 report", ALL).is_empty());
        assert!(resolve("notepad", ALL).iter().all(|m| m.id != "calc"));
        assert!(resolve("7zip", ALL).is_empty());
    }

    #[test]
    fn time_and_date() {
        assert_eq!(first("time", ALL).action, Action::Time);
        assert_eq!(first("DATE", ALL).action, Action::Date);
        assert!(resolve("time zone", ALL).is_empty());
    }

    #[test]
    fn open_targets_are_an_allowlist() {
        assert_eq!(first("open downloads", ALL).action, Action::OpenFolder { folder: KnownFolder::Downloads });
        assert_eq!(first("open settings", ALL).action, Action::OpenVexSettings);
        assert_eq!(first("open windows settings", ALL).action, Action::OpenWindowsSettings);
        let bad = first("open c:\\windows\\system32\\cmd.exe", ALL);
        assert!(!bad.available && bad.action == Action::Invalid);
        let all = resolve("open", ALL);
        assert_eq!(all.len(), 8);
        assert_eq!(resolve("open do", ALL).len(), 2);
    }

    #[test]
    fn web_search_only_when_enabled_and_encodes_the_query() {
        let m = first("search cybersecurity careers", ALL);
        assert_eq!(m.action, Action::WebSearch { query: "cybersecurity careers".into() });
        assert!(resolve("search cats", Features { web: false, ..ALL }).is_empty());
        assert!(!first("search", ALL).available);
        let url = web_search_url(WebEngine::Duckduckgo, "a&b=c d?é").unwrap();
        assert_eq!(url, "https://duckduckgo.com/?q=a%26b%3Dc+d%3F%C3%A9");
        assert!(web_search_url(WebEngine::Google, "  ").is_none());
        assert!(web_search_url(WebEngine::Bing, &"x".repeat(400)).is_none());
        assert!(web_search_url(WebEngine::Google, "x").unwrap().starts_with("https://www.google.com/search?q="));
    }

    #[test]
    fn urls_cannot_be_hijacked_through_the_query() {
        for q in ["\" & calc.exe", "https://evil.example/x", "javascript:alert(1)", "a\r\nb"] {
            let u = web_search_url(WebEngine::Duckduckgo, q).unwrap();
            assert!(u.starts_with("https://duckduckgo.com/?q="));
            assert!(!u[26..].contains(['"', ' ', '&', '\r', '\n', ':', '/']), "{u}");
        }
    }

    #[test]
    fn durations() {
        assert_eq!(parse_timer("10 minutes"), Ok((600, String::new())));
        assert_eq!(parse_timer("10m"), Ok((600, String::new())));
        assert_eq!(parse_timer("10"), Ok((600, String::new())));
        assert_eq!(parse_timer("1h30m"), Ok((5400, String::new())));
        assert_eq!(parse_timer("1 hour 30 minutes"), Ok((5400, String::new())));
        assert_eq!(parse_timer("90s"), Ok((90, String::new())));
        assert_eq!(parse_timer("1.5h"), Ok((5400, String::new())));
        assert_eq!(parse_timer("1:30"), Ok((90, String::new())));
        assert_eq!(parse_timer("25 min pomodoro"), Ok((1500, "pomodoro".into())));
        assert_eq!(parse_timer("5m tea break"), Ok((300, "tea break".into())));
    }

    #[test]
    fn invalid_durations_are_rejected() {
        for bad in ["", "abc", "0", "0m", "-5", "25 hours", "1:99", "90 hours"] {
            assert!(parse_timer(bad).is_err(), "{bad:?} should fail");
        }
        assert!(parse_timer("24h").is_ok());
    }

    #[test]
    fn timers_and_notes_are_honest_about_not_existing_yet() {
        let t = first("timer 10 minutes", NOW);
        assert!(!t.available && t.reason.unwrap().contains("not available yet"));
        assert_eq!(t.action, Action::Timer { seconds: 600, label: String::new() });
        assert!(first("timer 10 minutes", ALL).available);
        let n = first("note buy groceries", NOW);
        assert!(!n.available);
        assert!(first("note buy groceries", ALL).available);
        assert!(!first("note", ALL).available);
    }

    #[test]
    fn ai_commands_need_the_setting_and_explicit_text() {
        let m = first("explain what is a VPN", ALL);
        assert_eq!(m.action, Action::Ai { kind: AiKind::Explain, text: "what is a VPN".into(), language: None });
        assert!(resolve("explain hello", Features { ai: false, ..ALL }).is_empty());
        let empty = first("summarize", ALL);
        assert!(!empty.available && empty.reason.unwrap().contains("clipboard"));
        let long = first(&format!("rewrite {}", "x".repeat(5000)), ALL);
        assert!(!long.available);
    }

    #[test]
    fn translate_extracts_the_language() {
        let m = first("translate good morning to French", ALL);
        assert_eq!(m.action, Action::Ai { kind: AiKind::Translate, text: "good morning".into(), language: Some("French".into()) });
        let m = first("translate hello", ALL);
        assert_eq!(m.action, Action::Ai { kind: AiKind::Translate, text: "hello".into(), language: None });
        let m = first("translate go to the shop", ALL);
        assert!(matches!(m.action, Action::Ai { language: Some(ref l), .. } if l == "the shop"));
    }

    #[test]
    fn prompts_are_built_from_the_text_only() {
        assert_eq!(ai_prompt(AiKind::Ask, "why?", None), "why?");
        assert!(ai_prompt(AiKind::Translate, "hola", Some("English")).contains("into English"));
        assert!(ai_prompt(AiKind::Summarize, "long text", None).ends_with("long text"));
    }

    #[test]
    fn partial_names_suggest_commands() {
        let s = resolve("tim", ALL);
        let names: Vec<_> = s.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(names, vec!["time", "timer"]);
        assert!(matches!(s[0].action, Action::Fill { .. }));
        assert!(resolve("sea", Features { web: false, ..ALL }).is_empty());
        assert!(resolve("t", ALL).is_empty(), "one letter is too vague");
    }

    #[test]
    fn control_characters_never_reach_actions() {
        let m = first("note hello\u{0}\u{7}world", ALL);
        assert_eq!(m.action, Action::Note { text: "helloworld".into() });
    }
}
