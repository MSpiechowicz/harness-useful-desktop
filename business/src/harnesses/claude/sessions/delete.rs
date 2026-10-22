//! `session_delete` for Claude Code: removes a saved session with everything Claude Code keeps for
//! it.
//!
//! The session file is resolved like every other session command does ([`find_session`]: a
//! regular `<uuid>.jsonl` file found by a shallow scan of the project's folder) and must hold a
//! line of that session. Then, in this order:
//!
//! 1. the session file `<uuid>.jsonl`,
//! 2. the session's folder `<uuid>/` (subagent transcripts, tool outputs, the rename sidecar) and
//!    the `<uuid>.ccr-tip.json` and `<uuid>.precompact.json` files next to it, as Claude Code's own
//!    cleanup does,
//! 3. its file edit history `<config>/file-history/<uuid>/` and its environment
//!    `<config>/session-env/<uuid>/`.
//!
//! Those are removed without being read, so the deny list (which keeps both folders from being
//! read) does not stand in the way. Symlinks are never followed: a linked folder or file is left
//! alone.
//!
//! A session that runs in Claude Code is never deleted: each running `claude` writes
//! `<config>/sessions/<pid>.json` (`{pid, sessionId, cwd, …}`) and removes it on exit. While a
//! file names the session and its process may still run (see
//! [`pid_may_be_running`](crate::process::pid_may_be_running)), the delete is `Forbidden`.

use std::fs;
use std::path::Path;

use serde_json::Value;

use super::entries::text;
use super::{MAX_DIR_ENTRIES, find_session};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harnesses::claude::paths::{config_dir, sessions_dir};
use crate::pi_family::sessions::{Lines, SUMMARY_BYTES};
use crate::process::pid_may_be_running;
use crate::security::SafeReader;

/// Files next to the session file that belong to it: `<uuid>` plus each suffix.
const SIDE_FILES: [&str; 2] = [".ccr-tip.json", ".precompact.json"];

/// Folders below the config directory that hold a `<uuid>/` folder per session.
const SESSION_STORES: [&str; 2] = ["file-history", "session-env"];

/// The folder of the running sessions' pid files below the config directory.
const PID_FILES: &str = "sessions";

/// Largest pid file read.
const PID_FILE_MAX_BYTES: u64 = 64 * 1024;

/// Deletes session `id` of `project` (see the module docs). Claude Code sessions are never empty
/// copies, so `empty_copy_only` refuses every session with `InvalidInput`.
///
/// Errors: `InvalidInput` for an invalid id, a file without a line of the session, or
/// `empty_copy_only`; `NotFound` for an unknown id; `Forbidden` while the session runs; `Io` when
/// the session file or one of its folders cannot be removed (the rest is still removed).
pub(in crate::harnesses::claude) fn delete_session(
    env: &Env,
    project: &Path,
    id: &str,
    empty_copy_only: bool,
) -> Result<()> {
    let dir = sessions_dir(env, project);
    let path = find_session(&dir, id)?;

    if !names_session(&path, id) {
        return Err(BusinessError::InvalidInput(format!(
            "the file of session {id} is not a session"
        )));
    }

    if empty_copy_only {
        return Err(BusinessError::InvalidInput(format!(
            "session {id} is not an empty copy"
        )));
    }

    let config = config_dir(env);

    if is_running(&config, id) {
        return Err(BusinessError::Forbidden(format!(
            "session {id} is open in Claude Code, close it there first"
        )));
    }

    fs::remove_file(&path).map_err(|error| BusinessError::from_io(format!("deleting session {id}"), error))?;

    let mut failure = None;

    for side in SIDE_FILES {
        let side = dir.join(format!("{id}{side}"));

        if fs::symlink_metadata(&side).is_ok_and(|metadata| metadata.is_file()) {
            fs::remove_file(&side).ok();
        }
    }

    let folders = std::iter::once((dir.join(id), "folder"))
        .chain(SESSION_STORES.iter().map(|store| (config.join(store).join(id), *store)));

    for (folder, what) in folders {
        if let Err(error) = remove_folder(&folder) {
            let context = format!("session {id} was deleted but not its {what}");
            failure.get_or_insert(BusinessError::from_io(context, error));
        }
    }

    failure.map_or(Ok(()), Err)
}

/// Removes `folder` when it is a real folder (a symlink is left alone).
fn remove_folder(folder: &Path) -> std::io::Result<()> {
    // `symlink_metadata` does not follow links, so a linked folder is not a folder here.
    if fs::symlink_metadata(folder).is_ok_and(|metadata| metadata.is_dir()) {
        fs::remove_dir_all(folder)?;
    }

    Ok(())
}

/// True when a line in the first [`SUMMARY_BYTES`] of `path` belongs to session `id`.
fn names_session(path: &Path, id: &str) -> bool {
    let Ok(lines) = Lines::open(path, SUMMARY_BYTES) else {
        return false;
    };

    lines
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .any(|line| text(&line, "sessionId") == Some(id))
}

/// True when a pid file in `<config>/sessions/` names session `id` and its process may run.
fn is_running(config: &Path, id: &str) -> bool {
    let Ok(entries) = fs::read_dir(config.join(PID_FILES)) else {
        return false;
    };

    for entry in entries.take(MAX_DIR_ENTRIES).flatten() {
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        let name = entry.file_name();
        let named_by_pid = name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|byte| byte.is_ascii_digit()));

        if !is_file || !named_by_pid {
            continue;
        }

        let Ok(raw) = SafeReader::read_to_string(&entry.path(), PID_FILE_MAX_BYTES) else {
            continue;
        };

        let Ok(record) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };

        let pid = record
            .get("pid")
            .and_then(Value::as_u64)
            .and_then(|pid| u32::try_from(pid).ok());

        if text(&record, "sessionId") == Some(id) && pid.is_some_and(pid_may_be_running) {
            return true;
        }
    }

    false
}
