//! Local chat history. Conversations live in one JSON file in the app's data
//! folder (written atomically, recovered if corrupt) and are only ever sent to
//! the chosen AI provider as the *current* conversation's recent turns.

use crate::ai::{ChatTurn, Role};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

pub const MAX_CONVERSATIONS: usize = 100;
pub const MAX_MESSAGES: usize = 400;
pub const MAX_CONTENT_CHARS: usize = 60_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredMessage {
    pub role: Role,
    pub content: String,
    pub ts_ms: u64,
    /// App context that was shared with this message (only when the user asked about it).
    pub context: Option<String>,
    /// Short human label for `context`, e.g. "Now playing".
    pub context_label: Option<String>,
    /// A reply that was stopped or cut off before it finished.
    pub partial: bool,
}

impl Default for StoredMessage {
    fn default() -> Self {
        Self { role: Role::User, content: String::new(), ts_ms: 0, context: None, context_label: None, partial: false }
    }
}

impl StoredMessage {
    pub fn user(content: &str, ts_ms: u64, context: Option<(String, String)>) -> Self {
        let (label, ctx) = match context {
            Some((l, c)) => (Some(l), Some(c)),
            None => (None, None),
        };
        Self { role: Role::User, content: content.to_string(), ts_ms, context: ctx, context_label: label, partial: false }
    }
    pub fn assistant(content: &str, ts_ms: u64, partial: bool) -> Self {
        Self { role: Role::Assistant, content: content.to_string(), ts_ms, context: None, context_label: None, partial }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub messages: Vec<StoredMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub updated_ms: u64,
    pub message_count: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct FileData {
    version: u32,
    conversations: Vec<Conversation>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChatError {
    NotFound,
}

pub struct ChatStore {
    path: PathBuf,
    data: FileData,
    counter: u64,
    /// True when an existing file could not be read and was moved aside.
    pub recovered_from_corrupt: bool,
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

fn make_title(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let t: String = one_line.chars().take(48).collect();
    if one_line.chars().count() > 48 {
        format!("{t}…")
    } else if t.is_empty() {
        "New chat".into()
    } else {
        t
    }
}

impl ChatStore {
    pub fn load(path: PathBuf) -> Self {
        let mut recovered = false;
        let data = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<FileData>(&text) {
                Ok(d) => d,
                Err(_) => {
                    recovered = true;
                    let mut aside = path.clone().into_os_string();
                    aside.push(".corrupt");
                    let _ = fs::rename(&path, PathBuf::from(aside));
                    FileData::default()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => FileData::default(),
            Err(_) => {
                recovered = true;
                FileData::default()
            }
        };
        Self { path, data, counter: 0, recovered_from_corrupt: recovered }
    }

    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let json = serde_json::to_string(&FileData { version: 1, conversations: self.data.conversations.clone() }).map_err(io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)
    }

    /// Most recently updated first.
    pub fn summaries(&self) -> Vec<Summary> {
        let mut v: Vec<Summary> = self
            .data
            .conversations
            .iter()
            .map(|c| Summary { id: c.id.clone(), title: c.title.clone(), updated_ms: c.updated_ms, message_count: c.messages.len() })
            .collect();
        v.sort_by_key(|s| std::cmp::Reverse(s.updated_ms));
        v
    }

    pub fn get(&self, id: &str) -> Option<&Conversation> {
        self.data.conversations.iter().find(|c| c.id == id)
    }

    pub fn create(&mut self, now_ms: u64) -> String {
        loop {
            self.counter += 1;
            let id = format!("c{now_ms:x}-{:x}", self.counter);
            if self.get(&id).is_none() {
                self.data.conversations.push(Conversation { id: id.clone(), title: "New chat".into(), created_ms: now_ms, updated_ms: now_ms, messages: vec![] });
                self.enforce_limits();
                return id;
            }
        }
    }

    pub fn append(&mut self, id: &str, mut msg: StoredMessage) -> Result<(), ChatError> {
        let c = self.data.conversations.iter_mut().find(|c| c.id == id).ok_or(ChatError::NotFound)?;
        msg.content = truncate_chars(&msg.content, MAX_CONTENT_CHARS);
        if c.messages.is_empty() && msg.role == Role::User {
            c.title = make_title(&msg.content);
        }
        c.updated_ms = c.updated_ms.max(msg.ts_ms);
        c.messages.push(msg);
        if c.messages.len() > MAX_MESSAGES {
            let extra = c.messages.len() - MAX_MESSAGES;
            c.messages.drain(..extra);
        }
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.data.conversations.len();
        self.data.conversations.retain(|c| c.id != id);
        self.data.conversations.len() != before
    }

    pub fn clear_all(&mut self) {
        self.data.conversations.clear();
    }

    fn enforce_limits(&mut self) {
        if self.data.conversations.len() > MAX_CONVERSATIONS {
            self.data.conversations.sort_by_key(|c| c.updated_ms);
            let extra = self.data.conversations.len() - MAX_CONVERSATIONS;
            self.data.conversations.drain(..extra);
        }
    }

    /// The conversation as model turns, with any shared app context folded into its message.
    pub fn turns(&self, id: &str) -> Vec<ChatTurn> {
        self.get(id)
            .map(|c| {
                c.messages
                    .iter()
                    .map(|m| match m.role {
                        Role::User => ChatTurn::user(crate::ai::compose_user_content(&m.content, m.context.as_deref())),
                        Role::Assistant => ChatTurn::assistant(m.content.clone()),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, ChatStore) {
        let d = tempfile::tempdir().unwrap();
        let s = ChatStore::load(d.path().join("chats.json"));
        (d, s)
    }

    #[test]
    fn conversations_survive_a_restart() {
        let (d, mut s) = store();
        let id = s.create(1_000);
        s.append(&id, StoredMessage::user("hello there", 1_001, None)).unwrap();
        s.append(&id, StoredMessage::assistant("hi!", 1_002, false)).unwrap();
        s.save().unwrap();

        let s2 = ChatStore::load(d.path().join("chats.json"));
        let c = s2.get(&id).expect("conversation restored");
        assert_eq!(c.messages.len(), 2);
        assert_eq!(c.messages[1].content, "hi!");
        assert_eq!(c.title, "hello there");
        assert_eq!(c.created_ms, 1_000);
        assert_eq!(s2.summaries()[0].message_count, 2);
    }

    #[test]
    fn newest_conversation_is_listed_first() {
        let (_d, mut s) = store();
        let a = s.create(1);
        let b = s.create(2);
        s.append(&a, StoredMessage::user("old", 5, None)).unwrap();
        s.append(&b, StoredMessage::user("older?", 3, None)).unwrap();
        let ids: Vec<_> = s.summaries().into_iter().map(|x| x.id).collect();
        assert_eq!(ids, vec![a, b], "ordered by last activity");
    }

    #[test]
    fn ids_are_unique_even_in_the_same_millisecond() {
        let (_d, mut s) = store();
        let ids: std::collections::HashSet<_> = (0..20).map(|_| s.create(7)).collect();
        assert_eq!(ids.len(), 20);
    }

    #[test]
    fn delete_and_clear() {
        let (d, mut s) = store();
        let a = s.create(1);
        let b = s.create(2);
        assert!(s.delete(&a));
        assert!(!s.delete(&a));
        assert!(s.get(&b).is_some());
        s.clear_all();
        s.save().unwrap();
        assert!(ChatStore::load(d.path().join("chats.json")).summaries().is_empty());
    }

    #[test]
    fn long_titles_are_shortened_and_content_is_capped() {
        let (_d, mut s) = store();
        let id = s.create(1);
        s.append(&id, StoredMessage::user(&"word ".repeat(40), 2, None)).unwrap();
        assert!(s.get(&id).unwrap().title.ends_with('…') && s.get(&id).unwrap().title.chars().count() <= 49);
        s.append(&id, StoredMessage::assistant(&"x".repeat(MAX_CONTENT_CHARS + 10), 3, false)).unwrap();
        assert_eq!(s.get(&id).unwrap().messages[1].content.chars().count(), MAX_CONTENT_CHARS);
    }

    #[test]
    fn limits_drop_the_oldest() {
        let (_d, mut s) = store();
        for i in 0..(MAX_CONVERSATIONS as u64 + 5) {
            let id = s.create(i);
            s.append(&id, StoredMessage::user("x", i, None)).unwrap();
        }
        assert_eq!(s.summaries().len(), MAX_CONVERSATIONS);
        let id = s.create(10_000);
        for i in 0..(MAX_MESSAGES + 3) {
            s.append(&id, StoredMessage::user("m", 10_001 + i as u64, None)).unwrap();
        }
        assert_eq!(s.get(&id).unwrap().messages.len(), MAX_MESSAGES);
    }

    #[test]
    fn appending_to_a_missing_conversation_fails() {
        let (_d, mut s) = store();
        assert_eq!(s.append("nope", StoredMessage::user("x", 1, None)), Err(ChatError::NotFound));
    }

    #[test]
    fn corrupt_file_is_set_aside_not_fatal() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("chats.json");
        fs::write(&p, "{ definitely not json").unwrap();
        let s = ChatStore::load(p.clone());
        assert!(s.recovered_from_corrupt && s.summaries().is_empty());
        assert!(d.path().join("chats.json.corrupt").exists());
    }

    #[test]
    fn model_turns_include_context_only_for_the_message_that_carried_it() {
        let (_d, mut s) = store();
        let id = s.create(1);
        s.append(&id, StoredMessage::user("what song is this?", 2, Some(("Now playing".into(), "Song X by Y".into())))).unwrap();
        s.append(&id, StoredMessage::assistant("It's Song X.", 3, false)).unwrap();
        s.append(&id, StoredMessage::user("thanks", 4, None)).unwrap();
        let t = s.turns(&id);
        assert!(t[0].content.contains("Song X by Y"));
        assert_eq!(t[2].content, "thanks", "plain messages carry no context");
    }

    #[test]
    fn stored_format_is_camel_case_and_tolerates_missing_fields() {
        let m: StoredMessage = serde_json::from_str(r#"{"role":"user","content":"hi","tsMs":5}"#).unwrap();
        assert_eq!(m.ts_ms, 5);
        assert!(m.context.is_none() && !m.partial);
    }
}
