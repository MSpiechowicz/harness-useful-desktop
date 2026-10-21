//! The saves Harness Memory held for the user's review while "Ask before saving" was on
//! (`memory_held_list`, `memory_held_save`, `memory_held_skip`).
//!
//! The server writes each to `held/<chatKey>/<heldId>.json` in its data folder ([`HELD_DIR`], one
//! folder per chat) and owns that folder: it checks the version, sweeps old ones and keeps them out of the store. So the app
//! reads and settles them only through the server's user-only commands, as
//! [`memory_user_save`](super::memory_user_save) runs them, and only looks at the folder to
//! notice a change ([`held_stamp`]):
//!
//! - `held list --json --chat KEY` answers `{"held": [...]}`, newest first, its output read up to
//!   [`HELD_LIST_OUTPUT_BYTES`] (50 saves of up to 512 KiB each). An entry this app cannot read
//!   (another version, a kind or tool it does not know) is skipped;
//! - `held save <heldId> --json [--expect REVISION]`, with the user's edited draft on stdin when
//!   there is one, answers `{"saved": {"kind": "note", "id"}}` or `{"saved": {"kind": "script",
//!   "name"}}` (a script saved untrusted); a `note_update` of a note deleted meanwhile is refused
//!   `gone` (a conflict here), a save the agent held again since the user saw it (another
//!   revision) `changed`;
//! - `held skip <heldId> --json [--expect REVISION]` answers `{"skipped": "<heldId>"}`, refused
//!   `changed` as `held save` is.
//!
//! Neither the arguments nor a draft is logged.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::Value;

use super::catalog::MEMORY_BINARY;
use super::memory_binary::resolve_memory;
use super::memory_cli::CliCall;
use super::memory_features::MemoryFeatures;
use super::memory_guards::{check_chat_key, is_chat_key};
use super::memory_user_save::{SAVE_TIMEOUT, run_save, run_save_call};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{HeldItem, HeldKind, HeldSaved, HeldTool};
use crate::process::ProcessRunner;

/// The folder of held saves in the data folder.
pub const HELD_DIR: &str = "held";

/// The held file format this app reads.
const HELD_VERSION: u32 = 1;

/// The output kept of `held list` (64 MiB: a chat holds at most 50 saves of up to 512 KiB, in
/// JSON that may grow when it is escaped).
pub const HELD_LIST_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// The installed server when it lists `save-guards`: `notFound` when Harness Memory is not
/// installed, `unsupported` when it predates save guards.
pub async fn guarded_server(env: &Env, runner: &dyn ProcessRunner, features: &MemoryFeatures) -> Result<PathBuf> {
    let Some(server) = resolve_memory(env) else {
        return Err(BusinessError::NotFound(format!("executable {MEMORY_BINARY}")));
    };

    if !features.support(env, runner).await.save_guards() {
        return Err(BusinessError::Unsupported(format!(
            "this {MEMORY_BINARY} predates save guards; update Harness Memory"
        )));
    }

    Ok(server.path)
}

/// `held list --json --chat KEY`.
pub fn held_list_args(chat_key: &str) -> Vec<OsString> {
    ["held", "list", "--json", "--chat", chat_key]
        .map(OsString::from)
        .into()
}

/// `held save <heldId> --json [--expect REVISION]`.
pub fn held_save_args(held_id: &str, revision: Option<&str>) -> Vec<OsString> {
    held_args("save", held_id, revision)
}

/// `held skip <heldId> --json [--expect REVISION]`.
pub fn held_skip_args(held_id: &str, revision: Option<&str>) -> Vec<OsString> {
    held_args("skip", held_id, revision)
}

fn held_args(command: &str, held_id: &str, revision: Option<&str>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["held".into(), command.into(), held_id.into(), "--json".into()];

    if let Some(revision) = revision {
        args.push("--expect".into());
        args.push(revision.into());
    }

    args
}

