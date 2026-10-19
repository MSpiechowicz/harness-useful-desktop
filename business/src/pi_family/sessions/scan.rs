//! `sessions_list`: summaries of the session files in one project's session folder.
//!
//! Only top-level `*.jsonl` files count (companion directories hold subagent transcripts, tool
//! logs and drafts and are never entered); symlinks, hidden and deny-listed files and names
//! without a usable session id are skipped. The newest [`MAX_SESSION_FILES`] files by
//! modification time are summarized from at most [`SUMMARY_BYTES`] of each: the OMP title slot,
//! the header, the first user prompt, the latest PI `session_info` name or OMP `title_change`,
//! and the latest model. Sessions without a title and without any message (drafts, `/new`
//! stubs) are left out, as in the harness pickers. Summaries are cached by path, size and
//! modification time in a [`SessionCache`].
//!
//! A summary is flagged an empty copy when its file is a bare copy (see [`copy_parent`]) whose
//! parent is among the folder's session files. Whether a file is a bare copy is decided in the
//! same read for files of at most [`SUMMARY_BYTES`] and from a [`TAIL_BYTES`] tail otherwise.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use serde_json::Value;

use super::parse::{Lines, epoch_millis, preview_text};
use super::session_id_of;
use crate::agent::rfc3339;
use crate::model::{HarnessId, MAX_FIRST_PROMPT_CHARS, SessionRef, SessionSummary, SessionTitleSource};
use crate::security::{SafeReader, is_denied};

/// Session files summarized at most per listing (the newest ones).
pub const MAX_SESSION_FILES: usize = 500;

/// Bytes read at most per file for its summary.
pub const SUMMARY_BYTES: u64 = 4 * 1024 * 1024;

/// Bytes read from the end of a file larger than [`SUMMARY_BYTES`] to tell whether it is a bare
/// copy.
const TAIL_BYTES: u64 = 256 * 1024;

/// Directory entries examined at most per listing.
const MAX_DIR_ENTRIES: usize = 100_000;

/// Cached summaries kept at most; the cache is cleared when it grows past this.
const MAX_CACHED: usize = 4096;

/// Summaries keyed by file, valid while the file's size and modification time are unchanged.
/// Owned by the caller (the desktop keeps one in its state); safe to share between threads.
#[derive(Debug, Default)]
pub struct SessionCache {
    entries: Mutex<HashMap<PathBuf, Cached>>,
}

#[derive(Debug, Clone)]
struct Cached {
    size: u64,
    modified: SystemTime,
    summary: Option<SessionSummary>,
    /// The file is a bare copy; the listing still checks that its parent exists.
    bare_copy: bool,
}

impl SessionCache {
    fn get(&self, path: &Path, size: u64, modified: SystemTime) -> Option<Cached> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let cached = entries.get(path)?;

        (cached.size == size && cached.modified == modified).then(|| cached.clone())
    }

    fn put(&self, path: PathBuf, cached: Cached) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);

        if entries.len() >= MAX_CACHED {
            entries.clear();
        }

        entries.insert(path, cached);
    }
}

/// A candidate session file.
struct SessionFile {
    path: PathBuf,
    id: String,
    size: u64,
    modified: SystemTime,
}

/// Summaries of the sessions in `dir`, newest first; an unreadable folder lists nothing.
pub fn list_sessions(harness: HarnessId, dir: &Path, cache: &SessionCache) -> Vec<SessionSummary> {
    let mut files = session_files(dir);
    let ids: HashSet<String> = files.iter().map(|file| file.id.clone()).collect();

    files.sort_by(|left, right| right.modified.cmp(&left.modified).then(right.path.cmp(&left.path)));
    files.truncate(MAX_SESSION_FILES);

    files
        .into_iter()
        .filter_map(|file| {
            let cached = cache.get(&file.path, file.size, file.modified).unwrap_or_else(|| {
                let cached = summarize(harness, &file);
                cache.put(file.path.clone(), cached.clone());
                cached
            });

            let mut summary = cached.summary?;
            summary.empty_copy = cached.bare_copy
                && summary
                    .parent_session
                    .as_ref()
                    .is_some_and(|parent| *parent != file.id && ids.contains(parent));

            Some(summary)
        })
        .collect()
}

/// The parent id of the session file at `path` when the file is a bare copy: its header names a
/// `parentSession`, it has at least one `session_exit` record, and every other entry predates
/// the header time (the copy time). Unreadable files, entries without a time and unknown tails
/// count as content.
pub(super) fn copy_parent(path: &Path) -> Option<String> {
    let size = fs::symlink_metadata(path).ok()?.len();
    let facts = read_facts(path)?;
    let parent = facts
        .header
        .as_ref()?
        .get("parentSession")
        .and_then(Value::as_str)
        .and_then(parent_id)?;

    is_bare_copy(&facts, path, size).then_some(parent)
}

