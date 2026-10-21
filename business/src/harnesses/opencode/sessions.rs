//! OpenCode saved sessions, reachable only through the `opencode` CLI (the sessions database also
//! holds credentials and is never opened). OpenCode 1 is described here; OpenCode 2 runs other
//! commands with the same rules (see [`v2`]). [`cli_sessions`] picks the generation.
//!
//! - List: `opencode session list --pure --format json`, run in the project. OpenCode lists the
//!   root sessions of the whole project (its git worktree), so only sessions whose directory is the
//!   project or lies inside it are kept. Empty output means no sessions.
//! - Transcript and edited files: `opencode export <id> --pure` (see [`transcript`]). The export
//!   finds a session of any project, so its directory is checked the same way.
//! - Delete: `opencode session delete --pure <id>`, only for a session the project's list shows
//!   (listed again, bypassing the cache, when the cached list lacks it).
//!
//! Every command runs without external plugins (`--pure`), without the project's OpenCode config
//! (`OPENCODE_DISABLE_PROJECT_CONFIG`: otherwise starting up in the project writes
//! `.opencode/.gitignore` and installs packages there, and the sessions are the same either way),
//! with the harness's spawn environment (no self-update) and the launcher search directories;
//! session ids are checked before anything runs. At most [`MAX_RUNS`] commands run at once.
//!
//! Lists are cached for [`LIST_TTL`] and parsed exports for [`EXPORT_TTL`] (later transcript pages
//! and the edited files reuse the export of the first page), per binary and project; a delete drops
//! both. Concurrent list calls for the same binary and project share one run; the re-list of a
//! delete only shares a run that started after it was asked for.

mod limits;
mod transcript;
mod transcript_v2;
mod v2;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::Value;

use self::limits::{Flights, Runs};
use self::transcript::Transcript;
use super::SPAWN_ENV;
use crate::agent::rfc3339;
use crate::error::{BusinessError, Result};
use crate::harness::{CliSessionCtx, CliSessions, OpenCodeGeneration};
use crate::model::{HarnessId, SessionMessagesPage, SessionRef, SessionSummary};
use crate::opencode_server::mapper::project_dir;
use crate::pi_family::normalize::plain_text;
use crate::pi_family::sessions::{is_safe_session_id, page_range};
use crate::process::{ExecOutput, ExecSpec, launcher_search_dirs};
use crate::security::redact_text;

/// Time limit and output cap of `session list`.
const LIST_TIMEOUT: Duration = Duration::from_secs(15);
const LIST_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Time limit and output cap of `export`.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(30);
const EXPORT_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Time limit and output cap of `session delete`.
const DELETE_TIMEOUT: Duration = Duration::from_secs(15);
const DELETE_MAX_BYTES: usize = 64 * 1024;

/// How long a session list is reused.
const LIST_TTL: Duration = Duration::from_secs(5);

/// How long a parsed export is reused for paging.
const EXPORT_TTL: Duration = Duration::from_secs(60);

/// Cached lists and exports kept at most.
const MAX_LISTS: usize = 16;
const MAX_EXPORTS: usize = 4;

/// Longest CLI error line passed on, in characters.
const MAX_ERROR_CHARS: usize = 200;

/// OpenCode commands running at once at most; more wait for a free slot.
const MAX_RUNS: usize = 2;

/// The OpenCode CLI sessions of one generation, with their caches.
#[derive(Debug)]
pub struct OpenCodeSessions {
    generation: OpenCodeGeneration,
    lists: Mutex<Vec<Cached<Vec<SessionSummary>>>>,
    exports: Mutex<Vec<Cached<Transcript>>>,
    listing: Flights<CacheKey, Vec<SessionSummary>>,
    runs: Runs,
}

impl Default for OpenCodeSessions {
    fn default() -> Self {
        Self::new()
    }
}

static SESSIONS: OpenCodeSessions = OpenCodeSessions::new();
static SESSIONS_V2: OpenCodeSessions = OpenCodeSessions::with_generation(OpenCodeGeneration::V2);

/// The sessions [`super::OpenCode`] of `generation` returns from `cli_sessions`.
pub fn cli_sessions(generation: OpenCodeGeneration) -> &'static dyn CliSessions {
    match generation {
        OpenCodeGeneration::V1 => &SESSIONS,
        OpenCodeGeneration::V2 => &SESSIONS_V2,
    }
}

