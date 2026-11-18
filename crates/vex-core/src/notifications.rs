//! Windows notifications in the island: parsing, filtering, de-duplication, history and the
//! engine loop. Everything here is platform-neutral and unit-tested; the only Windows code is
//! the `NotificationSource` implementation in the shell crate.
//!
//! Privacy rules enforced here (not in the UI):
//! * Nothing is written to disk. History lives in memory and ends with the app.
//! * With previews off, the notification text is dropped before it is stored.
//! * Notifications that were already in the Action Center when listening began are never read
//!   into history; only notifications that arrive afterwards are kept.
//! * Disabling the module clears everything it holds.
//! * Notification text is never given to the AI code; this module has no link to it.

use crate::settings::Notifications;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

pub const MAX_TITLE_CHARS: usize = 160;
pub const MAX_BODY_CHARS: usize = 400;
const MAX_APP_NAME_CHARS: usize = 80;
/// Windows ids remembered for de-duplication.
const SEEN_CAPACITY: usize = 1024;
/// The same app repeating the same text inside this window counts as one notification.
const REPEAT_WINDOW_MS: u64 = 3_000;
const MAX_KNOWN_APPS: usize = 300;
/// Safety net when the OS change event is missed.
pub const FALLBACK_POLL: Duration = Duration::from_secs(15);

// ------------------------------------------------------------------ source boundary

/// What the OS reports about one notification, already reduced to plain data.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawNotification {
    /// The OS notification id (unique per notification while it exists).
    pub os_id: u32,
    /// Application user model id (e.g. `Microsoft.WindowsStore_8wekyb3d8bbwe!App`).
    pub app_id: String,
    pub app_name: String,
    /// The text elements of the toast, in order: first is the title, the rest is the message.
    pub text: Vec<String>,
    pub created_ms: u64,
    /// `data:image/png;base64,…` of the app logo when the OS provides one.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Access {
    /// Not asked yet (or the module is off).
    Idle,
    Allowed,
    /// The user (or policy) refused in Windows settings.
    Denied,
    /// This build or install cannot read notifications; `reason` says why in plain words.
    Unavailable {
        reason: String,
    },
}

pub trait NotificationSource: Send {
    /// Ask Windows for permission (showing its prompt the first time).
    fn request_access(&mut self) -> Access;
    /// The notifications Windows currently holds.
    fn snapshot(&mut self) -> Result<Vec<RawNotification>, String>;
}

impl NotificationSource for Box<dyn NotificationSource> {
    fn request_access(&mut self) -> Access {
        (**self).request_access()
    }
    fn snapshot(&mut self) -> Result<Vec<RawNotification>, String> {
        (**self).snapshot()
    }
}

/// A source for platforms without Windows notifications.
pub struct UnavailableSource(pub String);

impl NotificationSource for UnavailableSource {
    fn request_access(&mut self) -> Access {
        Access::Unavailable {
            reason: self.0.clone(),
        }
    }
    fn snapshot(&mut self) -> Result<Vec<RawNotification>, String> {
        Err(self.0.clone())
    }
}

// ------------------------------------------------------------------ parsing

/// Remove control characters, collapse whitespace and cap the length (adds "…" when cut).
pub fn clean_text(s: &str, max_chars: usize) -> String {
    let collapsed = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let mut t: String = collapsed
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect();
        t.push('…');
        t
    }
}

/// Toast text elements → (title, message). Empty elements are skipped, so a toast whose first
/// element is blank still gets its second line as the title.
pub fn parse_text(elements: &[String]) -> (String, String) {
    let mut parts = elements
        .iter()
        .map(|e| clean_text(e, MAX_BODY_CHARS))
        .filter(|e| !e.is_empty());
    let title = parts.next().unwrap_or_default();
    let body = parts.collect::<Vec<_>>().join("\n");
    (
        clean_text(&title, MAX_TITLE_CHARS),
        clean_text_keep_lines(&body, MAX_BODY_CHARS),
    )
}

