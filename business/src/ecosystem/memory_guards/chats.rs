//! The chat files after a chat started: its toggles, newest revision only, and moves, one write
//! at a time per chat ([`ChatGuardWrites`]), and the startup sweep of old ones ([`sweep_chats`]).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use super::{
    CHATS_DIR, GUARDS_DIR, chat_repo_root, check_chat_key, check_owner, is_chat_key, read_chat, read_chat_file,
    write_chat,
};
use crate::error::Result;
use crate::model::{MemoryChatGuards, MemoryChatGuardsSet};

/// How long a chat file stays untouched before the startup sweep removes it (30 days).
pub const CHAT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The writes of chat files after a chat started, one at a time per chat: the toggles
/// (`memory_chat_guards_set`), newest revision only, and moves. Each chat's newest revision is
/// kept for the app's lifetime.
#[derive(Default)]
pub struct ChatGuardWrites {
    chats: Mutex<HashMap<String, Arc<Mutex<Option<u64>>>>>,
}

impl ChatGuardWrites {
    /// Writes `guards` to chat `key`'s file in `data_dir` at `revision`, unless a write of the same
    /// or a higher revision came first; either way the answer names the last revision written. The
    /// file keeps the project root it names; while it is missing or cannot be read, the root is
    /// `project_root`, the root of the chat's project now ([`chat_repo_root`]), so a toggle never
    /// drops it. `invalidInput` for a key that is not one.
    pub fn set_guards(
        &self,
        data_dir: &Path,
        key: &str,
        guards: &MemoryChatGuards,
        revision: u64,
        project_root: Option<&str>,
    ) -> Result<MemoryChatGuardsSet> {
        check_chat_key(key)?;

        let chat = self.chat(key);
        let mut newest = chat.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(last) = newest.filter(|last| revision <= *last) {
            tracing::debug!(revision, last, "a late memory chat guards write was ignored");

            return Ok(MemoryChatGuardsSet {
                applied: false,
                revision: last,
            });
        }

        let repo_root = match read_chat_file(data_dir, key) {
            Ok(Some(file)) => file.repo_root,
            Ok(None) => project_root.map(str::to_owned),
            Err(error) => {
                tracing::warn!(
                    code = ?error.code(),
                    "a memory chat file that cannot be read is rebuilt from the chat's project"
                );
                project_root.map(str::to_owned)
            }
        };

        write_chat(data_dir, key, guards, repo_root.as_deref())?;
        *newest = Some(revision);

        Ok(MemoryChatGuardsSet {
            applied: true,
            revision,
        })
    }

    /// Chat `key` moved to `project`: its file in `data_dir` names the project root there from now on.
    /// Nothing is written while the file is missing (the chat has no session variable then).
    pub fn set_project(&self, data_dir: &Path, key: &str, project: &Path) -> Result<()> {
        check_chat_key(key)?;

        let chat = self.chat(key);
        let _writing = chat.lock().unwrap_or_else(PoisonError::into_inner);

        let Some(guards) = read_chat(data_dir, key)? else {
            return Ok(());
        };

        write_chat(data_dir, key, &guards, chat_repo_root(project).as_deref())
    }

    fn chat(&self, key: &str) -> Arc<Mutex<Option<u64>>> {
        let mut chats = self.chats.lock().unwrap_or_else(PoisonError::into_inner);

        chats.entry(key.to_owned()).or_default().clone()
    }
}

/// Removes the chat files in `data_dir` not changed for `max_age` ([`CHAT_MAX_AGE`] at startup);
/// how many went. Only `<key>.json` regular files go, never through a symbolic link: nothing is
/// removed when the guards or chats folder is a link or another user's. Errors are logged.
pub fn sweep_chats(data_dir: &Path, max_age: Duration) -> usize {
    let guards = data_dir.join(GUARDS_DIR);
    let chats = guards.join(CHATS_DIR);

    for dir in [data_dir, guards.as_path(), chats.as_path()] {
        let is_folder = std::fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.is_dir());

        if !is_folder || check_owner(dir).is_err() {
            return 0;
        }
    }

    let Ok(entries) = std::fs::read_dir(&chats) else {
        return 0;
    };

    let now = SystemTime::now();
    let mut removed = 0;

    for entry in entries.filter_map(|entry| entry.ok()) {
        let name = entry.file_name();
        let is_chat = name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .is_some_and(is_chat_key);

        // `DirEntry::metadata` does not follow a symbolic link.
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        let old = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= max_age);

        if !is_chat || !metadata.is_file() || !old {
            continue;
        }

        match std::fs::remove_file(entry.path()) {
            Ok(()) => removed += 1,
            Err(error) => tracing::warn!(%error, "an old memory chat file could not be removed"),
        }
    }

    if removed > 0 {
        tracing::info!(removed, "old memory chat files removed");
    }

    removed
}
