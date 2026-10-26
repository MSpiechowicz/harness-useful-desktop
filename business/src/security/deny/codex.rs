//! Codex's private files (Codex CLI 0.154): what lies directly in a Codex home (`~/.codex`, or the
//! directory `CODEX_HOME` names, registered with [`add_codex_root`]) and is never read.
//!
//! - Files: the prompts typed into Codex (`history.jsonl`), the session titles
//!   (`session_index.jsonl`), dictation transcripts (`transcription-history.jsonl`), the install id,
//!   the desktop app's state (`.codex-global-state.json` and its `.bak`), the copy Codex keeps of
//!   `config.toml` (`config.toml.backup`, which holds the same secrets) and its memory stores
//!   (`memories_*`). The login store `auth.json` is denied by name everywhere.
//! - Folders: the session rollouts (`sessions`, `archived_sessions`; the app reads sessions through
//!   `codex app-server`, never from these files), shell environment snapshots, MCP login locks,
//!   computer-use and dictation data, logs, databases, caches, IPC sockets, generated images, the
//!   node REPL, pets, writer locks, temporary files, rollout migrations and visualizations.
//!
//! Settings (`config.toml`), instructions (`AGENTS.md`), skills, rules, plugins and the marketplace
//! clones below `.tmp` stay readable.

use std::path::Path;
use std::sync::RwLock;

use super::{DenyReason, lowercase_components};

/// The default name of the Codex home below the home directory.
const CODEX_DIR: &str = ".codex";

/// Codex homes with another name (`CODEX_HOME`), as lowercase path components. Roots are only ever
/// added, so registering one never weakens the list.
static CODEX_ROOTS: RwLock<Vec<Vec<String>>> = RwLock::new(Vec::new());

/// File names (lowercase) never read directly in a Codex home.
const DENIED_FILES: &[&str] = &[
    "history.jsonl",
    "session_index.jsonl",
    "transcription-history.jsonl",
    "installation_id",
    ".codex-global-state.json",
    ".codex-global-state.json.bak",
    "config.toml.backup",
];

/// Prefix of file names (lowercase) never read directly in a Codex home: its memory stores
/// (`memories_1.sqlite` and its journal files).
const DENIED_FILE_PREFIX: &str = "memories_";

/// Folder names (lowercase) whose contents are never read when they lie directly in a Codex home.
const DENIED_DIRS: &[&str] = &[
    "sessions",
    "archived_sessions",
    "shell_snapshots",
    "mcp-oauth-locks",
    "computer-use",
    "dictation-history",
    "log",
    "logs",
    "sqlite",
    "cache",
    "ipc",
    "generated_images",
    "node_repl",
    "pets",
    "thread-writer-locks",
    "tmp",
    "rollout-migrations",
    "visualizations",
    "memories",
];

/// Registers `root` as a Codex home (`CODEX_HOME`): the private names [`denied`] refuses directly
/// in a folder named `.codex` are refused below `root` too, whatever its name. [`crate::env::Env`]
/// registers its root (and the root's canonical path) when it is captured or built. Registering
/// the same root again changes nothing.
pub fn add_codex_root(root: &Path) {
    let mut roots = vec![lowercase_components(root)];
    if let Ok(canonical) = dunce::canonicalize(root) {
        roots.push(lowercase_components(&canonical));
    }

    let mut registered = CODEX_ROOTS.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    for root in roots {
        if !root.is_empty() && !registered.contains(&root) {
            registered.push(root);
        }
    }
}

/// True when `ancestors` is a Codex home: a folder named `.codex` or a registered root.
fn is_codex_root(ancestors: &[String]) -> bool {
    if ancestors.last().is_some_and(|parent| parent == CODEX_DIR) {
        return true;
    }

    let roots = CODEX_ROOTS.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    roots.iter().any(|root| root.as_slice() == ancestors)
}

/// Checks `path` (as given, without touching the filesystem) against Codex's private names. Only
/// names directly in a Codex home count, so a project folder called `sessions` elsewhere is not
/// caught. Names are compared case-insensitively.
pub(crate) fn denied(path: &Path) -> Option<DenyReason> {
    let components = lowercase_components(path);
    let (file_name, parents) = components.split_last()?;

    let private_file = DENIED_FILES.contains(&file_name.as_str()) || file_name.starts_with(DENIED_FILE_PREFIX);
    if private_file && is_codex_root(parents) {
        return Some(DenyReason::FileName(file_name.clone()));
    }

    parents.iter().enumerate().find_map(|(index, dir)| {
        let private_dir = DENIED_DIRS.contains(&dir.as_str()) && is_codex_root(&parents[..index]);

        private_dir.then(|| DenyReason::Directory(dir.clone()))
    })
}