fn clean_text_keep_lines(s: &str, max: usize) -> String {
    let joined = s
        .split('\n')
        .map(|l| clean_text(l, max))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if joined.chars().count() <= max {
        joined
    } else {
        let mut t: String = joined.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// A readable name when the OS gives none: the part of an AUMID before `!`, or the exe name.
pub fn fallback_app_name(app_id: &str) -> String {
    let base = app_id.split('!').next().unwrap_or(app_id);
    let base = base.rsplit(['\\', '/']).next().unwrap_or(base);
    let base = base.strip_suffix(".exe").unwrap_or(base);
    let base = base.split('_').next().unwrap_or(base);
    let name = base.rsplit('.').next().unwrap_or(base);
    let name = clean_text(name, MAX_APP_NAME_CHARS);
    if name.is_empty() {
        "Unknown app".into()
    } else {
        name
    }
}

/// Only plain AUMID characters may reach the shell launch (see `launch_target`).
pub fn is_safe_app_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '.' | '_' | '!' | '-' | '\\' | ' ' | '{' | '}' | '(' | ')'
                )
        })
}

/// What to hand to `explorer.exe` to open the app that posted a notification.
pub fn launch_target(app_id: &str) -> Option<String> {
    is_safe_app_id(app_id).then(|| format!("shell:AppsFolder\\{app_id}"))
}

// ------------------------------------------------------------------ data

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Our own id (monotonic); the OS id is not unique over time.
    pub id: u64,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub ts_ms: u64,
    /// Not yet dismissed. Unread entries drive the island badge and the compact preview.
    pub unread: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub access: Access,
    /// Newest first.
    pub entries: Vec<Entry>,
    /// Apps seen so far (for icons and the per-app settings list).
    pub apps: Vec<AppInfo>,
    pub unread: usize,
}

// ------------------------------------------------------------------ the centre

pub struct Center {
    cfg: Notifications,
    access: Access,
    /// Oldest first.
    entries: VecDeque<Entry>,
    seen: HashSet<(String, u32)>,
    seen_order: VecDeque<(String, u32)>,
    apps: BTreeMap<String, (String, Option<String>)>,
    /// False until the first snapshot after enabling has been absorbed without keeping anything.
    baselined: bool,
    next_id: u64,
}

impl Center {
    pub fn new(cfg: Notifications) -> Self {
        Self {
            cfg,
            access: Access::Idle,
            entries: VecDeque::new(),
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
            apps: BTreeMap::new(),
            baselined: false,
            next_id: 1,
        }
    }

    pub fn config(&self) -> &Notifications {
        &self.cfg
    }

    pub fn set_access(&mut self, a: Access) {
        self.access = a;
    }

    pub fn access(&self) -> &Access {
        &self.access
    }

    /// Apply new settings. Turning the module off forgets everything; a lower limit trims at once;
    /// hiding previews strips the text of what is already stored.
    pub fn set_config(&mut self, cfg: Notifications) {
        let was_enabled = self.cfg.enabled;
        let had_previews = self.cfg.show_previews;
        self.cfg = cfg;
        if was_enabled && !self.cfg.enabled {
            self.forget_all();
        }
        if !was_enabled && self.cfg.enabled {
            self.baselined = false;
        }
        if had_previews && !self.cfg.show_previews {
            for e in &mut self.entries {
                e.title.clear();
                e.body.clear();
            }
        }
        let blocked = &self.cfg.blocked_apps;
        self.entries.retain(|e| !blocked.contains(&e.app_id));
        self.trim();
    }

    fn forget_all(&mut self) {
        self.entries.clear();
        self.seen.clear();
        self.seen_order.clear();
        self.apps.clear();
        self.baselined = false;
        self.access = Access::Idle;
    }

    fn mark_seen(&mut self, key: (String, u32)) {
        if self.seen.insert(key.clone()) {
            self.seen_order.push_back(key);
            while self.seen_order.len() > SEEN_CAPACITY {
                if let Some(old) = self.seen_order.pop_front() {
                    self.seen.remove(&old);
                }
            }
        }
    }

    fn trim(&mut self) {
        let limit = self.cfg.history_limit as usize;
        while self.entries.len() > limit {
            self.entries.pop_front();
        }
    }

