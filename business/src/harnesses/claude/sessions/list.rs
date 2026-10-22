//! `sessions_list` for Claude Code: summaries of the `<uuid>.jsonl` files in a project's session
//! folder, newest first.
//!
//! Only top-level regular files count (symlinks, hidden and deny-listed files, other names and
//! the per-session folders are skipped). The newest [`MAX_SESSION_FILES`] files by modification
//! time are summarized from their first [`SUMMARY_BYTES`] and, for a larger file, its last
//! [`TAIL_BYTES`] (titles are appended at the end):
//!
//! - title: the last `custom-title` line (`customTitle`, set by the user), else the rename
//!   sidecar `<uuid>/custom-title.json`, else the last `ai-title` line (`aiTitle`, generated);
//! - first prompt: the first prompt the user typed (commands, shell runs and tool results are not
//!   prompts);
//! - created: the first message's time; updated: the file's modification time;
//! - cwd: the last `relocated` line's `relocatedCwd`, else the first message's `cwd`;
//! - model: the last reply's model.
//!
//! Left out, as in Claude Code's own picker: files of other sessions (no line names the session),
//! subagent transcripts, sessions without a title and without any message, and sessions whose
//! working directory is another folder with the same slug (`/a/b-c` and `/a/b/c` share one).
//! Summaries are cached by file, size and modification time (the sidecar's too).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use serde_json::Value;

use super::entries::{Kind, kind, text};
use super::{MAX_DIR_ENTRIES, session_id_of};
use crate::agent::rfc3339;
use crate::harnesses::claude::paths::project_slug;
use crate::model::{HarnessId, MAX_FIRST_PROMPT_CHARS, SessionRef, SessionSummary, SessionTitleSource};
use crate::pi_family::sessions::{Lines, MAX_SESSION_FILES, SUMMARY_BYTES, preview_text, tail_bytes};
use crate::security::{SafeReader, is_denied};

/// Bytes read from the end of a file larger than [`SUMMARY_BYTES`].
const TAIL_BYTES: u64 = 1024 * 1024;

/// Largest rename sidecar read.
const SIDECAR_MAX_BYTES: u64 = 64 * 1024;

/// The rename sidecar's name in the session's folder.
const SIDECAR_NAME: &str = "custom-title.json";

/// Cached summaries kept at most; the cache is cleared when it grows past this.
const MAX_CACHED: usize = 4096;

/// What a summary was read from: the session file and the sidecar, by size and time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: SystemTime,
    sidecar: Option<SystemTime>,
}

/// Summaries by session file, with what they were read from.
type Summaries = HashMap<PathBuf, (Stamp, Option<SessionSummary>)>;

static CACHE: Mutex<Option<Summaries>> = Mutex::new(None);

/// The summaries of the sessions in `dir`, the session folder of `project`, newest first; an
/// unreadable folder lists nothing.
pub(in crate::harnesses::claude) fn list_sessions(dir: &Path, project: &Path) -> Vec<SessionSummary> {
    let mut files = session_files(dir);

    files.sort_by(|left, right| right.modified.cmp(&left.modified).then(right.path.cmp(&left.path)));
    files.truncate(MAX_SESSION_FILES);

    files
        .into_iter()
        .filter_map(|file| {
            let folder = dir.join(&file.id);
            let sidecar = folder.join(SIDECAR_NAME);
            let stamp = Stamp {
                size: file.size,
                modified: file.modified,
                sidecar: sidecar_time(&folder, &sidecar),
            };

            cached(&file.path, &stamp, || summarize(&file.id, &file.path, &stamp, &sidecar))
        })
        .filter(|summary| belongs_to(summary, project))
        .collect()
}

/// False when the session file `path` of session `id` in `dir`, the session folder of `project`,
/// records another folder with the same slug as its working directory (the rule of the list). A
/// file the list leaves out for another reason records none and counts as the project's.
pub(super) fn is_projects_session(dir: &Path, path: &Path, id: &str, project: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return true;
    };

    let folder = dir.join(id);
    let sidecar = folder.join(SIDECAR_NAME);
    let stamp = Stamp {
        size: metadata.len(),
        modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        sidecar: sidecar_time(&folder, &sidecar),
    };

    cached(path, &stamp, || summarize(id, path, &stamp, &sidecar)).is_none_or(|summary| belongs_to(&summary, project))
}

/// A candidate session file.
struct SessionFile {
    id: String,
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// The top-level `<uuid>.jsonl` regular files of `dir` that are not deny-listed.
fn session_files(dir: &Path) -> Vec<SessionFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .take(MAX_DIR_ENTRIES)
        .flatten()
        // `file_type` does not follow symlinks, so linked files are skipped.
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let id = entry.file_name().to_str().and_then(session_id_of)?.to_owned();
            let path = entry.path();
            let metadata = entry.metadata().ok()?;

            is_denied(&path).is_none().then(|| SessionFile {
                id,
                path,
                size: metadata.len(),
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            })
        })
        .collect()
}

