//! The last notes and scripts pages read from the team database, kept on this computer so the
//! Memory view still shows them while the database cannot be reached.
//!
//! While it is unreachable `harness-memory` lists this computer's own copy instead, which holds
//! none of the team's entries and says so with `"offline": true`. [`settle`] keeps every page read
//! live, and answers an offline one with the page last read for the same request (the same
//! repositories, search, filters, sort and page), marked `offline` with the time it was read
//! (`cachedAt`); a request never read live stays the empty offline page.
//!
//! One file per request (and per note read), `memory-cache/<kind>/<sha256 of the request>.json` in the app's
//! config folder: `{"version": 1, "savedAt", "value"}`, private to this user (`0700`/`0600`),
//! written in one rename. At most [`MAX_FILES`] per kind stay, the oldest go first. A file that
//! cannot be read is no copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::memory_cli::Repos;
use super::memory_notes::{NotesQuery, get_note, list_notes};
use super::memory_requests::memory_data_dir;
use super::memory_scripts::{MemoryScripts, ScriptsQuery};
use crate::agent::rfc3339;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::model::{MemoryNotesPage, MemoryScriptsAvailability, MemoryScriptsPage, NoteDetail};
use crate::process::ProcessRunner;
use crate::security::SafeReader;

/// The cache folder in the app's config folder.
pub const CACHE_DIR: &str = "memory-cache";

/// The file the server leaves in its data folder while the team database cannot be reached.
const UNREACHABLE_FILE: &str = "team-unreachable.json";

/// The file format written and read.
const VERSION: u32 = 1;

/// The most requests kept per kind.
const MAX_FILES: usize = 200;

/// The largest cache file read (8 MiB: a page holds 200 entries at most).
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// [`list_notes`], with the page kept on this computer (see [`settle`]).
pub async fn list_notes_cached(
    config_dir: &Path,
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    project: Repos<'_>,
    query: &NotesQuery,
) -> MemoryNotesPage {
    let page = list_notes(env, probe, runner, project, query).await;

    settle(config_dir, &format!("{project:?}{query:?}"), page)
}

/// [`MemoryScripts::list`], with the page kept on this computer (see [`settle`]).
pub async fn list_scripts_cached(
    config_dir: &Path,
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    project: Repos<'_>,
    query: &ScriptsQuery,
) -> MemoryScriptsPage {
    let page = probe.list(env, runner, project, query).await;

    settle(config_dir, &format!("{project:?}{query:?}"), page)
}

/// A list page that can be kept and answered again.
pub(crate) trait ListPage: Serialize + DeserializeOwned {
    /// The folder of this kind of page in [`CACHE_DIR`].
    const KIND: &'static str;

    /// Whether the entries of the page were read: not an `outdated`, `notInstalled` or `failed` page.
    fn readable(&self) -> bool;

    /// Whether the page came from this computer's copy, with no team entries.
    fn offline(&self) -> bool;

    /// Makes this the copy read at `saved_at` while the database is unreachable.
    fn into_cached(self, saved_at: String) -> Self;
}

/// `ListPage` for a page type with `availability`, `offline` and `cached_at` fields.
macro_rules! list_page {
    ($page:ty, $kind:literal) => {
        impl ListPage for $page {
            const KIND: &'static str = $kind;

            fn readable(&self) -> bool {
                self.availability == MemoryScriptsAvailability::Available
            }

            fn offline(&self) -> bool {
                self.offline
            }

            fn into_cached(self, saved_at: String) -> Self {
                Self {
                    offline: true,
                    cached_at: Some(saved_at),
                    ..self
                }
            }
        }
    };
}

list_page!(MemoryNotesPage, "notes");
list_page!(MemoryScriptsPage, "scripts");

/// `page`, read for `request` (any text that tells requests apart), as it should be shown: a live
/// page is kept and returned, an offline one is replaced by the copy kept for `request` when
/// there is one, anything else is returned as it is. Keeping a copy never fails the call: a
/// failure is logged.
pub(crate) fn settle<P: ListPage>(config_dir: &Path, request: &str, page: P) -> P {
    if page.offline() {
        return match recall::<P>(config_dir, P::KIND, request) {
            Some((copy, saved_at)) => copy.into_cached(saved_at),
            None => page,
        };
    }

    if page.readable() {
        keep(config_dir, P::KIND, request, &page);
    }

    page
}

/// [`get_note`], with the note kept on this computer. While the team database is unreachable
/// (the server leaves `team-unreachable.json` in its data folder) a note it cannot find is the
/// copy kept when it was last read, if any.
pub async fn get_note_cached(
    config_dir: &Path,
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    id: i64,
    project: Repos<'_>,
    hidden: bool,
) -> Result<NoteDetail> {
    let request = format!("{id}{project:?}{hidden}");

    match get_note(env, probe, runner, id, project, hidden).await {
        Ok(detail) => {
            keep(config_dir, "note", &request, &detail);

            Ok(detail)
        }
        Err(error @ (BusinessError::NotFound(_) | BusinessError::Process(_))) if team_unreachable(env) => {
            recall::<NoteDetail>(config_dir, "note", &request)
                .map(|(detail, _)| detail)
                .ok_or(error)
        }
        Err(error) => Err(error),
    }
}

/// Whether the server says the team database cannot be reached now.
fn team_unreachable(env: &Env) -> bool {
    memory_data_dir(env).is_some_and(|dir| dir.join(UNREACHABLE_FILE).is_file())
}

fn file_of(config_dir: &Path, kind: &str, request: &str) -> PathBuf {
    let digest = Sha256::digest(request.as_bytes());
    let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();

    config_dir.join(CACHE_DIR).join(kind).join(format!("{name}.json"))
}

/// The value kept for `request` and when it was kept.
fn recall<T: DeserializeOwned>(config_dir: &Path, kind: &str, request: &str) -> Option<(T, String)> {
    let text = SafeReader::read_to_string(&file_of(config_dir, kind, request), MAX_FILE_BYTES).ok()?;
    let mut file: Value = serde_json::from_str(&text).ok()?;

    if file["version"] != VERSION {
        return None;
    }

    let saved_at = file["savedAt"].as_str()?.to_owned();
    let value = serde_json::from_value(file["value"].take()).ok()?;

    Some((value, saved_at))
}

/// Keeps `value` for `request`; a failure is logged, never returned.
fn keep<T: Serialize>(config_dir: &Path, kind: &str, request: &str, value: &T) {
    if let Err(error) = write(config_dir, kind, request, value) {
        tracing::warn!(kind, %error, "memory list cache: nothing was kept");
    }
}

fn write<T: Serialize>(config_dir: &Path, kind: &str, request: &str, value: &T) -> Result<()> {
    let path = file_of(config_dir, kind, request);
    let dir = config_dir.join(CACHE_DIR).join(kind);

    create_private_dir(&dir, "the memory list cache folder")?;
    refuse_link(&path)?;

    let file = serde_json::json!({
        "version": VERSION,
        "savedAt": rfc3339(SystemTime::now()),
        "value": value,
    });
    let bytes =
        serde_json::to_vec(&file).map_err(|error| BusinessError::Internal(format!("memory list cache: {error}")))?;

    replace_atomically(&path, &bytes, "the memory list cache")?;
    prune(&dir);

    Ok(())
}

/// Removes the oldest files of `dir` beyond [`MAX_FILES`].
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "json"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();

    if files.len() <= MAX_FILES {
        return;
    }

    files.sort();

    for (_, path) in &files[..files.len() - MAX_FILES] {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests;