    /// Take one snapshot from the OS. Returns the ids of entries that are new.
    /// The first snapshot after enabling only records what already exists (nothing is kept).
    pub fn ingest(&mut self, raws: Vec<RawNotification>, now_ms: u64) -> Vec<u64> {
        if !self.cfg.enabled {
            return vec![];
        }
        let baseline = !self.baselined;
        self.baselined = true;
        let mut added = Vec::new();

        for raw in raws {
            let key = (raw.app_id.clone(), raw.os_id);
            if self.seen.contains(&key) {
                continue;
            }
            self.mark_seen(key);
            if baseline || raw.app_id.trim().is_empty() {
                continue;
            }
            if self.cfg.blocked_apps.contains(&raw.app_id) {
                continue;
            }

            let name = {
                let n = clean_text(&raw.app_name, MAX_APP_NAME_CHARS);
                if n.is_empty() {
                    fallback_app_name(&raw.app_id)
                } else {
                    n
                }
            };
            if self.apps.len() < MAX_KNOWN_APPS || self.apps.contains_key(&raw.app_id) {
                let slot = self
                    .apps
                    .entry(raw.app_id.clone())
                    .or_insert((name.clone(), None));
                slot.0 = name.clone();
                if raw.icon.is_some() {
                    slot.1 = raw.icon.clone();
                }
            }

            let (title, body) = if self.cfg.show_previews {
                parse_text(&raw.text)
            } else {
                (String::new(), String::new())
            };
            let ts = if raw.created_ms > 0 {
                raw.created_ms
            } else {
                now_ms
            };

            // Same app repeating the same text moments apart: one notification, not two.
            let repeat = self.entries.iter().rev().take(20).any(|e| {
                e.app_id == raw.app_id
                    && e.title == title
                    && e.body == body
                    && ts.abs_diff(e.ts_ms) <= REPEAT_WINDOW_MS
            });
            if repeat {
                continue;
            }

            let id = self.next_id;
            self.next_id += 1;
            self.entries.push_back(Entry {
                id,
                app_id: raw.app_id,
                app_name: name,
                title,
                body,
                ts_ms: ts,
                unread: true,
            });
            added.push(id);
        }
        self.trim();
        // The trim may have dropped some of the ids we are about to report.
        added.retain(|id| self.entries.iter().any(|e| e.id == *id));
        added
    }

    /// Hide an entry from the island (mark it read). It stays in the history.
    pub fn dismiss(&mut self, id: u64) -> bool {
        self.entries
            .iter_mut()
            .find(|e| e.id == id)
            .map(|e| e.unread = false)
            .is_some()
    }

    pub fn dismiss_all(&mut self) {
        for e in &mut self.entries {
            e.unread = false;
        }
    }

    /// Remove one entry from the history.
    pub fn clear(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    /// Empty the history. The remembered OS ids stay, so what Windows still holds is not re-added.
    pub fn clear_all(&mut self) {
        self.entries.clear();
    }

    pub fn app_id_of(&self, id: u64) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.app_id.as_str())
    }

    pub fn view(&self) -> View {
        let entries: Vec<Entry> = self.entries.iter().rev().cloned().collect();
        let unread = entries.iter().filter(|e| e.unread).count();
        let apps = self
            .apps
            .iter()
            .map(|(id, (name, icon))| AppInfo {
                id: id.clone(),
                name: name.clone(),
                icon: icon.clone(),
                blocked: false,
            })
            .chain(
                self.cfg
                    .blocked_apps
                    .iter()
                    .filter(|b| !self.apps.contains_key(*b))
                    .map(|id| AppInfo {
                        id: id.clone(),
                        name: fallback_app_name(id),
                        icon: None,
                        blocked: true,
                    }),
            )
            .map(|mut a| {
                a.blocked = self.cfg.blocked_apps.contains(&a.id);
                a
            })
            .collect();
        View {
            access: self.access.clone(),
            entries,
            apps,
            unread,
        }
    }
}

// ------------------------------------------------------------------ engine loop

pub enum Msg {
    /// The OS reported a change (or a timer fired): read the current notifications.
    Changed,
    /// New settings from the user.
    Config(Notifications),
    Dismiss(u64),
    DismissAll,
    Clear(u64),
    ClearAll,
    Stop,
}

