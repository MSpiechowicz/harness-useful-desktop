//! The project key of folders, asked of the installed server with `harness-memory project-key
//! --json` (see its `cli_project_key`), which agents cannot reach through MCP. Harness Memory
//! saves in a repository only while the key of its project is the one pinned when the user turned
//! it on, and this app cannot compute keys itself, so it asks:
//!
//! - when the user turns a repository on ([`pin_of`]): the key of the folder the user picked, not
//!   of the saving root stored for it (a worktree of a separate git folder has the key of its
//!   remote, the git folder alone one made from its path);
//! - when the guards are read or changed ([`with_changed`]): one question for the entries of both
//!   kinds, to tell the ones whose key changed since.
//!
//! The question is `[path, ...]` as JSON on stdin and the answer `[{"path", "root", "key"}]`, or
//! `{"path", "error"}` for a path the server cannot use, one per path in order. At most
//! [`PATHS_MAX`] paths are asked at once.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::memory_binary::resolve_memory;
use super::memory_cli::{CliAnswer, CliCall, USAGE_EXIT, run_memory_cli};
use super::memory_features::MemoryFeatures;
use super::memory_guards::{Pin, checkable_repos, read_global_healed, update_global, with_changed_repos};
use super::memory_held::guarded_server;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryGuards, MemoryGuardsPatch};
use crate::process::ProcessRunner;

/// How long one question may take.
const KEYS_TIMEOUT: Duration = Duration::from_secs(30);

/// The output kept of an answer (4 MiB: [`PATHS_MAX`] answers of a few KiB each).
const KEYS_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// The most paths asked at once: what the server answers (two kinds of 256 entries each).
pub const PATHS_MAX: usize = 512;

/// What the server says about one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderKey {
    /// Its project key, `None` for a folder that has no project (the home folder).
    Key(Option<String>),
    /// Why the server cannot use the folder (it is gone, not a folder, not absolute).
    Unusable(String),
}

/// `project-key --json`.
fn project_key_args() -> Vec<OsString> {
    ["project-key", "--json"].map(OsString::from).into()
}

/// The keys of `folders`, in order, from the server at `server`. `unsupported` when it predates
/// the command, `process` with the server's line when it fails, `protocol` for an answer this app
/// cannot read. At most [`PATHS_MAX`] folders.
pub async fn ask_keys(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    folders: &[String],
) -> Result<Vec<FolderKey>> {
    if folders.len() > PATHS_MAX {
        return Err(BusinessError::Internal(format!(
            "more than {PATHS_MAX} folders asked for their project keys at once"
        )));
    }

    let stdin = serde_json::to_vec(folders)
        .map_err(|error| BusinessError::Internal(format!("encoding the folders failed: {error}")))?;

    let call = CliCall {
        name: "project-key",
        args: project_key_args(),
        timeout: KEYS_TIMEOUT,
        output_bytes: KEYS_OUTPUT_BYTES,
        stdin: Some(stdin),
    };

    match run_memory_cli(env, runner, server, call).await? {
        CliAnswer::Done(stdout) => parse_keys(&stdout, folders),
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => Err(BusinessError::Unsupported(
            "this Harness Memory cannot tell which project a folder is; update Harness Memory".to_owned(),
        )),
        CliAnswer::Failed { message, .. } => Err(BusinessError::Process(message)),
    }
}

/// The answers in `stdout` for `folders`; `protocol` unless there is one per folder, in order.
fn parse_keys(stdout: &str, folders: &[String]) -> Result<Vec<FolderKey>> {
    let unreadable =
        || BusinessError::Protocol("harness-memory project-key answered output this app cannot read".to_owned());

    let Ok(Value::Array(answers)) = serde_json::from_str::<Value>(stdout) else {
        return Err(unreadable());
    };

    if answers.len() != folders.len() {
        return Err(unreadable());
    }

    answers
        .iter()
        .zip(folders)
        .map(|(answer, folder)| {
            if answer.get("path").and_then(Value::as_str) != Some(folder) {
                return Err(unreadable());
            }

            if let Some(error) = answer.get("error").and_then(Value::as_str) {
                return Ok(FolderKey::Unusable(error.to_owned()));
            }

            match answer.get("key") {
                Some(Value::String(key)) => Ok(FolderKey::Key(Some(key.clone()))),
                Some(Value::Null) => Ok(FolderKey::Key(None)),
                _ => Err(unreadable()),
            }
        })
        .collect()
}

/// The guards of every chat in `data_dir` as `memory_guards_get` shows them: read (a version 1 file
/// moved first, see [`read_global_healed`]) with each kind's `changedRepos` from the installed
/// server ([`with_changed`]).
pub async fn read_guards(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    data_dir: PathBuf,
) -> Result<MemoryGuards> {
    let reader = env.clone();
    let guards = blocking(move || read_global_healed(&reader, &data_dir)).await?;

    Ok(with_changed(env, runner, features, guards).await)
}

