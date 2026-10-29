//! Asking the user before Harness Memory runs a script this machine has not approved.
//!
//! The server and the app share files in the server's data folder ([`memory_data_dir`]), which the
//! server creates (`0700`):
//!
//! - `desktop.alive` ([`ALIVE_FILE`]): the app touches it every [`HEARTBEAT_INTERVAL`] while it
//!   runs ([`touch_alive`]) and removes it on a clean exit ([`remove_alive`]). A file younger than
//!   15 s tells the server someone can answer; without it the server refuses at once.
//! - `requests/<id>.json` ([`REQUESTS_DIR`]): a run request, written by the server (temp file and
//!   rename, `0600`). [`RequestWatcher`] reads new ones every [`POLL_INTERVAL`] and reports the
//!   ones that disappear.
//! - `requests/<id>.decision`: the answer, `allow` or `deny`, written by
//!   [`RequestWatcher::decide`] in one rename (`0600`), only for a request the watcher has shown
//!   and not yet answered. The server waits 45 s for it, then removes both files; an answer left
//!   without its request for longer than [`MAX_REQUEST_AGE`] (the server stopped before cleaning
//!   up) is removed by the watcher.
//!
//! The app never creates the data folder. An id is 32 lowercase hex characters; any other name in
//! the folder is ignored, and so is a request larger than [`MAX_REQUEST_BYTES`], older than
//! [`MAX_REQUEST_AGE`], unreadable, or in a folder other users may change (Unix).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use serde_json::Value;

use super::memory_scripts::parse;
use crate::agent::rfc3339;
use crate::app_dirs::shared_data_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{refuse_link, replace_atomically};
use crate::model::{ScriptPreviousVersion, ScriptRequestEvent, ScriptRunRequest, ScriptStdin};
use crate::security::SafeReader;

/// The variable naming the server's data folder instead of the platform one (see
/// [`memory_data_dir`]).
pub const DIR_VAR: &str = "HARNESS_MEMORY_DIR";

/// The heartbeat file in the data folder.
pub const ALIVE_FILE: &str = "desktop.alive";

/// The folder of run requests in the data folder.
pub const REQUESTS_DIR: &str = "requests";

/// How often the app touches [`ALIVE_FILE`] (5 s; the server trusts a file younger than 15 s).
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How often the app looks for run requests (500 ms).
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// The largest request read (512 KiB; a script body is at most 64 KiB, twice with `previous`).
pub const MAX_REQUEST_BYTES: u64 = 512 * 1024;

/// A request older than this is not shown (60 s; the server waits 45 s).
pub const MAX_REQUEST_AGE: Duration = Duration::from_secs(60);

/// The most requests shown at once; more wait in the folder until one is answered.
const MAX_PENDING: usize = 32;

/// The request file format this app reads.
const REQUEST_VERSION: u32 = 1;

/// The folder the server creates inside the platform data folder.
const APP_FOLDER: &str = "harness-memory";

/// Harness Memory's data folder, as the server finds it: [`DIR_VAR`] when set to an absolute path
/// (a leading `~` expanded; a relative one is ignored), else `$XDG_DATA_HOME` (absolute only, or
/// `~/.local/share`) on Linux, `~/Library/Application Support` on macOS and `%LOCALAPPDATA%`
/// (absolute only) on Windows, followed by `harness-memory`. `None` on Windows while
/// `LOCALAPPDATA` is unset.
pub fn memory_data_dir(env: &Env) -> Option<PathBuf> {
    shared_data_dir(env, DIR_VAR, APP_FOLDER)
}

/// Touches `<data_dir>/desktop.alive` (created `0600` when missing). `false`, with nothing written,
/// while the data folder does not exist: the server creates it. Refuses a symbolic link there.
pub fn touch_alive(data_dir: &Path) -> Result<bool> {
    if !data_dir.is_dir() {
        return Ok(false);
    }

    let path = data_dir.join(ALIVE_FILE);
    refuse_link(&path)?;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);

    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    options
        .open(&path)
        .and_then(|file| file.set_modified(SystemTime::now()))
        .map_err(|error| BusinessError::from_io("touching the Harness Memory heartbeat file", error))?;

    Ok(true)
}

/// Removes `<data_dir>/desktop.alive` (nothing to do when it is not there).
pub fn remove_alive(data_dir: &Path) -> Result<()> {
    match std::fs::remove_file(data_dir.join(ALIVE_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BusinessError::from_io(
            "removing the Harness Memory heartbeat file",
            error,
        )),
    }
}

