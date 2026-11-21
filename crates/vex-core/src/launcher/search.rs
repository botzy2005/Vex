//! Turns what the user typed into the rows the launcher shows, and checks the rows it is asked to run.
//!
//! Nothing here touches the disk or starts anything: it works on the cached app list, the cached file
//! index and the launcher state that the shell passes in, so it is fast (called on every debounced
//! keystroke) and fully unit-testable.
//!
//! The UI never decides what a row *does*. When the user presses Enter it sends back the row's
//! `Target` together with the text that produced it, and `validate` re-derives the allowed targets from
//! that text. A forged or stale target (an app id that is not in the cache, a path that is not in the
//! index, a command that the text does not resolve to) is refused.

use serde::{Deserialize, Serialize};

use super::{
    commands::{self, Action, AiKind, Features},
    files::{self, FileEntry, FileKind},
    rank,
    store::LauncherState,
    AppEntry, AppKind,
};
use crate::settings::Launcher;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Target {
    App { id: String },
    File { path: String },
    Command { action: Action },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    App,
    File,
    Folder,
    Command,
    Web,
    Ai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Stable within one result list (used for React keys and selection).
    pub key: String,
    /// Group heading: Pinned, Recent, Applications, Commands, Files, Folders, Web or AI.
    pub section: &'static str,
    pub category: Category,
    pub title: String,
    pub subtitle: String,
    pub target: Target,
    /// False when the row cannot run; `reason` says why.
    pub available: bool,
    pub reason: Option<String>,
    pub pinned: bool,
    pub app_kind: Option<AppKind>,
    pub file_kind: Option<FileKind>,
}

pub struct Inputs<'a> {
    pub settings: &'a Launcher,
    pub state: &'a LauncherState,
    pub apps: &'a [AppEntry],
    /// False until the first app discovery has finished (so empty results are not "uninstalled").
    pub apps_ready: bool,
    pub files: &'a [FileEntry],
    /// Which later modules exist (timers and notes arrive with the Widgets Hub).
    pub timers_ready: bool,
    pub notes_ready: bool,
}

impl Inputs<'_> {
    fn features(&self) -> Features {
        Features { web: self.settings.web, ai: self.settings.ai, timers: self.timers_ready, notes: self.notes_ready }
    }
}

fn app_subtitle(kind: AppKind) -> &'static str {
    match kind {
        AppKind::Desktop => "Application",
        AppKind::Store => "Microsoft Store app",
    }
}

fn app_row(section: &'static str, id: &str, name: &str, kind: AppKind, pinned: bool, installed: Option<bool>) -> Row {
    let missing = installed == Some(false);
    Row {
        key: format!("app:{id}"),
        section,
        category: Category::App,
        title: name.to_string(),
        subtitle: if missing { "No longer installed".into() } else { app_subtitle(kind).into() },
        target: Target::App { id: id.to_string() },
        available: !missing,
        reason: missing.then(|| "This app is no longer installed. Remove it from the list.".to_string()),
        pinned,
        app_kind: Some(kind),
        file_kind: None,
    }
}

