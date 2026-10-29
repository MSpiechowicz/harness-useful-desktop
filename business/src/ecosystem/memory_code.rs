//! The code index of Harness Memory: the switch for it, and the builds the app announces.
//!
//! The server indexes a repository's code the first time an agent asks (`code_map`, `code_symbol`,
//! `code_impact`, `code_path`) and keeps the index fresh on every call. The app has two parts in it:
//!
//! - the switch: `code status|enable|disable --json` prints `{"enabled", "source"}`
//!   ([`get_code_index`], [`set_code_index`]; the server keeps the choice in `code.toml` of its data
//!   folder, so its own tools agree with the app). Each asks for the server through the
//!   `code-index` feature ([`feature_server`](super::memory_embedder::feature_server));
//! - the builds: while the server builds a whole repository (`full`) or a large change (`bulk`) it
//!   keeps `code-builds/<first 16 hex of sha256(repository root)>.json` in its data folder
//!   (`0600`, rewritten atomically), and [`CodeBuildWatcher`] turns the changes of those files into
//!   [`CodeBuildEvent`]s for a toast.
//!
//! A status file is `{"version": 1, "buildId", "root", "repoName", "kind": "full|bulk", "phase":
//! "building|ready|error", "filesTotal", "filesDone", "startedAt", "finishedAt", "error"}`. The
//! watcher ignores a name that is not 16 lowercase hex characters and `.json`, a file larger than
//! [`MAX_STATUS_BYTES`], older than [`MAX_STATUS_AGE`], not a regular file, unreadable or unknown,
//! and everything in a folder other users may change (Unix). It never writes.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::Value;

use super::memory_binary::resolve_memory;
use super::memory_cli::{CliCall, LIST_OUTPUT_BYTES, LIST_TIMEOUT, clean, run_json};
use super::memory_features::MemoryFeatures;
use super::memory_requests::{memory_data_dir, unsafe_folder};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{CodeBuildEvent, CodeIndexState};
use crate::process::ProcessRunner;
use crate::security::SafeReader;
use crate::signed_docs::unicode::is_hidden_char;

/// The folder of build status files in the server's data folder.
pub const BUILDS_DIR: &str = "code-builds";

/// The largest status file read (4 KiB; a real one is under 400 bytes).
pub const MAX_STATUS_BYTES: u64 = 4 * 1024;

/// A status file older than this is not shown (15 min; the server deletes finished ones after
/// 10 min, and `building` ones nobody touched for 10 min).
pub const MAX_STATUS_AGE: Duration = Duration::from_secs(15 * 60);

/// A build that reported nothing for this long is `stale` (the server reports at least twice a
/// second while it works).
pub const STALE_AFTER: Duration = Duration::from_secs(60);

/// A build finished before the watcher saw it is announced only when it finished this recently,
/// so opening the app does not announce what happened while it was closed.
const FRESH_FINISH: Duration = Duration::from_secs(30);

/// The most status files followed at once (a computer has a handful of repositories).
const MAX_FILES: usize = 64;

/// The status file format this app reads.
const STATUS_VERSION: u32 = 1;

/// The longest repository root and name kept, in characters.
const MAX_PATH_CHARS: usize = 1024;

/// Progress is announced in steps of this many percent.
const PROGRESS_STEP: u64 = 10;

/// `code status --json`: whether the code index is on, and where the switch comes from.
/// `protocol` when the answer is not what the server prints.
pub async fn get_code_index(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<CodeIndexState> {
    let data = run_json(env, runner, server, code_call("code status", "status")).await?;

    read_state(&data, "code status")
}

/// What the hooks need to know about the code graph of the installed Harness Memory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CodeGraphSupport {
    /// The server lists the `code-index` feature (so it has the `code_*` tools).
    pub listed: bool,
    /// The Code index switch is on. Asked only when `listed`, and false when the server cannot say.
    pub on: bool,
}

/// Looks the code graph of the installed server up: the `code-index` feature, then its switch.
pub async fn code_graph_support(env: &Env, runner: &dyn ProcessRunner, features: &MemoryFeatures) -> CodeGraphSupport {
    let Some(server) = resolve_memory(env) else {
        return CodeGraphSupport::default();
    };

    if !features.support(env, runner).await.code_index() {
        return CodeGraphSupport::default();
    }

    let on = get_code_index(env, runner, &server.path)
        .await
        .is_ok_and(|state| state.enabled);

    CodeGraphSupport { listed: true, on }
}

/// The settings file of the switch in the server's data folder.
const SWITCH_FILE: &str = "code.toml";

/// The variable that overrides the file in the server's environment (`0` or `1`).
const SWITCH_VAR: &str = "HARNESS_MEMORY_CODE_INDEX";

/// The most bytes read from the settings file (the server reads 4 KiB).
const SWITCH_FILE_BYTES: u64 = 4 * 1024;

