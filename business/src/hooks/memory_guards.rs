//! Reads where a chat's memory guards stand ([`read_memory_guards`]) for its hooks: the
//! memory-save hook and the memory settings notice (see [`super::engine`]).

use std::path::Path;

use super::engine::MemoryGuardState;
use crate::ecosystem::{read_chat, read_global};
use crate::model::MemoryChatGuards;

/// Where the guards in `data_dir` leave chat `key` (none: no chat rules) in project root `repo`
/// (none outside one). Read as Harness Memory reads them: a missing file has no rules, a file that
/// cannot be read refuses every save ([`MemoryGuardState::CLOSED`]).
pub fn read_memory_guards(data_dir: &Path, key: Option<&str>, repo: Option<&Path>) -> MemoryGuardState {
    let global = match read_global(data_dir) {
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