fn is_bare_copy(facts: &Facts, path: &Path, size: u64) -> bool {
    if size <= SUMMARY_BYTES {
        return facts.copy.is_bare();
    }

    tail_is_bare(path, facts.copy.copied_at).unwrap_or(false)
}

/// Whether the whole lines of the last [`TAIL_BYTES`] of `path` show a bare copy; `None` when
/// the tail cannot be read. A tail of `session_exit` records alone is unknown, so not bare.
fn tail_is_bare(path: &Path, copied_at: Option<i64>) -> Option<bool> {
    let mut file = SafeReader::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TAIL_BYTES))).ok()?;

    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;

    // The first line is cut unless the tail starts the file.
    let whole = if length > TAIL_BYTES {
        let start = bytes.iter().position(|byte| *byte == b'\n')? + 1;
        &bytes[start..]
    } else {
        &bytes[..]
    };

    let mut check = CopyCheck {
        copied_at,
        ..CopyCheck::default()
    };

    for line in whole.split(|byte| *byte == b'\n') {
        let Ok(value) = serde_json::from_slice::<Value>(line.trim_ascii_end()) else {
            continue;
        };

        let kind = value.get("type").and_then(Value::as_str).unwrap_or_default();
        if !matches!(kind, "title" | "session") {
            check.note(kind, &value);
        }
    }

    Some(check.content > 0 && check.is_bare())
}

/// The top-level session files of `dir` (see the module docs).
fn session_files(dir: &Path) -> Vec<SessionFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files = Vec::new();

    for entry in entries.take(MAX_DIR_ENTRIES).flatten() {
        // `file_type` does not follow symlinks, so linked files are skipped.
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }

        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        if name.starts_with('.') || is_denied(&path).is_some() {
            continue;
        }

        let (Some(id), Ok(metadata)) = (session_id_of(&name), entry.metadata()) else {
            continue;
        };

        files.push(SessionFile {
            path,
            id,
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }

    files
}

/// What the first part of a session file says about it.
#[derive(Default)]
struct Facts {
    header: Option<Value>,
    slot_title: Option<(String, Option<SessionTitleSource>)>,
    latest_title: Option<(Option<String>, SessionTitleSource)>,
    first_prompt: Option<String>,
    model_change: Option<String>,
    assistant_model: Option<String>,
    has_messages: bool,
    copy: CopyCheck,
}

/// Tracks whether the entries of a session file show a bare copy (see [`copy_parent`]).
#[derive(Default)]
struct CopyCheck {
    /// The header time.
    copied_at: Option<i64>,
    /// `session_exit` records seen.
    exits: usize,
    /// Other entries seen.
    content: usize,
    /// True once another entry has no time or one at or after `copied_at`.
    late: bool,
}

impl CopyCheck {
    fn note(&mut self, kind: &str, value: &Value) {
        let custom_type = value.get("customType").and_then(Value::as_str);

        if kind == "custom" && custom_type == Some("session_exit") {
            self.exits += 1;
            return;
        }

        let time = value.get("timestamp").and_then(Value::as_str).and_then(epoch_millis);
        let predates = matches!((time, self.copied_at), (Some(time), Some(copied_at)) if time < copied_at);

        self.content += 1;
        self.late |= !predates;
    }

    fn is_bare(&self) -> bool {
        self.copied_at.is_some() && self.exits > 0 && !self.late
    }
}

/// The cache entry of `file`: its summary (`empty_copy` still unset) and whether it is a bare
/// copy.
fn summarize(harness: HarnessId, file: &SessionFile) -> Cached {
    let mut cached = Cached {
        size: file.size,
        modified: file.modified,
        summary: None,
        bare_copy: false,
    };

    let Some(facts) = read_facts(&file.path) else {
        return cached;
    };

    let has_parent = facts
        .header
        .as_ref()
        .is_some_and(|header| header.get("parentSession").is_some());
    cached.bare_copy = has_parent && is_bare_copy(&facts, &file.path, file.size);
    cached.summary = summary_of(harness, file, facts);

    cached
}

fn summary_of(harness: HarnessId, file: &SessionFile, facts: Facts) -> Option<SessionSummary> {
    let header = facts.header.as_ref()?;
    let text = |key: &str| header.get(key).and_then(Value::as_str).map(str::to_owned);

    let header_title = text("title").map(|title| (title, title_source(header.get("titleSource"))));
    let (title, title_source) = match (&facts.slot_title, &facts.latest_title) {
        (Some((title, source)), _) => (Some(title.clone()), *source),
        (None, Some((title, source))) => (title.clone(), Some(*source)),
        (None, None) => header_title.map_or((None, None), |(title, source)| (Some(title), source)),
    };
    let title = title.filter(|title| !title.trim().is_empty());

    if title.is_none() && !facts.has_messages {
        return None;
    }

    let updated_at = rfc3339(file.modified);

    Some(SessionSummary {
        session: SessionRef {
            harness,
            id: file.id.clone(),
        },
        title_source: title.as_ref().and(title_source),
        title,
        cwd: text("cwd").unwrap_or_default(),
        created_at: text("timestamp").unwrap_or_else(|| updated_at.clone()),
        updated_at,
        size_bytes: file.size,
        model: facts.model_change.or(facts.assistant_model),
        first_prompt: facts.first_prompt,
        parent_session: text("parentSession").and_then(|parent| parent_id(&parent)),
        empty_copy: false,
        routine: None,
    })
}