/// Runs until `Msg::Stop` or the channel closes. `on_view` is called after every change.
/// `on_new` is told which entries just arrived (the island shows a preview for them).
pub fn run_loop<S: NotificationSource>(
    mut source: S,
    initial: Notifications,
    rx: Receiver<Msg>,
    mut on_view: impl FnMut(&View),
    mut on_new: impl FnMut(&[u64]),
    now_ms: impl Fn() -> u64,
) {
    let mut center = Center::new(initial);
    let mut asked = false;

    // Enabled at start-up: ask for access right away.
    let ensure_access = |center: &mut Center, source: &mut S, asked: &mut bool| {
        if center.config().enabled && !*asked {
            *asked = true;
            let a = source.request_access();
            center.set_access(a);
        }
    };
    ensure_access(&mut center, &mut source, &mut asked);
    on_view(&center.view());

    let sync = |center: &mut Center, source: &mut S, on_new: &mut dyn FnMut(&[u64])| -> bool {
        if !center.config().enabled || *center.access() != Access::Allowed {
            return false;
        }
        match source.snapshot() {
            Ok(raws) => {
                let added = center.ingest(raws, now_ms());
                if !added.is_empty() {
                    on_new(&added);
                }
                true
            }
            Err(reason) => {
                center.set_access(Access::Unavailable { reason });
                true
            }
        }
    };

    if sync(&mut center, &mut source, &mut on_new) {
        on_view(&center.view());
    }

    loop {
        let msg = match rx.recv_timeout(FALLBACK_POLL) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => Msg::Changed,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut changed = true;
        match msg {
            Msg::Stop => return,
            Msg::Changed => {
                changed = sync(&mut center, &mut source, &mut on_new);
            }
            Msg::Config(cfg) => {
                let was = center.config().enabled;
                center.set_config(cfg);
                if center.config().enabled && !was {
                    asked = false;
                    ensure_access(&mut center, &mut source, &mut asked);
                    sync(&mut center, &mut source, &mut on_new);
                }
            }
            Msg::Dismiss(id) => {
                center.dismiss(id);
            }
            Msg::DismissAll => center.dismiss_all(),
            Msg::Clear(id) => {
                center.clear(id);
            }
            Msg::ClearAll => center.clear_all(),
        }
        if changed {
            on_view(&center.view());
        }
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};

    fn cfg() -> Notifications {
        Notifications {
            enabled: true,
            ..Notifications::default()
        }
    }

    fn raw(os_id: u32, app: &str, title: &str, body: &str, at: u64) -> RawNotification {
        RawNotification {
            os_id,
            app_id: app.into(),
            app_name: format!("{app} app"),
            text: vec![title.into(), body.into()],
            created_ms: at,
            icon: None,
        }
    }

    /// A centre that has absorbed its baseline snapshot, so later ingests are kept.
    fn ready(c: Notifications) -> Center {
        let mut center = Center::new(c);
        center.set_access(Access::Allowed);
        center.ingest(vec![], 0);
        center
    }

    // ---- parsing
    #[test]
    fn parse_takes_title_then_message() {
        let (t, b) = parse_text(&["New mail".into(), "From Ana".into(), "Lunch?".into()]);
        assert_eq!(t, "New mail");
        assert_eq!(b, "From Ana\nLunch?");
    }

    #[test]
    fn parse_skips_blank_elements_and_handles_empty() {
        assert_eq!(
            parse_text(&["".into(), "  ".into(), "Hello".into()]),
            ("Hello".into(), "".into())
        );
        assert_eq!(parse_text(&[]), (String::new(), String::new()));
        assert_eq!(
            parse_text(&["\u{0}\u{7}".into()]),
            (String::new(), String::new())
        );
    }

    #[test]
    fn parse_strips_controls_collapses_space_and_caps_length() {
        let (t, _) = parse_text(&["a\tb\r\n  c\u{0}d".into()]);
        assert_eq!(t, "a b c d");
        let long = "x".repeat(1000);
        let (t, b) = parse_text(&[long.clone(), long]);
        assert_eq!(t.chars().count(), MAX_TITLE_CHARS);
        assert!(t.ends_with('…'));
        assert_eq!(b.chars().count(), MAX_BODY_CHARS);
    }

    #[test]
    fn app_name_fallbacks_are_readable() {
        assert_eq!(
            fallback_app_name("Microsoft.WindowsStore_8wekyb3d8bbwe!App"),
            "WindowsStore"
        );
        assert_eq!(fallback_app_name("C:\\Program Files\\Foo\\bar.exe"), "bar");
        assert_eq!(fallback_app_name(""), "Unknown app");
        assert_eq!(fallback_app_name("Spotify.exe"), "Spotify");
    }

    #[test]
    fn launch_target_only_accepts_plain_app_ids() {
        assert_eq!(
            launch_target("Microsoft.Teams_8wekyb3d8bbwe!MSTeams").as_deref(),
            Some("shell:AppsFolder\\Microsoft.Teams_8wekyb3d8bbwe!MSTeams")
        );
        assert!(launch_target("evil\" & calc").is_none());
        assert!(launch_target("a;b").is_none());
        assert!(launch_target("").is_none());
        assert!(launch_target(&"a".repeat(300)).is_none());
    }

    // ---- baseline, dedupe
    #[test]
    fn existing_notifications_are_not_read_into_history() {
        let mut c = Center::new(cfg());
        c.set_access(Access::Allowed);
        let added = c.ingest(
            vec![raw(1, "a", "old", "x", 10), raw(2, "a", "older", "y", 11)],
            100,
        );
        assert!(added.is_empty());
        assert!(c.view().entries.is_empty());
        // ...and they are not picked up on the next snapshot either.
        assert!(c.ingest(vec![raw(1, "a", "old", "x", 10)], 200).is_empty());
        assert_eq!(c.ingest(vec![raw(3, "a", "new", "z", 300)], 300).len(), 1);
    }

    #[test]
    fn the_same_os_id_is_added_once() {
        let mut c = ready(cfg());
        assert_eq!(
            c.ingest(vec![raw(7, "a", "Hi", "there", 1000)], 1000).len(),
            1
        );
        assert!(c
            .ingest(vec![raw(7, "a", "Hi", "there", 1000)], 2000)
            .is_empty());
        assert_eq!(c.view().entries.len(), 1);
    }

    #[test]
    fn identical_text_moments_apart_is_one_notification_but_later_is_new() {
        let mut c = ready(cfg());
        c.ingest(vec![raw(1, "a", "Ping", "x", 10_000)], 10_000);
        assert!(c
            .ingest(vec![raw(2, "a", "Ping", "x", 11_000)], 11_000)
            .is_empty());
        assert_eq!(
            c.ingest(vec![raw(3, "a", "Ping", "x", 20_000)], 20_000)
                .len(),
            1
        );
        // Different app, same text: separate.
        assert_eq!(
            c.ingest(vec![raw(4, "b", "Ping", "x", 20_500)], 20_500)
                .len(),
            1
        );
    }

    #[test]
    fn empty_content_is_kept_and_missing_name_falls_back() {
        let mut c = ready(cfg());
        let mut r = raw(1, "Contoso.Chat_abc!App", "", "", 5_000);
        r.app_name = String::new();
        r.text = vec![];
        assert_eq!(c.ingest(vec![r], 5_000).len(), 1);
        let v = c.view();
        assert_eq!(v.entries[0].app_name, "Chat");
        assert_eq!(v.entries[0].title, "");
    }

    #[test]
    fn missing_timestamp_uses_arrival_time_and_blank_app_id_is_ignored() {
        let mut c = ready(cfg());
        assert_eq!(
            c.ingest(
                vec![raw(1, "a", "t", "b", 0), raw(2, "  ", "t", "b", 5)],
                777
            )
            .len(),
            1
        );
        assert_eq!(c.view().entries[0].ts_ms, 777);
    }

    // ---- filtering
    #[test]
    fn blocked_apps_are_dropped_and_listed() {
        let mut c = ready(Notifications {
            blocked_apps: vec!["spam".into()],
            ..cfg()
        });
        assert!(c
            .ingest(vec![raw(1, "spam", "Buy", "now", 1000)], 1000)
            .is_empty());
        assert_eq!(
            c.ingest(vec![raw(2, "ok", "Hello", "", 1000)], 1000).len(),
            1
        );
        let v = c.view();
        assert_eq!(v.entries.len(), 1);
        assert!(v.apps.iter().any(|a| a.id == "spam" && a.blocked));
        assert!(v.apps.iter().any(|a| a.id == "ok" && !a.blocked));
    }

    #[test]
    fn blocking_an_app_removes_its_existing_entries() {
        let mut c = ready(cfg());
        c.ingest(vec![raw(1, "a", "x", "", 1), raw(2, "b", "y", "", 2)], 5);
        c.set_config(Notifications {
            blocked_apps: vec!["a".into()],
            ..cfg()
        });
        let v = c.view();
        assert_eq!(v.entries.len(), 1);
        assert_eq!(v.entries[0].app_id, "b");
    }

    #[test]
    fn disabled_centre_ignores_everything() {
        let mut c = Center::new(Notifications::default());
        assert!(c.ingest(vec![raw(1, "a", "x", "", 1)], 1).is_empty());
        assert!(c.view().entries.is_empty());
    }

    // ---- previews and privacy
    #[test]
    fn previews_off_never_stores_the_text() {
        let mut c = ready(Notifications {
            show_previews: false,
            ..cfg()
        });
        c.ingest(vec![raw(1, "a", "Secret title", "Secret body", 1000)], 1000);
        let json = serde_json::to_string(&c.view()).unwrap();
        assert!(!json.contains("Secret"));
        assert_eq!(c.view().entries.len(), 1);
    }

    #[test]
    fn turning_previews_off_scrubs_stored_text() {
        let mut c = ready(cfg());
        c.ingest(vec![raw(1, "a", "Secret title", "Secret body", 1000)], 1000);
        c.set_config(Notifications {
            show_previews: false,
            ..cfg()
        });
        let json = serde_json::to_string(&c.view()).unwrap();
        assert!(!json.contains("Secret"));
    }

    #[test]
    fn disabling_forgets_everything_and_reenabling_baselines_again() {
        let mut c = ready(cfg());
        c.ingest(vec![raw(1, "a", "x", "", 1000)], 1000);
        c.set_config(Notifications::default());
        let v = c.view();
        assert!(v.entries.is_empty() && v.apps.is_empty());
        assert_eq!(v.access, Access::Idle);
        c.set_config(cfg());
        c.set_access(Access::Allowed);
        // Whatever Windows holds at this moment is baseline, not history.
        assert!(c.ingest(vec![raw(1, "a", "x", "", 1000)], 2000).is_empty());
        assert!(c.view().entries.is_empty());
    }

    // ---- history management
    #[test]
    fn history_is_capped_newest_kept_and_limit_changes_trim_at_once() {
        let mut c = ready(Notifications {
            history_limit: 10,
            ..cfg()
        });
        for i in 0..15u32 {
            c.ingest(
                vec![raw(
                    i + 1,
                    "a",
                    &format!("n{i}"),
                    "",
                    10_000 + u64::from(i) * 10_000,
                )],
                0,
            );
        }
        let v = c.view();
        assert_eq!(v.entries.len(), 10);
        assert_eq!(v.entries[0].title, "n14"); // newest first
        assert_eq!(v.entries[9].title, "n5");
        c.set_config(Notifications {
            history_limit: 10,
            ..cfg()
        });
        assert_eq!(c.view().entries.len(), 10);
    }

    #[test]
    fn new_ids_reported_only_for_entries_that_survive() {
        let mut c = ready(Notifications {
            history_limit: 10,
            ..cfg()
        });
        let raws: Vec<_> = (0..12u32)
            .map(|i| {
                raw(
                    i + 1,
                    "a",
                    &format!("n{i}"),
                    "",
                    10_000 + u64::from(i) * 10_000,
                )
            })
            .collect();
        let added = c.ingest(raws, 0);
        assert_eq!(added.len(), 10);
    }

    #[test]
    fn dismiss_marks_read_but_keeps_history_and_clear_removes() {
        let mut c = ready(cfg());
        let ids = c.ingest(
            vec![raw(1, "a", "one", "", 1_000), raw(2, "a", "two", "", 9_000)],
            0,
        );
        assert_eq!(c.view().unread, 2);
        assert!(c.dismiss(ids[0]));
        let v = c.view();
        assert_eq!((v.unread, v.entries.len()), (1, 2));
        assert!(c.clear(ids[1]));
        assert_eq!(c.view().entries.len(), 1);
        assert!(!c.clear(ids[1]) && !c.dismiss(999));
        c.dismiss_all();
        assert_eq!(c.view().unread, 0);
    }

    #[test]
    fn clear_all_empties_history_without_resurrecting_old_notifications() {
        let mut c = ready(cfg());
        c.ingest(vec![raw(1, "a", "one", "", 1_000)], 0);
        c.clear_all();
        assert!(c.view().entries.is_empty());
        // Windows still reports id 1; it must not come back.
        assert!(c.ingest(vec![raw(1, "a", "one", "", 1_000)], 0).is_empty());
    }

    #[test]
    fn icons_are_kept_per_app_and_not_lost_by_later_notifications_without_one() {
        let mut c = ready(cfg());
        let mut r = raw(1, "a", "x", "", 1_000);
        r.icon = Some("data:image/png;base64,AAAA".into());
        c.ingest(vec![r], 0);
        c.ingest(vec![raw(2, "a", "y", "", 9_000)], 0);
        assert_eq!(
            c.view().apps[0].icon.as_deref(),
            Some("data:image/png;base64,AAAA")
        );
    }

    #[test]
    fn the_seen_set_is_bounded() {
        let mut c = ready(Notifications {
            history_limit: 10,
            ..cfg()
        });
        let raws: Vec<_> = (0..(SEEN_CAPACITY as u32 + 200))
            .map(|i| {
                raw(
                    i + 1,
                    "a",
                    &format!("n{i}"),
                    "",
                    u64::from(i) * 10_000 + 10_000,
                )
            })
            .collect();
        c.ingest(raws, 0);
        assert!(c.seen.len() <= SEEN_CAPACITY && c.seen_order.len() <= SEEN_CAPACITY);
    }

    // ---- engine loop with a fake source
    struct Fake {
        access: Access,
        snaps: Arc<Mutex<Vec<Vec<RawNotification>>>>,
        asked: Arc<Mutex<u32>>,
        fail: Arc<Mutex<Option<String>>>,
    }
    impl NotificationSource for Fake {
        fn request_access(&mut self) -> Access {
            *self.asked.lock().unwrap() += 1;
            self.access.clone()
        }
        fn snapshot(&mut self) -> Result<Vec<RawNotification>, String> {
            if let Some(e) = self.fail.lock().unwrap().clone() {
                return Err(e);
            }
            let mut s = self.snaps.lock().unwrap();
            Ok(if s.len() > 1 {
                s.remove(0)
            } else {
                s.first().cloned().unwrap_or_default()
            })
        }
    }

    type Views = Arc<Mutex<Vec<View>>>;

    #[allow(clippy::type_complexity)]
    fn spawn(
        access: Access,
        initial: Notifications,
        snaps: Vec<Vec<RawNotification>>,
    ) -> (
        mpsc::Sender<Msg>,
        Views,
        Arc<Mutex<Vec<u64>>>,
        Arc<Mutex<u32>>,
        Arc<Mutex<Option<String>>>,
        std::thread::JoinHandle<()>,
    ) {
        let (tx, rx) = mpsc::channel();
        let views: Views = Arc::new(Mutex::new(vec![]));
        let news = Arc::new(Mutex::new(vec![]));
        let asked = Arc::new(Mutex::new(0));
        let fail = Arc::new(Mutex::new(None));
        let src = Fake {
            access,
            snaps: Arc::new(Mutex::new(snaps)),
            asked: asked.clone(),
            fail: fail.clone(),
        };
        let (v2, n2) = (views.clone(), news.clone());
        let h = std::thread::spawn(move || {
            run_loop(
                src,
                initial,
                rx,
                move |v| v2.lock().unwrap().push(v.clone()),
                move |ids| n2.lock().unwrap().extend_from_slice(ids),
                || 50_000,
            )
        });
        (tx, views, news, asked, fail, h)
    }

    fn settle(tx: &mpsc::Sender<Msg>, h: std::thread::JoinHandle<()>) {
        tx.send(Msg::Stop).unwrap();
        h.join().unwrap();
    }

    #[test]
    fn engine_baselines_then_reports_new_notifications() {
        let snaps = vec![
            vec![raw(1, "a", "old", "", 1)],
            vec![
                raw(1, "a", "old", "", 1),
                raw(2, "a", "fresh", "hello", 40_000),
            ],
        ];
        let (tx, views, news, asked, _f, h) = spawn(Access::Allowed, cfg(), snaps);
        tx.send(Msg::Changed).unwrap();
        settle(&tx, h);
        assert_eq!(*asked.lock().unwrap(), 1);
        let last = views.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.entries.len(), 1);
        assert_eq!(last.entries[0].title, "fresh");
        assert_eq!(news.lock().unwrap().len(), 1);
    }

    #[test]
    fn engine_does_not_ask_windows_while_disabled_and_asks_once_when_enabled() {
        let (tx, views, _n, asked, _f, h) =
            spawn(Access::Allowed, Notifications::default(), vec![vec![]]);
        tx.send(Msg::Changed).unwrap();
        tx.send(Msg::Config(cfg())).unwrap();
        tx.send(Msg::Changed).unwrap();
        settle(&tx, h);
        assert_eq!(*asked.lock().unwrap(), 1);
        assert_eq!(
            views.lock().unwrap().last().unwrap().access,
            Access::Allowed
        );
    }

    #[test]
    fn engine_reports_denied_and_unavailable_honestly_and_never_reads() {
        let (tx, views, _n, _a, fail, h) =
            spawn(Access::Denied, cfg(), vec![vec![raw(1, "a", "x", "", 1)]]);
        *fail.lock().unwrap() = Some("must not be read".into());
        tx.send(Msg::Changed).unwrap();
        settle(&tx, h);
        assert_eq!(views.lock().unwrap().last().unwrap().access, Access::Denied);

        let (tx, views, _n, _a, _f, h) = spawn(
            Access::Unavailable {
                reason: "no identity".into(),
            },
            cfg(),
            vec![vec![]],
        );
        settle(&tx, h);
        assert_eq!(
            views.lock().unwrap().last().unwrap().access,
            Access::Unavailable {
                reason: "no identity".into()
            }
        );
    }

    #[test]
    fn a_failing_snapshot_becomes_an_unavailable_state_not_a_crash() {
        let (tx, views, _n, _a, fail, h) = spawn(Access::Allowed, cfg(), vec![vec![]]);
        *fail.lock().unwrap() = Some("Element not found".into());
        tx.send(Msg::Changed).unwrap();
        settle(&tx, h);
        assert_eq!(
            views.lock().unwrap().last().unwrap().access,
            Access::Unavailable {
                reason: "Element not found".into()
            }
        );
    }

    #[test]
    fn engine_dismiss_clear_and_clear_all_commands() {
        let snaps = vec![
            vec![],
            vec![
                raw(1, "a", "one", "", 40_000),
                raw(2, "a", "two", "", 49_000),
            ],
        ];
        let (tx, views, news, _a, _f, h) = spawn(Access::Allowed, cfg(), snaps);
        tx.send(Msg::Changed).unwrap();
        // Wait for the entries to exist before addressing them.
        for _ in 0..200 {
            if news.lock().unwrap().len() == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let ids = news.lock().unwrap().clone();
        tx.send(Msg::Dismiss(ids[0])).unwrap();
        tx.send(Msg::Clear(ids[1])).unwrap();
        tx.send(Msg::ClearAll).unwrap();
        settle(&tx, h);
        let vs = views.lock().unwrap();
        let after_dismiss = &vs[vs.len() - 3];
        assert_eq!(after_dismiss.unread, 1);
        assert_eq!(after_dismiss.entries.len(), 2);
        assert_eq!(vs[vs.len() - 2].entries.len(), 1);
        assert!(vs.last().unwrap().entries.is_empty());
    }

    #[test]
    fn view_serialises_for_the_ui() {
        let mut c = ready(cfg());
        c.ingest(vec![raw(1, "a", "Hello", "World", 1_000)], 0);
        let v = serde_json::to_value(c.view()).unwrap();
        assert_eq!(v["access"]["state"], "allowed");
        assert_eq!(v["entries"][0]["appName"], "a app");
        assert_eq!(v["entries"][0]["unread"], true);
        assert_eq!(v["unread"], 1);
    }

    /// Notification text must have no route to an AI provider: neither the assistant's TypeScript nor
    /// the Rust AI code may mention notifications at all.
    #[test]
    fn ai_code_has_no_path_to_notification_content() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = vec![
            root.join("src-tauri/src/ai.rs"),
            root.join("src-tauri/src/ai/bench.rs"),
            root.join("crates/vex-core/src/ai.rs"),
        ];
        for e in std::fs::read_dir(root.join("src/assistant")).unwrap() {
            files.push(e.unwrap().path());
        }
        assert!(files.len() > 5);
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap().to_lowercase();
            assert!(
                !text.contains("notification"),
                "{} mentions notifications",
                f.display()
            );
        }
    }
}