/// The held saves of chat `chat_key`, newest first. `invalidInput` for a key that is not one.
pub async fn list_held(env: &Env, runner: &dyn ProcessRunner, server: &Path, chat_key: &str) -> Result<Vec<HeldItem>> {
    check_chat_key(chat_key)?;

    let call = CliCall {
        name: "held list",
        args: held_list_args(chat_key),
        timeout: SAVE_TIMEOUT,
        output_bytes: HELD_LIST_OUTPUT_BYTES,
        stdin: None,
    };

    let data = run_save_call(env, runner, server, call)
        .await
        .map_err(|refusal| refusal.error)?;

    let Some(entries) = data.get("held").and_then(Value::as_array) else {
        return Err(BusinessError::Protocol(
            "harness-memory held list answered without `held`".to_owned(),
        ));
    };

    let items: Vec<HeldItem> = entries.iter().filter_map(parse_held).collect();

    if items.len() < entries.len() {
        tracing::warn!(
            skipped = entries.len() - items.len(),
            "held memory saves this app cannot read were skipped"
        );
    }

    Ok(items)
}

/// What the user saves of a held save: its id, the revision they saw (when the UI knows it) and
/// their edited draft, if any.
#[derive(Debug, Clone, Copy)]
pub struct HeldSave<'a> {
    /// The held save's id.
    pub held_id: &'a str,
    /// The revision the user reviewed: the save is refused `changed` when it is no longer the one
    /// held.
    pub revision: Option<&'a str>,
    /// The user's draft (a note or script), replacing the agent's fields it names.
    pub edited: Option<&'a Value>,
}

/// Saves held save `save.held_id`, as the agent sent it or as the user edited it. `invalidInput`
/// for an id or revision that is not one or an edit that is not an object, `notFound` when it is
/// gone, `conflict` when the note it updates was deleted, `changed` when the agent held it again
/// since the user saw `save.revision`.
pub async fn save_held(env: &Env, runner: &dyn ProcessRunner, server: &Path, save: HeldSave<'_>) -> Result<HeldSaved> {
    let HeldSave {
        held_id,
        revision,
        edited,
    } = save;

    check_held_id(held_id)?;

    if let Some(revision) = revision {
        check_revision(revision)?;
    }

    if edited.is_some_and(|draft| !draft.is_object()) {
        return Err(BusinessError::InvalidInput("an edited draft is an object".to_owned()));
    }

    let args = held_save_args(held_id, revision);
    let data = run_save(env, runner, server, "held save", args, edited)
        .await
        .map_err(|refusal| refusal.error)?;

    let saved = data
        .get("saved")
        .cloned()
        .and_then(|saved| serde_json::from_value::<HeldSaved>(saved).ok())
        .ok_or_else(|| BusinessError::Protocol("harness-memory held save answered without `saved`".to_owned()))?;

    tracing::info!(edited = edited.is_some(), "a held memory save was saved");
    Ok(saved)
}

/// Drops held save `held_id`: nothing is saved. With `revision` (the one the user saw) only that
/// version is dropped. `invalidInput` for an id or revision that is not one, `notFound` when it is
/// gone, `changed` when the agent held it again since the user saw `revision` (it stays held).
pub async fn skip_held(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    held_id: &str,
    revision: Option<&str>,
) -> Result<()> {
    check_held_id(held_id)?;

    if let Some(revision) = revision {
        check_revision(revision)?;
    }

    run_save(
        env,
        runner,
        server,
        "held skip",
        held_skip_args(held_id, revision),
        None,
    )
    .await
    .map_err(|refusal| refusal.error)?;

    tracing::info!("a held memory save was skipped");
    Ok(())
}

/// `invalidInput` unless `id` is a held save's id (a lowercase UUID).
fn check_held_id(id: &str) -> Result<()> {
    if is_chat_key(id) {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "a held save id is a lowercase UUID".to_owned(),
    ))
}