/// Whether the Code index switch is on right now, read from where the server keeps it, without
/// starting a process: `HARNESS_MEMORY_CODE_INDEX` (`0` or `1`) of this app's environment, which
/// the agents and so their servers inherit, wins; else `enabled = bool` in `code.toml` of the
/// server's data folder. A missing file, or one without the key, is on. A file that cannot be read,
/// is not a regular file, is not valid TOML or has a wrong type is off, as the server treats it.
/// On when the data folder is unknown (the answer of `code status` stands then).
pub fn code_switch_is_on(env: &Env) -> bool {
    match env.var(SWITCH_VAR).map(str::trim) {
        Some("0") => return false,
        Some("1") => return true,
        _ => {}
    }

    let Some(folder) = memory_data_dir(env) else {
        return true;
    };

    let path = folder.join(SWITCH_FILE);

    match std::fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
    }

    let Ok(text) = SafeReader::read_to_string(&path, SWITCH_FILE_BYTES) else {
        return false;
    };

    let Ok(table) = toml::from_str::<toml::Table>(&text) else {
        return false;
    };

    match table.get("enabled") {
        None => true,
        Some(toml::Value::Boolean(enabled)) => *enabled,
        Some(_) => false,
    }
}

/// `code enable --json` or `code disable --json`: switches the index and answers the state the
/// server reports afterwards. `process` with the server's line when it cannot write the choice.
pub async fn set_code_index(env: &Env, runner: &dyn ProcessRunner, server: &Path, on: bool) -> Result<CodeIndexState> {
    let (name, command) = if on {
        ("code enable", "enable")
    } else {
        ("code disable", "disable")
    };

    let data = run_json(env, runner, server, code_call(name, command)).await?;
    let state = read_state(&data, name)?;

    tracing::info!(enabled = state.enabled, source = %state.source, "code index switched");
    Ok(state)
}

fn code_call<'a>(name: &'a str, command: &str) -> CliCall<'a> {
    CliCall {
        name,
        args: ["code", command, "--json"].map(OsString::from).into(),
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    }
}

fn read_state(data: &Value, name: &str) -> Result<CodeIndexState> {
    let Some(enabled) = data.get("enabled").and_then(Value::as_bool) else {
        return Err(BusinessError::Protocol(format!(
            "harness-memory {name} answered without `enabled`"
        )));
    };

    let source = data
        .get("source")
        .and_then(Value::as_str)
        .filter(|source| matches!(*source, "default" | "file" | "env"))
        .unwrap_or("default");

    Ok(CodeIndexState {
        enabled,
        source: source.to_owned(),
    })
}

/// Whether `name` is a status file: 16 lowercase hex characters and `.json`.
fn is_status_name(name: &str) -> bool {
    name.strip_suffix(".json")
        .is_some_and(|stem| stem.len() == 16 && stem.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')))
}

/// The status files of one data folder: what was announced of each, and what changed since the
/// last look.
pub struct CodeBuildWatcher {
    builds: PathBuf,
    seen: BTreeMap<String, Seen>,
    /// Files that are not shown (unreadable, too large, unknown), by size and modification time:
    /// not read again until they change.
    ignored: BTreeMap<String, (u64, Option<SystemTime>)>,
    warned_unsafe: bool,
}

/// What was announced of one status file.
struct Seen {
    event: CodeBuildEvent,
    bucket: u64,
}

/// A status file that was read.
struct Status {
    event: CodeBuildEvent,
    /// When the server last wrote it (the finish for a finished build).
    touched: SystemTime,
    finished: bool,
}

/// The status file as the server writes it (camelCase).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusFile {
    version: u32,
    build_id: i64,
    root: String,
    repo_name: String,
    kind: String,
    phase: String,
    #[serde(default)]
    files_total: u64,
    #[serde(default)]
    files_done: u64,
    #[serde(default)]
    finished_at: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

impl CodeBuildWatcher {
    /// A watcher of `<data_dir>/code-builds` that has seen nothing yet.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            builds: data_dir.join(BUILDS_DIR),
            seen: BTreeMap::new(),
            ignored: BTreeMap::new(),
            warned_unsafe: false,
        }
    }

    /// Looks at the folder once (`now` is the clock for the age limits): an event for each build
    /// that is new, changed its `phase` or moved into another 10 % step of its files since the
    /// last look, ordered by file name. A `building` file nobody wrote for [`STALE_AFTER`] is
    /// `stale`, and a build the agent cancelled is `cancelled` (not an error).
    pub fn poll(&mut self, now: SystemTime) -> Vec<CodeBuildEvent> {
        let names = self.listing();
        let mut events = Vec::new();

        self.seen.retain(|name, _| names.contains(name));
        self.ignored.retain(|name, _| names.contains(name));

        for name in names {
            let Some(status) = self.read(&name, now) else {
                self.seen.remove(&name);
                continue;
            };

            if let Some(event) = self.announce(&name, status, now) {
                events.push(event);
            }
        }

        events
    }

    /// The builds last announced as `building` (what a page that starts listening needs to show
    /// them).
    pub fn active(&self) -> Vec<CodeBuildEvent> {
        self.seen
            .values()
            .filter(|seen| seen.event.phase == "building")
            .map(|seen| seen.event.clone())
            .collect()
    }

    /// Folds `status` of file `name` into what was announced; the event to send, if any.
    fn announce(&mut self, name: &str, status: Status, now: SystemTime) -> Option<CodeBuildEvent> {
        let mut event = status.event;

        if event.phase == "building" && age(status.touched, now) >= STALE_AFTER {
            event.phase = "stale".to_owned();
        }

        let bucket = bucket_of(&event);

        let changed = match self.seen.get(name) {
            Some(seen) => {
                seen.event.build_id != event.build_id
                    || seen.event.phase != event.phase
                    || (event.phase == "building"
                        && (seen.bucket != bucket || seen.event.files_total != event.files_total))
            }
            // A build that ended before the watcher looked is news only when it ended just now.
            None => !status.finished || age(status.touched, now) <= FRESH_FINISH,
        };

        let remembered = Seen {
            event: event.clone(),
            bucket,
        };
        self.seen.insert(name.to_owned(), remembered);

        changed.then_some(event)
    }

    /// The names of the status files in the folder, sorted; none while it is missing, not a real
    /// folder, or one other users may change.
    fn listing(&mut self) -> BTreeSet<String> {
        let Ok(metadata) = std::fs::symlink_metadata(&self.builds) else {
            return BTreeSet::new();
        };

        if !metadata.is_dir() {
            return BTreeSet::new();
        }

        if let Some(problem) = unsafe_folder(&metadata) {
            if !self.warned_unsafe {
                tracing::warn!(problem, "memory code builds are not read");
                self.warned_unsafe = true;
            }

            return BTreeSet::new();
        }

        let Ok(entries) = std::fs::read_dir(&self.builds) else {
            return BTreeSet::new();
        };

        entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| is_status_name(name))
            .take(MAX_FILES)
            .collect()
    }

    /// The status file `name`, when it is a regular file within the limits that parses.
    fn read(&mut self, name: &str, now: SystemTime) -> Option<Status> {
        let path = self.builds.join(name);
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        let stamp = (metadata.len(), metadata.modified().ok());

        if self.ignored.get(name) == Some(&stamp) {
            return None;
        }

        let status = read_status(&path, &metadata, now);

        if status.is_none() {
            self.ignored.insert(name.to_owned(), stamp);
        } else {
            self.ignored.remove(name);
        }

        status
    }
}