fn file_row(e: &FileEntry) -> Row {
    let folder = e.kind == FileKind::Folder;
    Row {
        key: format!("file:{}", e.path),
        section: if folder { "Folders" } else { "Files" },
        category: if folder { Category::Folder } else { Category::File },
        title: e.name.clone(),
        subtitle: files::parent_of(&e.path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        target: Target::File { path: e.path.clone() },
        available: true,
        reason: None,
        pinned: false,
        app_kind: None,
        file_kind: Some(e.kind),
    }
}

fn command_row(i: usize, m: commands::CommandMatch) -> Row {
    let category = match m.action {
        Action::WebSearch { .. } => Category::Web,
        Action::Ai { .. } => Category::Ai,
        _ => Category::Command,
    };
    let section = match category {
        Category::Web => "Web",
        Category::Ai => "AI",
        _ => "Commands",
    };
    Row {
        key: format!("cmd:{i}:{}", m.id),
        section,
        category,
        title: m.title,
        subtitle: m.subtitle,
        target: Target::Command { action: m.action },
        available: m.available,
        reason: m.reason,
        pinned: false,
        app_kind: None,
        file_kind: None,
    }
}

fn web_fallback(query: &str) -> Row {
    Row {
        key: "web:fallback".into(),
        section: "Web",
        category: Category::Web,
        title: format!("Search the web for \u{201c}{query}\u{201d}"),
        subtitle: "Opens your default browser".into(),
        target: Target::Command { action: Action::WebSearch { query: query.to_string() } },
        available: true,
        reason: None,
        pinned: false,
        app_kind: None,
        file_kind: None,
    }
}

fn ai_fallback(query: &str) -> Row {
    Row {
        key: "ai:fallback".into(),
        section: "AI",
        category: Category::Ai,
        title: "Ask AI".into(),
        subtitle: query.to_string(),
        target: Target::Command { action: Action::Ai { kind: AiKind::Ask, text: query.to_string(), language: None } },
        available: true,
        reason: None,
        pinned: false,
        app_kind: None,
        file_kind: None,
    }
}

fn clean(q: &str) -> String {
    q.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}

/// Rows for the empty search box: pinned apps, then recent launches.
fn start_rows(i: &Inputs) -> Vec<Row> {
    let installed = |id: &str| i.apps_ready.then(|| i.apps.iter().any(|a| a.id == id));
    let mut rows: Vec<Row> = Vec::new();
    if i.settings.apps {
        for p in &i.state.pins {
            rows.push(app_row("Pinned", &p.id, &p.name, p.kind, true, installed(&p.id)));
        }
        if i.settings.remember_recents {
            for r in i.state.recents.iter().filter(|r| !i.state.is_pinned(&r.id)).take(6) {
                rows.push(app_row("Recent", &r.id, &r.name, r.kind, false, installed(&r.id)));
            }
        }
    }
    rows
}

/// Results for the typed text. `query` is used as typed (trimmed).
pub fn search(query: &str, i: &Inputs) -> Vec<Row> {
    let q = clean(query);
    let max = i.settings.max_results.clamp(3, 12) as usize;
    if q.is_empty() {
        return start_rows(i);
    }

    let commands: Vec<Row> = if i.settings.commands || i.settings.web || i.settings.ai {
        commands::resolve(&q, i.features())
            .into_iter()
            .filter(|m| match m.action {
                Action::WebSearch { .. } => i.settings.web,
                Action::Ai { .. } => i.settings.ai,
                _ => i.settings.commands,
            })
            .enumerate()
            .map(|(n, m)| command_row(n, m))
            .collect()
    } else {
        Vec::new()
    };
    // A typed command word (or a calculation) is what the user is after: show it first.
    // A bare command word ("note") still lets the app of that name come first.
    let first = q.chars().next().unwrap_or(' ');
    let commands_first = !commands.is_empty() && (q.contains(char::is_whitespace) || first == '=' || first.is_ascii_digit() || matches!(first, '(' | '-' | '.' | '+'));

    let mut apps: Vec<Row> = Vec::new();
    if i.settings.apps {
        let boost = |a: &AppEntry| -> u32 {
            let pin = if i.state.is_pinned(&a.id) { 150 } else { 0 };
            let recent = if i.settings.remember_recents {
                i.state.recents.iter().position(|r| r.id == a.id).map(|p| 100u32.saturating_sub(p as u32 * 5)).unwrap_or(0)
            } else {
                0
            };
            pin + recent
        };
        for r in rank::rank(i.apps, &q, |a| a.name.as_str(), boost, max) {
            let a = r.item;
            apps.push(app_row("Applications", &a.id, &a.name, a.kind, i.state.is_pinned(&a.id), None));
        }
    }

    let mut found_files: Vec<Row> = Vec::new();
    if i.settings.files && q.chars().count() >= 2 {
        let limit = (max / 2).max(3);
        found_files = files::search(i.files, &q, None, limit).into_iter().map(file_row).collect();
    }

    let mut main: Vec<Row> = Vec::new();
    if commands_first {
        main.extend(commands.into_iter().filter(|r| r.category == Category::Command));
        main.extend(apps);
    } else {
        main.extend(apps);
        main.extend(commands.iter().filter(|r| r.category == Category::Command).cloned());
    }
    main.extend(found_files);
    main.truncate(max);

    // Explicit web and AI rows come from the command word ("search x", "ask x") or as a last-resort fallback.
    let mut tail: Vec<Row> = commands_tail(&q, i);
    main.append(&mut tail);
    main
}

fn commands_tail(q: &str, i: &Inputs) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let explicit: Vec<Row> = commands::resolve(q, i.features())
        .into_iter()
        .filter(|m| match m.action {
            Action::WebSearch { .. } => i.settings.web,
            Action::Ai { .. } => i.settings.ai,
            _ => false,
        })
        .enumerate()
        .map(|(n, m)| command_row(100 + n, m))
        .collect();
    let has_web = explicit.iter().any(|r| r.category == Category::Web);
    let has_ai = explicit.iter().any(|r| r.category == Category::Ai);
    out.extend(explicit);
    if i.settings.web && !has_web {
        out.push(web_fallback(q));
    }
    if i.settings.ai && !has_ai && (q.ends_with('?') || q.split_whitespace().count() >= 3) {
        out.push(ai_fallback(q));
    }
    out
}

