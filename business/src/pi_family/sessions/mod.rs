//! Saved sessions on disk, shared by the PI-family harnesses (OMP keeps PI's session format).
//!
//! A project's sessions live in one folder, `<sessions root>/<encoded project>/` ([`encode`]),
//! or a configured session directory, as `<timestamp>_<id>.jsonl` files (JSONL, header line,
//! entries forming a tree through `id`/`parentId`; OMP adds a fixed-width title slot as line 1).
//! Next to each file a same-named companion directory may hold subagent transcripts and tool
//! logs; it is never entered.
//!
//! The webview only ever sends an opaque [`SessionRef`](crate::model::SessionRef); [`find_session`]
//! resolves its id with a shallow scan of the project's folder, so no other file can be named.
//!
//! - [`list_sessions`]: summaries for `sessions_list` (cached in a [`SessionCache`]).
//! - [`session_tree`]: the branch tree for `session_tree`.
//! - [`session_messages`]: the paged active-path transcript for `session_messages`.
//! - [`session_files_touched`]: edited files for `session_files_touched`.
//! - [`delete_session`]: removes a session with its companion directory for `session_delete`.
//! - [`session_totals`]: the session count and last use across every project for
//!   `harness_session_totals`.
//! - [`recall_block`]: the memory recall block older app versions put in front of a message, left
//!   out of previews.

mod delete;
pub mod encode;
mod parse;
pub mod recall_block;
mod scan;
mod stats;
mod totals;
mod transcript;
mod tree;

pub use delete::delete_session;
pub use encode::{canonical_text, omp_dir_name, pi_dir_name, temp_root};
pub use parse::epoch_millis;
pub use scan::{MAX_SESSION_FILES, SUMMARY_BYTES, SessionCache, list_sessions};
pub(crate) use stats::{StatsTally, page_range};
pub use totals::{MAX_TOTALS_ENTRIES, SessionStore, configured_store, session_totals};
pub use transcript::{session_files_touched, session_messages};
pub use tree::{TreeReport, read_tree, session_tree};

use std::fs;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::security::is_denied;

/// Directory entries examined at most per lookup.
const MAX_ENTRIES: usize = 100_000;

/// Longest accepted session id.
const MAX_ID_LEN: usize = 128;

/// True for a session id that is safe as a file-name suffix and as a CLI argument: 1 to 128 of
/// `A-Z a-z 0-9 . _ -`, not starting with `-` or `.`.
pub fn is_safe_session_id(id: &str) -> bool {
    let allowed = |character: char| character.is_ascii_alphanumeric() || "._-".contains(character);

    !id.is_empty() && id.len() <= MAX_ID_LEN && !id.starts_with(['-', '.']) && id.chars().all(allowed)
}

/// The session id of a `<timestamp>_<id>.jsonl` file name (the part after the last `_`), when
/// it is a safe id.
pub fn session_id_of(file_name: &str) -> Option<String> {
    let stem = file_name.strip_suffix(".jsonl")?;
    let (_, id) = stem.rsplit_once('_')?;

    is_safe_session_id(id).then(|| id.to_owned())
}

/// The session file of `id` in the project's session folder `dir`: a top-level regular file
/// named `<timestamp>_<id>.jsonl` (symlinks, subdirectories and deny-listed files never match).
/// When several files carry the id, the first one found is returned.
pub fn find_session(dir: &Path, id: &str) -> Result<PathBuf> {
    session_paths(dir, id)?.into_iter().next().ok_or_else(|| not_found(id))
}

/// Like [`find_session`], but an id carried by several files is `InvalidInput`.
fn find_only_session(dir: &Path, id: &str) -> Result<PathBuf> {
    let mut paths = session_paths(dir, id)?;

    if paths.len() > 1 {
        return Err(BusinessError::InvalidInput(format!("several files carry session {id}")));
    }

    paths.pop().ok_or_else(|| not_found(id))
}

/// Every file of `dir` that [`find_session`] could return for `id`.
fn session_paths(dir: &Path, id: &str) -> Result<Vec<PathBuf>> {
    if !is_safe_session_id(id) {
        return Err(BusinessError::InvalidInput("invalid session id".to_owned()));
    }

    let entries = fs::read_dir(dir).map_err(|_| not_found(id))?;
    let mut paths = Vec::new();

    for entry in entries.take(MAX_ENTRIES).flatten() {
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        let name = entry.file_name();
        let matches = name
            .to_str()
            .and_then(session_id_of)
            .is_some_and(|candidate| candidate == id);
        let path = entry.path();

        if is_file && matches && is_denied(&path).is_none() {
            paths.push(path);
        }
    }

    Ok(paths)
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}

/// A configured session directory (`--session-dir` semantics: the folder itself, not a root):
/// `~` is expanded and a relative value resolves against the project, like the harness does.
pub fn configured_session_dir(env: &Env, project: &Path, raw: &str) -> PathBuf {
    let expanded = env.expand_home(raw.trim());

    if expanded.is_absolute() {
        expanded
    } else {
        project.join(expanded)
    }
}