/// How long ago `time` was (zero for a time in the future).
fn age(time: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(time).unwrap_or_default()
}

/// The 10 % step of the files done (0 while the total is not known).
fn bucket_of(event: &CodeBuildEvent) -> u64 {
    if event.files_total == 0 {
        return 0;
    }

    let percent = event.files_done.min(event.files_total).saturating_mul(100) / event.files_total;

    percent / PROGRESS_STEP
}

fn read_status(path: &Path, metadata: &std::fs::Metadata, now: SystemTime) -> Option<Status> {
    if !metadata.is_file() || metadata.len() > MAX_STATUS_BYTES {
        return None;
    }

    let modified = metadata.modified().ok()?;
    let text = SafeReader::read_to_string(path, MAX_STATUS_BYTES).ok()?;
    let file: StatusFile = serde_json::from_str(&text).ok()?;

    if file.version != STATUS_VERSION {
        return None;
    }

    let finished = file.phase != "building";
    // A time too far out to represent is a broken or hostile file: it is not shown.
    let touched = match file.finished_at.filter(|_| finished) {
        Some(seconds) => UNIX_EPOCH.checked_add(Duration::from_secs(seconds))?,
        None => modified,
    };

    if age(touched, now) > MAX_STATUS_AGE {
        return None;
    }

    let event = event_of(file)?;

    Some(Status {
        event,
        touched,
        finished,
    })
}

/// The event for `file`; `None` for a kind or phase this app does not know.
fn event_of(file: StatusFile) -> Option<CodeBuildEvent> {
    if !matches!(file.kind.as_str(), "full" | "bulk") {
        return None;
    }

    let cancelled = file.phase == "error" && file.error.as_deref() == Some("cancelled");

    let (phase, error) = match file.phase.as_str() {
        "building" => ("building", None),
        "ready" => ("ready", None),
        "error" if cancelled => ("cancelled", None),
        "error" => (
            "error",
            Some(clean(&clip(&one_line(file.error.as_deref().unwrap_or_default())))),
        ),
        _ => return None,
    };

    Some(CodeBuildEvent {
        build_id: file.build_id,
        root: clip(&file.root),
        repo_name: clip(&file.repo_name),
        kind: file.kind,
        phase: phase.to_owned(),
        files_total: file.files_total,
        files_done: file.files_done,
        error,
    })
}

/// `text` without control characters and the characters that draw nothing but can reorder or hide
/// the text around them ([`is_hidden_char`]), at most [`MAX_PATH_CHARS`] long.
fn clip(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() && !is_hidden_char(*character))
        .take(MAX_PATH_CHARS)
        .collect()
}

/// `text` with every run of white space (new lines included) as one space.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