/// Whether `target` is something the text `input` is allowed to run. The shell calls this before it
/// acts on anything the UI sent.
pub fn validate(input: &str, target: &Target, i: &Inputs) -> Result<(), String> {
    match target {
        Target::App { id } => {
            if !i.settings.apps {
                return Err("Application search is switched off.".into());
            }
            if i.apps.iter().any(|a| &a.id == id) || i.state.pins.iter().any(|p| &p.id == id) || i.state.recents.iter().any(|r| &r.id == id) {
                Ok(())
            } else {
                Err("That app is not in the launcher's list.".into())
            }
        }
        Target::File { path } => {
            if !i.settings.files {
                return Err("File search is switched off.".into());
            }
            if i.files.iter().any(|f| &f.path == path) {
                Ok(())
            } else {
                Err("That file is not in the launcher's index.".into())
            }
        }
        Target::Command { action } => {
            let q = clean(input);
            match action {
                Action::WebSearch { query } => {
                    if !i.settings.web {
                        return Err("Web search is switched off.".into());
                    }
                    let typed_ok = clean(query) == q || commands::resolve(&q, i.features()).iter().any(|m| m.action == *action);
                    if typed_ok && !clean(query).is_empty() {
                        Ok(())
                    } else {
                        Err("The search text does not match what was typed.".into())
                    }
                }
                Action::Ai { kind, text, .. } => {
                    if !i.settings.ai {
                        return Err("AI commands are switched off.".into());
                    }
                    let resolved = commands::resolve(&q, i.features()).iter().any(|m| m.action == *action);
                    let fallback = *kind == AiKind::Ask && clean(text) == q;
                    if resolved || fallback {
                        Ok(())
                    } else {
                        Err("The AI request does not match what was typed.".into())
                    }
                }
                Action::Fill { .. } | Action::Invalid => Err("That entry cannot be run.".into()),
                other => {
                    if !i.settings.commands {
                        return Err("Quick commands are switched off.".into());
                    }
                    if commands::resolve(&q, i.features()).iter().any(|m| m.available && m.action == *other) {
                        Ok(())
                    } else {
                        Err("That command does not match what was typed.".into())
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::store::Remembered;

    fn app(id: &str, name: &str) -> AppEntry {
        AppEntry { id: id.into(), name: name.into(), kind: AppKind::Desktop }
    }

    fn file(name: &str, kind: FileKind) -> FileEntry {
        FileEntry { path: format!("C:\\Users\\me\\Documents\\{name}"), name: name.into(), kind, modified: 1 }
    }

    struct Fixture {
        settings: Launcher,
        state: LauncherState,
        apps: Vec<AppEntry>,
        files: Vec<FileEntry>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                settings: Launcher::default(),
                state: LauncherState::default(),
                apps: vec![app("a1", "Visual Studio Code"), app("a2", "Notepad"), app("a3", "Calculator"), app("a4", "Chrome")],
                files: vec![file("budget.xlsx", FileKind::Document), file("Projects", FileKind::Folder), file("holiday.png", FileKind::Image)],
            }
        }

        fn inputs(&self) -> Inputs<'_> {
            Inputs { settings: &self.settings, state: &self.state, apps: &self.apps, apps_ready: true, files: &self.files, timers_ready: false, notes_ready: false }
        }

        fn run(&self, q: &str) -> Vec<Row> {
            search(q, &self.inputs())
        }
    }

    #[test]
    fn empty_query_shows_pins_then_recents() {
        let mut f = Fixture::new();
        f.state.pin(Remembered { id: "a2".into(), name: "Notepad".into(), kind: AppKind::Desktop });
        f.state.record_launch(Remembered { id: "a4".into(), name: "Chrome".into(), kind: AppKind::Desktop }, 10);
        let rows = f.run("");
        assert_eq!(rows[0].section, "Pinned");
        assert!(rows[0].pinned);
        assert_eq!(rows[1].section, "Recent");
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn recents_respect_the_setting() {
        let mut f = Fixture::new();
        f.state.record_launch(Remembered { id: "a4".into(), name: "Chrome".into(), kind: AppKind::Desktop }, 10);
        f.settings.remember_recents = false;
        assert!(f.run("").is_empty());
    }

    #[test]
    fn uninstalled_pin_is_marked_unavailable() {
        let mut f = Fixture::new();
        f.state.pin(Remembered { id: "gone".into(), name: "Old Tool".into(), kind: AppKind::Desktop });
        let rows = f.run("");
        assert!(!rows[0].available);
        assert_eq!(rows[0].subtitle, "No longer installed");
        // before the first discovery finishes nothing is declared missing
        let mut inp = f.inputs();
        inp.apps_ready = false;
        assert!(search("", &inp)[0].available);
    }

    #[test]
    fn typing_finds_apps_and_files() {
        let f = Fixture::new();
        let rows = f.run("note");
        assert_eq!(rows[0].title, "Notepad");
        let rows = f.run("budget");
        assert!(rows.iter().any(|r| r.category == Category::File && r.title == "budget.xlsx"));
        let rows = f.run("proj");
        assert!(rows.iter().any(|r| r.category == Category::Folder && r.section == "Folders"));
    }

    #[test]
    fn pins_and_recents_boost_ranking() {
        let mut f = Fixture::new();
        f.apps.push(app("a5", "Calendar Pro Edition"));
        let plain = f.run("cal");
        assert_eq!(plain[0].title, "Calculator"); // shorter name wins at equal match
        f.state.pin(Remembered { id: "a5".into(), name: "Calendar Pro Edition".into(), kind: AppKind::Desktop });
        assert_eq!(f.run("cal")[0].title, "Calendar Pro Edition");
    }

    #[test]
    fn calculator_comes_first() {
        let f = Fixture::new();
        let rows = f.run("calc 245 * 18");
        assert_eq!(rows[0].category, Category::Command);
        assert_eq!(rows[0].title, "4410");
        let rows = f.run("2+2");
        assert_eq!(rows[0].title, "4");
    }

    #[test]
    fn category_switches_are_honoured() {
        let mut f = Fixture::new();
        f.settings.apps = false;
        assert!(f.run("note").iter().all(|r| r.category != Category::App));
        f.settings.files = false;
        assert!(f.run("budget").iter().all(|r| r.category != Category::File));
        f.settings.commands = false;
        assert!(f.run("calc 1+1").iter().all(|r| r.category != Category::Command));
        f.settings.web = false;
        assert!(f.run("zzz").iter().all(|r| r.category != Category::Web));
        f.settings.ai = false;
        assert!(f.run("what is a firewall?").iter().all(|r| r.category != Category::Ai));
    }

    #[test]
    fn web_row_is_a_fallback_and_never_adds_local_content() {
        let f = Fixture::new();
        let rows = f.run("cybersecurity");
        let web: Vec<_> = rows.iter().filter(|r| r.category == Category::Web).collect();
        assert_eq!(web.len(), 1);
        assert_eq!(web[0].target, Target::Command { action: Action::WebSearch { query: "cybersecurity".into() } });
        // explicit command: no duplicate fallback
        let rows = f.run("search cybersecurity");
        assert_eq!(rows.iter().filter(|r| r.category == Category::Web).count(), 1);
    }

    #[test]
    fn ai_row_only_for_question_like_text() {
        let f = Fixture::new();
        assert!(f.run("notepad").iter().all(|r| r.category != Category::Ai));
        assert!(f.run("what is a firewall").iter().any(|r| r.category == Category::Ai));
        assert!(f.run("firewall?").iter().any(|r| r.category == Category::Ai));
        assert!(f.run("explain how tcp works").iter().any(|r| r.category == Category::Ai));
    }

    #[test]
    fn results_respect_the_maximum() {
        let mut f = Fixture::new();
        f.apps = (0..40).map(|n| app(&format!("x{n}"), &format!("Tool {n}"))).collect();
        f.settings.max_results = 5;
        let rows = f.run("tool");
        let main = rows.iter().filter(|r| r.category == Category::App).count();
        assert_eq!(main, 5);
    }

    #[test]
    fn unavailable_commands_say_why() {
        let f = Fixture::new();
        let rows = f.run("timer 10 minutes");
        let t = rows.iter().find(|r| r.title.to_lowercase().contains("timer")).unwrap();
        assert!(!t.available);
        assert!(t.reason.is_some());
    }

    #[test]
    fn validation_accepts_real_rows_and_refuses_forged_ones() {
        let f = Fixture::new();
        let inp = f.inputs();
        for q in ["note", "calc 6*7", "budget", "cybersecurity", "what is a firewall?"] {
            for row in f.run(q) {
                if row.available {
                    assert!(validate(q, &row.target, &inp).is_ok(), "{q}: {:?}", row.target);
                }
            }
        }
        assert!(validate("x", &Target::App { id: "evil.exe".into() }, &inp).is_err());
        assert!(validate("x", &Target::File { path: "C:\\Windows\\System32\\cmd.exe".into() }, &inp).is_err());
        // a command that the text does not produce
        assert!(validate("hello", &Target::Command { action: Action::OpenWindowsSettings }, &inp).is_err());
        assert!(validate("hello", &Target::Command { action: Action::Invalid }, &inp).is_err());
        // web search text must be what was typed
        assert!(validate("hello", &Target::Command { action: Action::WebSearch { query: "my secret".into() } }, &inp).is_err());
        assert!(validate("hello", &Target::Command { action: Action::WebSearch { query: "hello".into() } }, &inp).is_ok());
        // AI text must be what was typed
        assert!(validate("hello there you", &Target::Command { action: Action::Ai { kind: AiKind::Ask, text: "clipboard contents".into(), language: None } }, &inp).is_err());
    }

    #[test]
    fn switched_off_categories_refuse_to_run() {
        let mut f = Fixture::new();
        f.settings.web = false;
        let q = "hello";
        let t = Target::Command { action: Action::WebSearch { query: q.into() } };
        assert!(validate(q, &t, &f.inputs()).is_err());
    }
}