/// `invalidInput` unless `revision` is a held save's revision (64 lowercase hex digits).
fn check_revision(revision: &str) -> Result<()> {
    if revision.len() == 64 && revision.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "a held save revision is 64 lowercase hex digits".to_owned(),
    ))
}

/// What the held saves in `data_dir` look like now: the chat key, name, size and modification
/// time of each `held/<chatKey>/<heldId>.json`, sorted; empty while the folder is missing or not
/// a folder. Symbolic links are never followed, and files of the old flat layout
/// (`held/<heldId>.json`) and other names are left out. Two different stamps mean the held saves
/// changed.
pub fn held_stamp(data_dir: &Path) -> Vec<(String, u64, Option<SystemTime>)> {
    let mut stamp = Vec::new();

    for chat in private_entries(&data_dir.join(HELD_DIR)) {
        let Some(key) = chat.file_name().into_string().ok().filter(|key| is_chat_key(key)) else {
            continue;
        };

        // `DirEntry::file_type` does not follow a symbolic link.
        if !chat.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }

        for entry in private_entries(&chat.path()) {
            let Some(name) = entry.file_name().into_string().ok() else {
                continue;
            };

            let is_held = name.strip_suffix(".json").is_some_and(is_chat_key);
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if is_held && metadata.is_file() {
                stamp.push((format!("{key}/{name}"), metadata.len(), metadata.modified().ok()));
            }
        }
    }

    stamp.sort();
    stamp
}

/// The entries of folder `dir`; none while it is missing, a symbolic link or not a folder.
fn private_entries(dir: &Path) -> Vec<std::fs::DirEntry> {
    let is_folder = std::fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.is_dir());

    if !is_folder {
        return Vec::new();
    }

    std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(|entry| entry.ok()).collect())
        .unwrap_or_default()
}

/// One entry of `held list` as the server writes it (camelCase).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeldFile {
    version: u32,
    held_id: String,
    chat_key: String,
    created_at: String,
    #[serde(default)]
    client: Option<String>,
    kind: HeldKind,
    tool: HeldTool,
    #[serde(default)]
    project_path: Option<String>,
    #[serde(default)]
    repo_root: Option<String>,
    #[serde(default)]
    revision: Option<String>,
    arguments: Value,
    #[serde(default)]
    current: Value,
}