/// Whether `id` names a run request: 32 lowercase hex characters.
pub fn is_request_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// `invalidInput` unless `id` names a run request ([`is_request_id`]).
pub fn check_request_id(id: &str) -> Result<()> {
    if is_request_id(id) {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "a run request id is 32 lowercase hex characters".to_owned(),
    ))
}

/// Writes `allow` or `deny` to `<requests>/<id>.decision` in one rename (`0600`). `notFound` when
/// the request file is gone (the server stopped waiting).
fn write_decision(requests: &Path, request_id: &str, allow: bool) -> Result<()> {
    refuse_link(requests)?;

    let request = requests.join(format!("{request_id}.json"));
    let waiting = std::fs::symlink_metadata(&request).is_ok_and(|metadata| metadata.is_file());

    if !waiting {
        return Err(BusinessError::NotFound(format!("run request {request_id}")));
    }

    let answer: &[u8] = if allow { b"allow" } else { b"deny" };

    replace_atomically(
        &requests.join(format!("{request_id}.decision")),
        answer,
        "answering a Harness Memory run request",
    )
}

/// The run requests in one data folder: what is waiting, and what changed since the last look.
pub struct RequestWatcher {
    requests: PathBuf,
    pending: BTreeMap<String, Pending>,
    /// Files that are not shown (unreadable, too large, too old, answered): not read again while
    /// they stay.
    ignored: BTreeSet<String>,
    warned_unsafe: bool,
}

struct Pending {
    request: ScriptRunRequest,
    modified: SystemTime,
}

/// The ids in the requests folder: of `<id>.json` files, and of `<id>.decision` files.
#[derive(Default)]
struct Listing {
    requests: BTreeSet<String>,
    decisions: BTreeSet<String>,
}

impl RequestWatcher {
    /// A watcher of `<data_dir>/requests` that has seen nothing yet.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            requests: data_dir.join(REQUESTS_DIR),
            pending: BTreeMap::new(),
            ignored: BTreeSet::new(),
            warned_unsafe: false,
        }
    }

    /// Looks at the folder once (`now` is the clock for the age limit): a `request` event for each
    /// new request, oldest first, and a `gone` event for each shown one whose file disappeared or
    /// grew too old.
    ///
    /// It also removes the answers whose request has been gone for longer than
    /// [`MAX_REQUEST_AGE`].
    pub fn poll(&mut self, now: SystemTime) -> Vec<ScriptRequestEvent> {
        let Listing {
            requests: present,
            decisions,
        } = self.listing();
        let mut events = Vec::new();

        for id in decisions.difference(&present) {
            self.sweep_decision(id, now);
        }

        let gone: Vec<String> = self
            .pending
            .iter()
            .filter(|(id, pending)| !present.contains(*id) || age(pending.modified, now) > MAX_REQUEST_AGE)
            .map(|(id, _)| id.clone())
            .collect();

        for id in gone {
            self.pending.remove(&id);

            if present.contains(&id) {
                self.ignored.insert(id.clone());
            }

            events.push(ScriptRequestEvent::Gone { request_id: id });
        }

        self.ignored.retain(|id| present.contains(id));

        let mut fresh = Vec::new();

        for id in present {
            if self.pending.len() + fresh.len() >= MAX_PENDING {
                break;
            }

            if self.pending.contains_key(&id) || self.ignored.contains(&id) {
                continue;
            }

            match read_request(&self.requests, &id, now) {
                Some(pending) => fresh.push((id, pending)),
                None => {
                    self.ignored.insert(id);
                }
            }
        }

        fresh.sort_by_key(|(_, pending)| pending.modified);

        for (id, pending) in fresh {
            events.push(ScriptRequestEvent::Request {
                request: Box::new(pending.request.clone()),
            });
            self.pending.insert(id, pending);
        }

        events
    }

    /// The requests shown and still waiting, oldest first.
    pub fn pending(&self) -> Vec<ScriptRunRequest> {
        let mut pending: Vec<&Pending> = self.pending.values().collect();
        pending.sort_by_key(|pending| pending.modified);

        pending.into_iter().map(|pending| pending.request.clone()).collect()
    }

    /// Answers request `request_id`, which must be one [`RequestWatcher::poll`] has shown and that
    /// is not answered yet: writes `allow` or `deny` to `requests/<id>.decision` in one rename
    /// (`0600`). The request then leaves [`RequestWatcher::pending`] and is not shown again.
    /// `invalidInput` for a malformed id, `notFound` for a request never shown, already answered,
    /// or whose file is gone (the server stopped waiting).
    pub fn decide(&mut self, request_id: &str, allow: bool) -> Result<()> {
        check_request_id(request_id)?;

        if !self.pending.contains_key(request_id) {
            return Err(BusinessError::NotFound(format!("run request {request_id}")));
        }

        write_decision(&self.requests, request_id, allow)?;

        self.pending.remove(request_id);
        self.ignored.insert(request_id.to_owned());

        Ok(())
    }

    /// The ids in the folder; none while it is missing, not a real folder, or one other users may
    /// change.
    fn listing(&mut self) -> Listing {
        let Ok(metadata) = std::fs::symlink_metadata(&self.requests) else {
            return Listing::default();
        };

        if !metadata.is_dir() {
            return Listing::default();
        }

        if let Some(problem) = unsafe_folder(&metadata) {
            if !self.warned_unsafe {
                tracing::warn!(problem, "memory run requests are not read");
                self.warned_unsafe = true;
            }

            return Listing::default();
        }

        let Ok(entries) = std::fs::read_dir(&self.requests) else {
            return Listing::default();
        };

        let mut listing = Listing::default();

        for entry in entries.filter_map(|entry| entry.ok()) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };

            if let Some(id) = name.strip_suffix(".json").filter(|id| is_request_id(id)) {
                listing.requests.insert(id.to_owned());
            } else if let Some(id) = name.strip_suffix(".decision").filter(|id| is_request_id(id)) {
                listing.decisions.insert(id.to_owned());
            }
        }

        listing
    }

    /// Removes the answer `<id>.decision`, whose request is gone, once it is older than
    /// [`MAX_REQUEST_AGE`].
    fn sweep_decision(&self, id: &str, now: SystemTime) {
        let path = self.requests.join(format!("{id}.decision"));

        let stale = std::fs::symlink_metadata(&path)
            .ok()
            .filter(|metadata| metadata.is_file())
            .and_then(|metadata| metadata.modified().ok())
            .is_some_and(|modified| age(modified, now) > MAX_REQUEST_AGE);

        if !stale {
            return;
        }

        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!("a memory run answer left without its request was removed"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(%error, "a memory run answer left without its request could not be removed"),
        }
    }
}