/// An `opencode` command this module runs.
#[derive(Debug, Clone, Copy)]
enum Command<'a> {
    List,
    Export(&'a str),
    Delete(&'a str),
}

impl OpenCodeSessions {
    /// OpenCode 1 sessions with empty caches.
    pub const fn new() -> Self {
        Self::with_generation(OpenCodeGeneration::V1)
    }

    /// Sessions of `generation` with empty caches.
    pub const fn with_generation(generation: OpenCodeGeneration) -> Self {
        Self {
            generation,
            lists: Mutex::new(Vec::new()),
            exports: Mutex::new(Vec::new()),
            listing: Flights::new(),
            runs: Runs::new(MAX_RUNS),
        }
    }

    /// The project's sessions, from the cache when fresh unless `bypass_cache`. A caller that
    /// finds a list run for the same binary and project in flight waits for its result; with
    /// `bypass_cache`, that run may predate the call, so the caller waits it out and takes the
    /// result of a run started after the call.
    fn sessions(&self, ctx: &CliSessionCtx<'_>, bypass_cache: bool) -> Result<Arc<Vec<SessionSummary>>> {
        let key = CacheKey::new(ctx, None);
        let list = || {
            let sessions = self.list_now(ctx)?;

            Ok(store(&self.lists, key.clone(), sessions, MAX_LISTS))
        };

        if bypass_cache {
            return self.listing.share_fresh(&key, list);
        }

        if let Some(sessions) = cached(&self.lists, &key, LIST_TTL) {
            return Ok(sessions);
        }

        self.listing.share(&key, list)
    }

    /// Runs `session list` for the project.
    fn list_now(&self, ctx: &CliSessionCtx<'_>) -> Result<Vec<SessionSummary>> {
        let spec = self.spec(ctx, Command::List, LIST_TIMEOUT, LIST_MAX_BYTES);
        let stdout = self.run(ctx, &spec, "session list", None)?;

        parse_list(&stdout, ctx.project)
    }

    /// The parsed export of session `id`; `fresh` skips the cache.
    fn transcript(&self, ctx: &CliSessionCtx<'_>, id: &str, fresh: bool) -> Result<Arc<Transcript>> {
        let key = CacheKey::new(ctx, Some(id));

        if !fresh && let Some(transcript) = cached(&self.exports, &key, EXPORT_TTL) {
            return Ok(transcript);
        }

        let spec = self.spec(ctx, Command::Export(id), EXPORT_TIMEOUT, EXPORT_MAX_BYTES);
        let stdout = self.run(ctx, &spec, "export", Some(id))?;
        let transcript = match self.generation {
            OpenCodeGeneration::V1 => transcript::parse(&stdout, id, ctx.project)?,
            OpenCodeGeneration::V2 => transcript_v2::parse(&stdout, id, ctx.project)?,
        };

        Ok(store(&self.exports, key, transcript, MAX_EXPORTS))
    }

    /// `command` of this generation, run in the project (see the module docs).
    fn spec(
        &self,
        ctx: &CliSessionCtx<'_>,
        command: Command<'_>,
        timeout: Duration,
        max_output_bytes: usize,
    ) -> ExecSpec {
        let (args, extra_env) = match self.generation {
            OpenCodeGeneration::V1 => (v1_args(command), &[][..]),
            OpenCodeGeneration::V2 => (v2::args(command), v2::SPAWN_ENV),
        };

        spec(ctx, &args, timeout, max_output_bytes).envs(extra_env.iter().copied())
    }

    /// Drops the project's cached list and the cached export of `id`.
    fn forget(&self, ctx: &CliSessionCtx<'_>, id: &str) {
        let list = CacheKey::new(ctx, None);
        let export = CacheKey::new(ctx, Some(id));

        lock(&self.lists).retain(|entry| entry.key != list);
        lock(&self.exports).retain(|entry| entry.key != export);
    }

    /// Runs `spec` (waiting for a free slot) and returns its stdout. A timeout, failed exit or
    /// oversized output is an error naming `command`; "session not found" is `NotFound` for
    /// session `id`.
    fn run(&self, ctx: &CliSessionCtx<'_>, spec: &ExecSpec, command: &str, id: Option<&str>) -> Result<String> {
        let output = {
            let _slot = self.runs.acquire();
            ctx.runner.run(spec)?
        };

        if output.timed_out {
            return Err(BusinessError::Timeout(format!("opencode {command}")));
        }

        // A failure says why even when its output overflowed.
        if !output.success() {
            return Err(failure(&output, command, id));
        }

        if output.truncated {
            return Err(BusinessError::InvalidInput(format!(
                "the output of opencode {command} is too large"
            )));
        }

        Ok(output.stdout)
    }
}

impl CliSessions for OpenCodeSessions {
    fn list(&self, ctx: &CliSessionCtx<'_>) -> Result<Vec<SessionSummary>> {
        Ok(self.sessions(ctx, false)?.as_ref().clone())
    }

    /// The first page (no cursor) exports the session again; later pages page the same export.
    fn messages(
        &self,
        ctx: &CliSessionCtx<'_>,
        session: &SessionRef,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionMessagesPage> {
        let id = checked_id(session)?;
        let transcript = self.transcript(ctx, id, cursor.is_none())?;

        page(&transcript, cursor, limit)
    }

    fn files_touched(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<Vec<String>> {
        let id = checked_id(session)?;

        Ok(self.transcript(ctx, id, false)?.touched.clone())
    }

    fn delete(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<()> {
        let id = checked_id(session)?;

        // `session delete` finds a session of any project: only one the project lists is deleted.
        // A session newer than the cached list is looked up in a new one.
        let listed_in = |sessions: &[SessionSummary]| sessions.iter().any(|summary| summary.session.id == id);

        let listed = listed_in(&self.sessions(ctx, false)?) || listed_in(&self.sessions(ctx, true)?);
        if !listed {
            return Err(not_found(id));
        }

        let spec = self.spec(ctx, Command::Delete(id), DELETE_TIMEOUT, DELETE_MAX_BYTES);
        let deleted = self.run(ctx, &spec, "session delete", Some(id));
        self.forget(ctx, id);

        deleted.map(drop)
    }
}

/// The id of `session`, when it is an OpenCode session id safe to pass to the CLI.
fn checked_id(session: &SessionRef) -> Result<&str> {
    if session.harness != HarnessId::OpenCode {
        return Err(BusinessError::InvalidInput(
            "the session belongs to another harness".to_owned(),
        ));
    }

    if !is_opencode_id(&session.id) {
        return Err(BusinessError::InvalidInput("invalid session id".to_owned()));
    }

    Ok(&session.id)
}

/// True for `ses_…` ids made only of characters safe as a CLI argument.
fn is_opencode_id(id: &str) -> bool {
    id.starts_with("ses_") && is_safe_session_id(id)
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}

/// The OpenCode 1 argv of `command`.
fn v1_args(command: Command<'_>) -> Vec<&str> {
    match command {
        Command::List => vec!["session", "list", "--pure", "--format", "json"],
        Command::Export(id) => vec!["export", id, "--pure"],
        Command::Delete(id) => vec!["session", "delete", "--pure", id],
    }
}

/// An `opencode` command run in the project.
fn spec(ctx: &CliSessionCtx<'_>, args: &[&str], timeout: Duration, max_output_bytes: usize) -> ExecSpec {
    ExecSpec::new(&ctx.binary.path)
        .args(args.iter().copied())
        .cwd(ctx.project)
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .env("OPENCODE_DISABLE_PROJECT_CONFIG", "1")
        .envs(SPAWN_ENV.iter().copied())
        .search_dirs(launcher_search_dirs(ctx.env, &ctx.binary.path))
        .timeout(timeout)
        .max_output_bytes(max_output_bytes)
}

/// A failed command as a short error: the last stderr line, without colors, redacted and cut.
fn failure(output: &ExecOutput, command: &str, id: Option<&str>) -> BusinessError {
    let stderr = plain_text(&output.stderr);

    if let Some(id) = id
        && stderr.contains("Session not found")
    {
        return not_found(id);
    }

    let last_line = stderr.lines().map(str::trim).rfind(|line| !line.is_empty());

    match last_line {
        Some(line) => {
            let detail: String = redact_text(line).chars().take(MAX_ERROR_CHARS).collect();
            BusinessError::Process(format!("opencode {command} failed: {detail}"))
        }
        None => BusinessError::Process(format!("opencode {command} failed")),
    }
}

/// The sessions of `session list` JSON that belong to `project`, newest first. Entries without a
/// usable id or directory are skipped.
fn parse_list(stdout: &str, project: &Path) -> Result<Vec<SessionSummary>> {
    let stdout = stdout.trim();

    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    let Ok(Value::Array(entries)) = serde_json::from_str::<Value>(stdout) else {
        return Err(BusinessError::Protocol(
            "the OpenCode session list is not a JSON array".to_owned(),
        ));
    };

    let mut sessions: Vec<(i64, SessionSummary)> =
        entries.iter().filter_map(|entry| summary_of(entry, project)).collect();
    sessions.sort_by_key(|(updated, _)| std::cmp::Reverse(*updated));

    Ok(sessions.into_iter().map(|(_, summary)| summary).collect())
}

/// One `session list` entry with its update time (Unix milliseconds), when it is a session of
/// `project`.
fn summary_of(entry: &Value, project: &Path) -> Option<(i64, SessionSummary)> {
    let id = entry
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| is_opencode_id(id))?;
    let directory = entry.get("directory").and_then(Value::as_str)?;
    project_dir(directory, project)?;

    let millis = |key: &str| entry.get(key).and_then(Value::as_i64).filter(|time| *time >= 0);
    let updated = millis("updated").or_else(|| millis("created")).unwrap_or(0);
    let created = millis("created").unwrap_or(updated);

    let title = entry
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .map(str::to_owned);

    let summary = SessionSummary {
        session: SessionRef {
            harness: HarnessId::OpenCode,
            id: id.to_owned(),
        },
        title,
        title_source: None,
        cwd: directory.to_owned(),
        created_at: time_text(created),
        updated_at: time_text(updated),
        size_bytes: 0,
        model: None,
        first_prompt: None,
        parent_session: None,
        empty_copy: false,
        routine: None,
    };

    Some((updated, summary))
}

/// Unix milliseconds (non-negative) as RFC 3339.
fn time_text(millis: i64) -> String {
    let millis = u64::try_from(millis).unwrap_or(0);

    rfc3339(UNIX_EPOCH + Duration::from_millis(millis))
}

/// One page of `transcript`: `cursor` is the index of the first message (a previous page's
/// `nextCursor`), `limit` is clamped to `1..=`[`MAX_SESSION_PAGE`](crate::model::MAX_SESSION_PAGE);
/// only the first page carries the totals.
fn page(transcript: &Transcript, cursor: Option<&str>, limit: u32) -> Result<SessionMessagesPage> {
    let total = transcript.messages.len();
    let Range { start, end } = page_range(cursor, limit, total)?;

    Ok(SessionMessagesPage {
        messages: transcript.messages[start..end].to_vec(),
        next_cursor: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).unwrap_or(u32::MAX),
        stats: cursor.is_none().then(|| transcript.stats.clone()),
    })
}

/// What a cache entry belongs to: the binary, the project and, for exports, the session.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheKey {
    binary: PathBuf,
    project: PathBuf,
    session: Option<String>,
}

impl CacheKey {
    fn new(ctx: &CliSessionCtx<'_>, session: Option<&str>) -> Self {
        Self {
            binary: ctx.binary.path.clone(),
            project: ctx.project.to_path_buf(),
            session: session.map(str::to_owned),
        }
    }
}

#[derive(Debug)]
struct Cached<T> {
    key: CacheKey,
    at: Instant,
    value: Arc<T>,
}

fn lock<T>(cache: &Mutex<T>) -> MutexGuard<'_, T> {
    cache.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The value cached for `key`, when younger than `ttl`. Expired entries are dropped.
fn cached<T>(cache: &Mutex<Vec<Cached<T>>>, key: &CacheKey, ttl: Duration) -> Option<Arc<T>> {
    let mut entries = lock(cache);
    entries.retain(|entry| entry.at.elapsed() < ttl);

    entries
        .iter()
        .find(|entry| &entry.key == key)
        .map(|entry| Arc::clone(&entry.value))
}

/// Caches `value` for `key`, keeping at most `max` entries (the oldest go first).
fn store<T>(cache: &Mutex<Vec<Cached<T>>>, key: CacheKey, value: T, max: usize) -> Arc<T> {
    let value = Arc::new(value);
    let mut entries = lock(cache);

    entries.retain(|entry| entry.key != key);
    entries.push(Cached {
        key,
        at: Instant::now(),
        value: Arc::clone(&value),
    });

    let excess = entries.len().saturating_sub(max);
    entries.drain(..excess);

    value
}