/// Applies `patch` as `memory_guards_update` does and returns the guards as written, with
/// `changedRepos` ([`with_changed`]). Turning a repository on (`addRepo`) first asks the installed
/// server for the key of the folder given ([`pin_of`]) and writes the entry with that pin under
/// the lock ([`update_global`]): when the server cannot be asked, or cannot use the folder,
/// nothing is written.
pub async fn update_guards(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    data_dir: PathBuf,
    patch: MemoryGuardsPatch,
) -> Result<MemoryGuards> {
    let pin = match &patch.add_repo {
        Some(folder) => Some(pin_of(env, runner, features, folder).await?),
        None => None,
    };

    let guards = blocking(move || update_global(&data_dir, &patch, pin)).await?;

    Ok(with_changed(env, runner, features, guards).await)
}

/// Runs `task` (file IO under a lock) off the async runtime's worker threads.
async fn blocking<T, F>(task: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| BusinessError::Internal(format!("a background task failed: {error}")))?
}

/// The [`Pin`] of the folder `folder` the user turns a repository on from: the key the installed
/// server (with save guards) gives it now. `notFound` while Harness Memory is not installed,
/// `unsupported` when it predates save guards or the command, `process` when it fails, and
/// `invalidInput` naming the reason when it cannot use the folder: nothing is turned on then.
pub async fn pin_of(env: &Env, runner: &dyn ProcessRunner, features: &MemoryFeatures, folder: &str) -> Result<Pin> {
    let server = guarded_server(env, runner, features).await?;
    let answers = ask_keys(env, runner, &server, &[folder.to_owned()]).await?;

    match answers.into_iter().next() {
        Some(FolderKey::Key(key)) => Ok(Pin(key)),
        Some(FolderKey::Unusable(reason)) => Err(BusinessError::InvalidInput(format!(
            "Harness Memory cannot use this folder: {reason}"
        ))),
        None => Err(BusinessError::Protocol(
            "harness-memory project-key answered nothing".to_owned(),
        )),
    }
}

/// `guards` with each kind's `changedRepos` set from the keys the installed server gives the
/// entries now, in one question. Left empty, with the reason logged, when the server is not
/// installed, predates `save-opt-in`, cannot be asked or answers something unreadable: a missing
/// answer never marks an entry as changed.
pub async fn with_changed(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    guards: MemoryGuards,
) -> MemoryGuards {
    let entries = checkable_repos(&guards);

    if entries.is_empty() {
        return guards;
    }

    match current_keys(env, runner, features, &entries).await {
        Ok(current) => with_changed_repos(guards, &current),
        Err(error) => {
            tracing::warn!(code = ?error.code(), "the project keys of the repositories turned on could not be checked");
            guards
        }
    }
}

/// The current key of each of `entries` the server could answer for.
async fn current_keys(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    entries: &[String],
) -> Result<HashMap<String, Option<String>>> {
    let Some(server) = resolve_memory(env) else {
        return Err(BusinessError::NotFound("Harness Memory".to_owned()));
    };

    if !features.support(env, runner).await.save_opt_in() {
        return Err(BusinessError::Unsupported(
            "Harness Memory predates opt-in saving".to_owned(),
        ));
    }

    let mut current = HashMap::new();

    for chunk in entries.chunks(PATHS_MAX) {
        let answers = ask_keys(env, runner, &server.path, chunk).await?;

        for (entry, answer) in chunk.iter().zip(answers) {
            if let FolderKey::Key(key) = answer {
                current.insert(entry.clone(), key);
            }
        }
    }

    Ok(current)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn folders(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn each_answer_is_a_key_no_key_or_a_reason_in_the_order_asked() {
        let asked = folders(&["/a", "/home", "/gone"]);
        let stdout = r#"[
            {"path": "/a", "root": "/a", "key": "git:example.invalid/acme/a"},
            {"path": "/home", "root": "/home", "key": null},
            {"path": "/gone", "error": "no such folder"}
        ]"#;

        assert_eq!(
            parse_keys(stdout, &asked).unwrap(),
            vec![
                FolderKey::Key(Some("git:example.invalid/acme/a".to_owned())),
                FolderKey::Key(None),
                FolderKey::Unusable("no such folder".to_owned()),
            ]
        );
    }

    #[test]
    fn an_answer_that_is_not_one_per_folder_in_order_is_refused() {
        let asked = folders(&["/a", "/b"]);

        for stdout in [
            "",
            "{}",
            r#"[{"path": "/a", "key": null}]"#,
            r#"[{"path": "/b", "key": null}, {"path": "/a", "key": null}]"#,
            r#"[{"path": "/a", "key": 3}, {"path": "/b", "key": null}]"#,
            r#"[{"path": "/a", "key": null}, {"path": "/b"}]"#,
        ] {
            assert!(
                matches!(parse_keys(stdout, &asked), Err(BusinessError::Protocol(_))),
                "{stdout}"
            );
        }
    }
}