/// How long ago `modified` was (zero for a time in the future).
fn age(modified: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(modified).unwrap_or_default()
}

/// The request file of `id`, when it is a regular file within the limits that parses.
fn read_request(requests: &Path, id: &str, now: SystemTime) -> Option<Pending> {
    let path = requests.join(format!("{id}.json"));
    let metadata = std::fs::symlink_metadata(&path).ok()?;

    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES {
        return None;
    }

    let modified = metadata.modified().ok()?;
    if age(modified, now) > MAX_REQUEST_AGE {
        return None;
    }

    let text = SafeReader::read_to_string(&path, MAX_REQUEST_BYTES).ok()?;

    match parse_request(&text, id, now) {
        Some(request) => Some(Pending { request, modified }),
        None => {
            tracing::warn!("an unreadable memory run request was ignored");
            None
        }
    }
}

/// The request file as the server writes it (camelCase).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestFile {
    version: u32,
    request_id: String,
    created_at: String,
    #[serde(default)]
    client: Option<String>,
    script: Value,
    body: String,
    language: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    stdin: Option<ScriptStdin>,
    #[serde(default)]
    previous: Option<ScriptPreviousVersion>,
}

/// The request in `text`, which must be version 1 and name `id`, first seen at `received` (this
/// app's clock). `None` otherwise.
pub fn parse_request(text: &str, id: &str, received: SystemTime) -> Option<ScriptRunRequest> {
    let file: RequestFile = serde_json::from_str(text).ok()?;

    if file.version != REQUEST_VERSION || file.request_id != id {
        return None;
    }

    Some(ScriptRunRequest {
        request_id: file.request_id,
        created_at: file.created_at,
        received_at: rfc3339(received),
        client: file.client.unwrap_or_default(),
        script: parse(file.script, "a run request").ok()?,
        body: file.body,
        language: file.language,
        args: file.args,
        cwd: file.cwd.unwrap_or_default(),
        stdin: file.stdin,
        previous: file.previous,
    })
}

/// Why requests in a folder with `metadata` must not be trusted (Unix: other users may change it,
/// or it belongs to another user); `None` when it is private.
#[cfg(unix)]
pub(super) fn unsafe_folder(metadata: &std::fs::Metadata) -> Option<&'static str> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if metadata.permissions().mode() & 0o022 != 0 {
        return Some("other users can change the requests folder");
    }

    (metadata.uid() != nix::unistd::geteuid().as_raw()).then_some("the requests folder belongs to another user")
}

#[cfg(not(unix))]
pub(super) fn unsafe_folder(_metadata: &std::fs::Metadata) -> Option<&'static str> {
    None
}
