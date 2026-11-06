//! Reads where a chat's memory guards stand ([`read_memory_guards`]) for its hooks: the
//! memory-save hook and the memory settings notice (see [`super::engine`]).

use std::path::Path;

use super::engine::MemoryGuardState;
use crate::ecosystem::memory_guards::read_listed;
use crate::ecosystem::read_chat;
use crate::model::MemoryChatGuards;

/// Where the guards in `data_dir` leave chat `key` (none: no chat rules) in project root `repo`
/// (none outside one). A missing `global.json` leaves saving off here (the desktop treats it as off
/// and writes it before any agent starts, while Harness Memory treats it as no rules until the
/// desktop's marker exists), a file that
/// cannot be read refuses every save ([`MemoryGuardState::CLOSED`]).
pub fn read_memory_guards(data_dir: &Path, key: Option<&str>, repo: Option<&Path>) -> MemoryGuardState {
    let global = match read_listed(data_dir) {
        Ok(global) => global,
        Err(error) => {
            tracing::warn!(code = ?error.code(), "the memory save guards cannot be read");
            return MemoryGuardState::CLOSED;
        }
    };

    let chat = match key.map(|key| read_chat(data_dir, key)) {
        None | Some(Ok(None)) => MemoryChatGuards::default(),
        Some(Ok(Some(chat))) => chat,
        Some(Err(error)) => {
            tracing::warn!(code = ?error.code(), "the memory guards of a chat cannot be read");
            return MemoryGuardState::CLOSED;
        }
    };

    MemoryGuardState::of(&global, &chat, repo)
}
