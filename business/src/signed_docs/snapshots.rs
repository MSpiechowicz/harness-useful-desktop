//! `<app_config_dir>/<kind>-approved/<id>.json` ([`DocKind::SNAPSHOT_FOLDER`]): a copy of the
//! approved bytes of each document (`0600`, the folder `0700` on Unix), written with the approval
//! and removed with it, so a view can show what changed in a pending edit. Written only by the app
//! window; only that diff needs the copies, so failing to keep one is logged, not an error.
//!
//! Each copy is named after its approval key: `<id>.json` for a local file, `team-<id>.json` for
//! the team copy of a kind that keeps team approvals apart (key `team:<id>`, see
//! [`DocKind::SEPARATE_TEAM_APPROVALS`]).

use std::path::{Path, PathBuf};

use super::{DocKind, TEAM_APPROVAL_PREFIX, is_approval_key, is_doc_id, sha256_hex};

/// The file name prefix of the approved copy of a team copy (`team-<id>.json`).
const TEAM_SNAPSHOT_PREFIX: &str = "team-";

/// The approved copy of the approval key `key` in `config_dir`; `None` for a malformed key.
fn snapshot_path<K: DocKind>(config_dir: &Path, key: &str) -> Option<PathBuf> {
    if !is_approval_key(key) {
        return None;
    }

    let stem = match key.strip_prefix(TEAM_APPROVAL_PREFIX) {
        Some(id) => format!("{TEAM_SNAPSHOT_PREFIX}{id}"),
        None => key.to_owned(),
    };

    Some(config_dir.join(K::SNAPSHOT_FOLDER).join(format!("{stem}.json")))
}
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::security::SafeReader;

/// Keeps `bytes` as the approved copy of the approval key `key` (`0600`, replaced in one rename,
/// the folder `0700` on Unix). A failure is logged.
pub(crate) fn save_snapshot<K: DocKind>(config_dir: &Path, key: &str, bytes: &[u8]) {
    let dir = config_dir.join(K::SNAPSHOT_FOLDER);
    let Some(path) = snapshot_path::<K>(config_dir, key) else {
        return;
    };

    let saved = create_private_dir(&dir, &format!("creating the approved {} folder", K::PLURAL))
        .and_then(|()| refuse_link(&path))
        .and_then(|()| replace_atomically(&path, bytes, &format!("saving an approved {}", K::NOUN)));

    if let Err(error) = saved {
        tracing::warn!(%error, "the approved copy of a {} was not kept", K::NOUN);
    }
}

/// The approval keys of the approved copies kept in `config_dir` (`<id>` and `team:<id>`).
pub(crate) fn snapshot_ids<K: DocKind>(config_dir: &Path) -> Vec<String> {
    let dir = config_dir.join(K::SNAPSHOT_FOLDER);
    let mut keys = json_ids(&dir);

    let team = json_stems(&dir).filter_map(|stem| {
        let id = stem.strip_prefix(TEAM_SNAPSHOT_PREFIX)?;
        is_doc_id(id).then(|| format!("{TEAM_APPROVAL_PREFIX}{id}"))
    });
    keys.extend(team);

    keys
}

/// The id of every `<id>.json` entry in `dir`, whatever its type (none when it cannot be listed).
pub(crate) fn json_ids(dir: &Path) -> Vec<String> {
    json_stems(dir).filter(|id| is_doc_id(id)).collect()
}

/// The stem of every `<stem>.json` entry in `dir` (none when it cannot be listed).
fn json_stems(dir: &Path) -> impl Iterator<Item = String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".json").map(str::to_owned))
}

/// Drops the approved copy of the approval key `key`.
pub(crate) fn remove_snapshot<K: DocKind>(config_dir: &Path, key: &str) {
    let Some(path) = snapshot_path::<K>(config_dir, key) else {
        return;
    };

    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, "the approved copy of a {} was not removed", K::NOUN),
    }
}

/// The approved version of document `id`: the kept copy in `config_dir`, when it still hashes to
/// `approved_sha256` and parses ([`DocKind::parse`]).
pub fn approved_doc<K: DocKind>(config_dir: &Path, id: &str, approved_sha256: &str) -> Option<K::Doc> {
    let bytes = approved_bytes::<K>(config_dir, id, approved_sha256)?;
    K::parse(id, &bytes).ok()
}

/// The bytes of the kept copy of the approval key `key`, when they hash to `approved_sha256`.
pub(crate) fn approved_bytes<K: DocKind>(config_dir: &Path, key: &str, approved_sha256: &str) -> Option<Vec<u8>> {
    let path = snapshot_path::<K>(config_dir, key)?;
    let text = SafeReader::read_to_string(&path, K::MAX_FILE_BYTES as u64).ok()?;

    (sha256_hex(text.as_bytes()) == approved_sha256).then(|| text.into_bytes())
}
