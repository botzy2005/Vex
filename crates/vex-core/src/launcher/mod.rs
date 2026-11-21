//! Quick Launcher core: everything that can be decided without Windows.
//!
//! * `calc`: a safe arithmetic parser (no `eval`, no code execution).
//! * `commands`: the allowlisted command registry and its argument validation.
//! * `rank`: fuzzy matching and ordering of results.
//! * `files`: the in-memory file index and its filters.
//! * `store`: pins, recent launches and search history.
//!
//! Nothing here touches the filesystem except `store` (atomic JSON) and `files::scan` (bounded walk),
//! and nothing here starts a process: the shell crate decides how to open a target.

pub mod calc;
pub mod commands;
pub mod files;
pub mod rank;
pub mod search;
pub mod store;

use serde::{Deserialize, Serialize};

/// One installed application the launcher can start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    /// Stable id: the shortcut path for desktop apps or `shell:AppsFolder\<AUMID>` for Store apps.
    pub id: String,
    pub name: String,
    pub kind: AppKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppKind {
    Desktop,
    Store,
}