/// `entry` as a held save; `None` for one this app cannot read.
fn parse_held(entry: &Value) -> Option<HeldItem> {
    let file: HeldFile = serde_json::from_value(entry.clone()).ok()?;

    if file.version != HELD_VERSION || !is_chat_key(&file.held_id) {
        return None;
    }

    Some(HeldItem {
        held_id: file.held_id,
        chat_key: file.chat_key,
        created_at: file.created_at,
        client: file.client.unwrap_or_default(),
        kind: file.kind,
        tool: file.tool,
        project_path: file.project_path,
        repo_root: file.repo_root,
        revision: file.revision,
        arguments: file.arguments,
        current: file.current,
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    const HELD: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter().map(|arg| arg.into_string().unwrap()).collect()
    }

    #[test]
    fn the_held_commands_name_the_chat_and_the_save() {
        let revision = "a".repeat(64);

        assert_eq!(
            strings(held_list_args(HELD)),
            ["held", "list", "--json", "--chat", HELD]
        );
        assert_eq!(strings(held_save_args(HELD, None)), ["held", "save", HELD, "--json"]);
        assert_eq!(
            strings(held_save_args(HELD, Some(&revision))),
            ["held", "save", HELD, "--json", "--expect", revision.as_str()]
        );
        assert_eq!(strings(held_skip_args(HELD, None)), ["held", "skip", HELD, "--json"]);
        assert_eq!(
            strings(held_skip_args(HELD, Some(&revision))),
            ["held", "skip", HELD, "--json", "--expect", revision.as_str()]
        );
    }

    #[test]
    fn a_held_entry_is_read_and_one_of_another_version_skipped() {
        let entry = json!({
            "version": 1, "heldId": HELD, "chatKey": HELD, "createdAt": "2026-10-21T10:00:00Z",
            "client": "omp", "kind": "note", "tool": "note_update", "projectPath": "/work/repo",
            "repoRoot": "/work/repo", "revision": "ab12", "arguments": {"id": 4, "title": "T"},
            "current": {"id": 4, "title": "Old"},
        });

        let item = parse_held(&entry).unwrap();
        assert_eq!(item.tool, HeldTool::NoteUpdate);
        assert_eq!(item.kind, HeldKind::Note);
        assert_eq!(item.current["title"], "Old");
        assert_eq!(item.client, "omp");
        assert_eq!(item.revision.as_deref(), Some("ab12"));

        let with = |field: &str, value: Value| {
            let mut changed = entry.as_object().unwrap().clone();
            changed.insert(field.to_owned(), value);
            Value::Object(changed)
        };

        assert!(parse_held(&with("version", json!(2))).is_none());
        assert!(parse_held(&with("tool", json!("note_delete"))).is_none());
        assert!(parse_held(&with("heldId", json!("../escape"))).is_none());
    }

    #[test]
    fn a_save_answer_names_what_was_saved() {
        let note: HeldSaved = serde_json::from_value(json!({"kind": "note", "id": 12})).unwrap();
        assert_eq!(note, HeldSaved::Note { id: 12 });

        let script: HeldSaved = serde_json::from_value(json!({"kind": "script", "name": "deploy"})).unwrap();
        assert_eq!(
            script,
            HeldSaved::Script {
                name: "deploy".to_owned()
            }
        );
    }

    #[test]
    fn the_stamp_changes_with_the_held_files_of_each_chat() {
        const CHAT: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

        let dir = tempfile::tempdir().unwrap();
        assert!(held_stamp(dir.path()).is_empty());

        let held = dir.path().join(HELD_DIR);
        std::fs::create_dir(&held).unwrap();
        std::fs::write(held.join(format!("{HELD}.json")), "{}").unwrap();
        assert!(held_stamp(dir.path()).is_empty(), "the old flat layout is ignored");

        let chat = held.join(CHAT);
        std::fs::create_dir(&chat).unwrap();
        std::fs::write(chat.join("notes.txt"), "ignored").unwrap();
        assert!(held_stamp(dir.path()).is_empty());

        std::fs::write(chat.join(format!("{HELD}.json")), "{}").unwrap();
        let first = held_stamp(dir.path());
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, format!("{CHAT}/{HELD}.json"));

        std::fs::write(chat.join(format!("{HELD}.json")), "{\"longer\": true}").unwrap();
        assert_ne!(held_stamp(dir.path()), first);
    }

    #[cfg(unix)]
    #[test]
    fn the_stamp_never_follows_a_symbolic_link() {
        const LINKED: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

        let dir = tempfile::tempdir().unwrap();
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        std::fs::write(elsewhere.join(format!("{HELD}.json")), "{}").unwrap();

        let held = dir.path().join(HELD_DIR);
        std::fs::create_dir(&held).unwrap();
        std::os::unix::fs::symlink(&elsewhere, held.join(LINKED)).unwrap();
        assert!(held_stamp(dir.path()).is_empty(), "a linked chat folder");

        std::fs::remove_dir_all(&held).unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(data.join(HELD_DIR).join(LINKED)).unwrap();
        std::os::unix::fs::symlink(
            elsewhere.join(format!("{HELD}.json")),
            data.join(HELD_DIR).join(LINKED).join(format!("{HELD}.json")),
        )
        .unwrap();
        assert!(held_stamp(&data).is_empty(), "a linked held file");

        let linked_root = dir.path().join("linked");
        std::fs::create_dir(&linked_root).unwrap();
        std::os::unix::fs::symlink(data.join(HELD_DIR), linked_root.join(HELD_DIR)).unwrap();
        assert!(held_stamp(&linked_root).is_empty(), "a linked held folder");
    }
}