/// Reads the facts from at most [`SUMMARY_BYTES`]; `None` when there is no session header.
fn read_facts(path: &Path) -> Option<Facts> {
    let mut facts = Facts::default();

    for (number, line) in Lines::open(path, SUMMARY_BYTES).ok()?.enumerate() {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        let kind = value.get("type").and_then(Value::as_str).unwrap_or_default();

        if facts.header.is_none() {
            match lead(number, kind) {
                Lead::Slot => facts.slot_title = slot_title(&value),
                Lead::Header => {
                    facts.copy.copied_at = value.get("timestamp").and_then(Value::as_str).and_then(epoch_millis);
                    facts.header = Some(value);
                }
                Lead::Other => return None,
            }
            continue;
        }

        facts.copy.note(kind, &value);
        note_entry(&mut facts, kind, &value);
    }

    facts.header.is_some().then_some(facts)
}

/// A line before the session header.
enum Lead {
    /// The OMP title slot.
    Slot,
    /// The session header.
    Header,
    /// Anything else: the file is not a session.
    Other,
}

/// The header is the first line, or the second after an OMP title slot.
fn lead(number: usize, kind: &str) -> Lead {
    match (number, kind) {
        (0, "title") => Lead::Slot,
        (0 | 1, "session") => Lead::Header,
        _ => Lead::Other,
    }
}

/// True when the file at `path` starts with the session header of `id`, by the listing's rule.
pub(super) fn has_session_header(path: &Path, id: &str) -> bool {
    let Ok(lines) = Lines::open(path, SUMMARY_BYTES) else {
        return false;
    };

    // Only the first two lines can hold the header.
    for (number, line) in lines.enumerate().take(2) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        let kind = value.get("type").and_then(Value::as_str).unwrap_or_default();

        match lead(number, kind) {
            Lead::Slot => {}
            Lead::Header => return value.get("id").and_then(Value::as_str) == Some(id),
            Lead::Other => return false,
        }
    }

    false
}

fn note_entry(facts: &mut Facts, kind: &str, value: &Value) {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);

    match kind {
        "message" => {
            let message = value.get("message").unwrap_or(&Value::Null);
            let role = message.get("role").and_then(Value::as_str).unwrap_or_default();
            facts.has_messages |= matches!(role, "user" | "assistant");

            if role == "user" && facts.first_prompt.is_none() {
                facts.first_prompt = preview_text(message.get("content"), MAX_FIRST_PROMPT_CHARS);
            }

            if role == "assistant"
                && let Some(model) = message.get("model").and_then(Value::as_str)
            {
                facts.assistant_model = Some(model.to_owned());
            }
        }
        "session_info" => facts.latest_title = Some((text("name"), SessionTitleSource::User)),
        "title_change" => {
            let source = title_source(value.get("source")).unwrap_or(SessionTitleSource::Auto);
            facts.latest_title = Some((text("title"), source));
        }
        "model_change" => {
            let role = value.get("role").and_then(Value::as_str).unwrap_or("default");

            // PI records `modelId`; OMP records `provider/modelId` per role.
            let model = text("modelId").or_else(|| {
                text("model").map(|model| model.split_once('/').map_or(model.clone(), |(_, id)| id.to_owned()))
            });

            if role == "default" && model.is_some() {
                facts.model_change = model;
            }
        }
        _ => {}
    }
}

fn slot_title(value: &Value) -> Option<(String, Option<SessionTitleSource>)> {
    let title = value.get("title").and_then(Value::as_str)?;
    Some((title.to_owned(), title_source(value.get("source"))))
}

fn title_source(value: Option<&Value>) -> Option<SessionTitleSource> {
    match value?.as_str()? {
        "auto" => Some(SessionTitleSource::Auto),
        "user" => Some(SessionTitleSource::User),
        _ => None,
    }
}

/// The session id of a `parentSession` value: a session file path (its file name's id) or a
/// bare id. Paths are never returned.
fn parent_id(parent: &str) -> Option<String> {
    let name = parent.rsplit(['/', '\\']).next().unwrap_or(parent);

    if name.ends_with(".jsonl") {
        return session_id_of(name);
    }

    super::is_safe_session_id(name).then(|| name.to_owned())
}