/// The cached summary of `path` when `stamp` still matches, else `summarize`'s (then cached).
fn cached(path: &Path, stamp: &Stamp, summarize: impl FnOnce() -> Option<SessionSummary>) -> Option<SessionSummary> {
    {
        let cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
        let hit = cache.as_ref().and_then(|cache| cache.get(path));

        if let Some((cached_stamp, summary)) = hit
            && cached_stamp == stamp
        {
            return summary.clone();
        }
    }

    let summary = summarize();
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let entries = cache.get_or_insert_with(HashMap::new);

    if entries.len() >= MAX_CACHED {
        entries.clear();
    }

    entries.insert(path.to_path_buf(), (stamp.clone(), summary.clone()));
    summary
}

/// False for a session that ran in another folder with the same slug as `project` (canonical).
fn belongs_to(summary: &SessionSummary, project: &Path) -> bool {
    let cwd = Path::new(&summary.cwd);

    if summary.cwd.is_empty() || cwd == project || project_slug(cwd) != project_slug(project) {
        return true;
    }

    dunce::canonicalize(cwd).is_ok_and(|canonical| canonical == project)
}

/// What the lines of a session file say about it.
#[derive(Default)]
struct Facts {
    own: bool,
    sidechain: bool,
    has_messages: bool,
    first_prompt: Option<String>,
    created: Option<String>,
    cwd: Option<String>,
    relocated: Option<String>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    model: Option<String>,
}

impl Facts {
    fn note(&mut self, id: &str, line: &Value) {
        let field = |key: &str| {
            text(line, key)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };

        self.own |= text(line, "sessionId") == Some(id);

        match text(line, "type").unwrap_or_default() {
            "custom-title" => self.custom_title = field("customTitle").or(self.custom_title.take()),
            "ai-title" => self.ai_title = field("aiTitle").or(self.ai_title.take()),
            "relocated" => self.relocated = field("relocatedCwd").or(self.relocated.take()),
            "user" | "assistant" => self.note_message(line),
            _ => {}
        }
    }

    fn note_message(&mut self, line: &Value) {
        let text_of = |key: &str| text(line, key).map(str::to_owned);

        if self.created.is_none() {
            self.sidechain = line.get("isSidechain").and_then(Value::as_bool) == Some(true);
            self.created = text_of("timestamp");
            self.cwd = text_of("cwd");
        }

        match kind(line) {
            Some(Kind::User) => {
                self.has_messages = true;

                if self.first_prompt.is_none() {
                    self.first_prompt = preview_text(line.pointer("/message/content"), MAX_FIRST_PROMPT_CHARS);
                }
            }
            Some(Kind::Assistant) => {
                self.has_messages = true;

                let model = line.pointer("/message/model").and_then(Value::as_str);
                if let Some(model) = model.filter(|model| *model != "<synthetic>") {
                    self.model = Some(model.to_owned());
                }
            }
            _ => {}
        }
    }
}

/// The summary of session `id` in `path` (see the module docs); `None` when it is left out.
fn summarize(id: &str, path: &Path, stamp: &Stamp, sidecar: &Path) -> Option<SessionSummary> {
    let mut facts = Facts::default();

    for line in Lines::open(path, SUMMARY_BYTES).ok()? {
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            facts.note(id, &value);
        }
    }

    if stamp.size > SUMMARY_BYTES {
        for value in tail(path) {
            facts.note(id, &value);
        }
    }

    if !facts.own || facts.sidechain {
        return None;
    }

    let renamed = stamp.sidecar.and_then(|_| sidecar_title(sidecar));
    let (title, title_source) = match (facts.custom_title, renamed, facts.ai_title) {
        (Some(title), _, _) | (None, Some(title), _) => (Some(title), Some(SessionTitleSource::User)),
        (None, None, Some(title)) => (Some(title), Some(SessionTitleSource::Auto)),
        (None, None, None) => (None, None),
    };

    if title.is_none() && !facts.has_messages {
        return None;
    }

    let updated_at = rfc3339(stamp.modified);

    Some(SessionSummary {
        session: SessionRef {
            harness: HarnessId::Claude,
            id: id.to_owned(),
        },
        title,
        title_source,
        cwd: facts.relocated.or(facts.cwd).unwrap_or_default(),
        created_at: facts.created.unwrap_or_else(|| updated_at.clone()),
        updated_at,
        size_bytes: stamp.size,
        model: facts.model,
        first_prompt: facts.first_prompt,
        parent_session: None,
        empty_copy: false,
        routine: None,
    })
}

/// The JSON lines wholly inside the last [`TAIL_BYTES`] of `path`; nothing when unreadable.
fn tail(path: &Path) -> Vec<Value> {
    tail_bytes(path, TAIL_BYTES)
        .unwrap_or_default()
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line.trim_ascii_end()).ok())
        .collect()
}

/// The modification time of the rename sidecar, when it is a regular file in a real (not linked)
/// session folder.
fn sidecar_time(folder: &Path, sidecar: &Path) -> Option<SystemTime> {
    let is_dir = |path: &Path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir());
    let is_file = |path: &Path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file());

    if !is_dir(folder) || !is_file(sidecar) {
        return None;
    }

    fs::symlink_metadata(sidecar)
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// The title in a rename sidecar (`{"customTitle": …}`), trimmed; `None` when empty or unreadable.
fn sidecar_title(path: &Path) -> Option<String> {
    let raw = SafeReader::read_to_string(path, SIDECAR_MAX_BYTES).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let title = text(&value, "customTitle")?.trim();

    (!title.is_empty()).then(|| title.to_owned())
}
