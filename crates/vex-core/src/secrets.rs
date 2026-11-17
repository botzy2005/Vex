//! Where API keys live. The core only defines the interface; the Windows shell
//! implements it on top of Windows Credential Manager, so keys are never written
//! to settings, chat history, logs or the browser's storage.

use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, String>;
    fn set(&self, name: &str, value: &str) -> Result<(), String>;
    fn delete(&self, name: &str) -> Result<(), String>;
}

/// Credential name for a provider's API key.
pub fn key_name(provider_id: &str) -> String {
    format!("ai-api-key:{provider_id}")
}

/// Keeps secrets in memory only. Used by tests, and by development builds on
/// operating systems other than Windows (keys then last only for the session).
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().map_err(|_| "secret store poisoned")?.get(name).cloned())
    }
    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().map_err(|_| "secret store poisoned")?.insert(name.into(), value.into());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        self.0.lock().map_err(|_| "secret store poisoned")?.remove(name);
        Ok(())
    }
}

/// Validate and normalise a key typed by the user.
pub fn clean_key(raw: &str) -> Result<String, String> {
    let k = raw.trim();
    if k.is_empty() {
        return Err("The API key is empty.".into());
    }
    if k.len() > 500 {
        return Err("That doesn't look like an API key (too long).".into());
    }
    if k.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("API keys can't contain spaces or line breaks. Paste just the key.".into());
    }
    Ok(k.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let s = MemoryStore::default();
        assert_eq!(s.get("a").unwrap(), None);
        s.set("a", "secret").unwrap();
        assert_eq!(s.get("a").unwrap().as_deref(), Some("secret"));
        s.delete("a").unwrap();
        assert_eq!(s.get("a").unwrap(), None);
        s.delete("a").unwrap(); // deleting a missing secret is not an error
    }

    #[test]
    fn keys_are_validated() {
        assert_eq!(clean_key("  sk-abc123  ").unwrap(), "sk-abc123");
        assert!(clean_key("").is_err());
        assert!(clean_key("sk abc").is_err());
        assert!(clean_key("sk-abc\nX-Evil: 1").is_err(), "header injection via a pasted newline");
        assert!(clean_key(&"k".repeat(600)).is_err());
    }

    #[test]
    fn names_are_per_provider() {
        assert_ne!(key_name("openai"), key_name("gemini"));
    }
}
