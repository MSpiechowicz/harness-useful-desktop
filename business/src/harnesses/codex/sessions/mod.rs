//! Codex saved sessions (threads), read and deleted through a short-lived `codex app-server`
//! ([`app_server`]), one process per view load, never by parsing the rollout files (their format
//! changes from version to version).
//!
//! - List: `thread/list` of the project folder (`cwd`, an exact match), paged, newest update
//!   first, archived threads left out. Codex lists only interactive sources by default; the
//!   app's own threads (forks too) are `vscode`, so every source but subagents is asked for. A
//!   fork names the thread it came from (`parentSession`). The title is the thread's name, the
//!   fallback its preview (the first prompt).
//! - Transcript and touched files: `thread/read` (the thread must be of the project or a folder
//!   inside it), then every page of `thread/turns/list` and `thread/items/list`, oldest first
//!   (never `includeTurns`, which Codex deprecates), as in [`transcript`]. Later pages and the
//!   touched files reuse the read of the first page.
//! - Delete: `thread/read` (same check), then `thread/delete`. Codex refuses a thread whose
//!   forks still reference it: that is `Conflict`, the forks are never deleted for the user.
//! - The chat engine forks at a turn ([`CodexSessions::fork_point`]: a message's id is its
//!   turn's) and resumes by the thread id; the export renders the saved messages with the shared
//!   HTML exporter ([`CodexSessions::export_html`]).
//!
//! Lists are cached for [`LIST_TTL`] and reads for [`READ_TTL`], per binary and project. One
//! app-server runs at a time; a caller that waited for another finds its result in the cache.
//! Session ids are checked before anything runs.

mod app_server;
mod cache;
mod transcript;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::de::DeserializeOwned;

use self::app_server::{AppServer, CallError, failure};
use self::cache::{CacheKey, Cached, cached, lock, store};
use self::transcript::{Origin, Transcript, transcript};
use crate::agent::rfc3339;
use crate::codex_rpc::wire::items::Turn;
use crate::codex_rpc::wire::outbound::{Paged, Request, ThreadList};
use crate::codex_rpc::wire::results::{ItemEntry, Page, Thread, ThreadRead};
use crate::error::{BusinessError, Result};
use crate::harness::{CliSessionCtx, CliSessions};
use crate::model::{
    ChatMessage, HarnessId, MAX_FIRST_PROMPT_CHARS, SessionMessagesPage, SessionRef, SessionSummary, SessionTitleSource,
};
use crate::opencode_server::export_html::{ExportPage, render};
use crate::opencode_server::mapper::project_dir;
use crate::pi_family::sessions::{is_safe_session_id, page_range};

/// How long one app-server run may take, start to last answer.
const TIMEOUT: Duration = Duration::from_secs(30);

/// How long a session list is reused.
const LIST_TTL: Duration = Duration::from_secs(5);

/// How long a read thread is reused for paging and the touched files.
const READ_TTL: Duration = Duration::from_secs(60);

/// Cached lists and reads kept at most.
const MAX_LISTS: usize = 16;
const MAX_READS: usize = 4;

/// Page sizes asked for, and the most entries read of each kind.
const THREAD_PAGE: u32 = 100;
const TURN_PAGE: u32 = 100;
const ITEM_PAGE: u32 = 200;
const MAX_THREADS: usize = 5_000;
const MAX_TURNS: usize = 20_000;
const MAX_ITEMS: usize = 100_000;

/// Every thread source but subagents: Codex's default is the interactive ones only.
const SOURCE_KINDS: [&str; 5] = ["cli", "vscode", "exec", "appServer", "unknown"];

/// The export title of a thread without a name or a prompt.
const UNTITLED: &str = "Codex session";

/// What a refused delete says: the forks have to go first.
const FORKS_FIRST: &str = "delete the forks of this session first";

/// The Codex sessions with their caches (see the module docs).
#[derive(Debug)]
pub struct CodexSessions {
    timeout: Duration,
    lists: Mutex<Vec<Cached<Vec<SessionSummary>>>>,
    reads: Mutex<Vec<Cached<Read>>>,
    /// Held while an app-server runs.
    runs: Mutex<()>,
}

impl Default for CodexSessions {
    fn default() -> Self {
        Self::new()
    }
}

static SESSIONS: CodexSessions = CodexSessions::new();

/// The sessions [`super::Codex`] returns from `cli_sessions`.
pub fn cli_sessions() -> &'static CodexSessions {
    &SESSIONS
}

impl CodexSessions {
    /// Empty caches, each app-server run limited to 30 seconds.
    pub const fn new() -> Self {
        Self::with_timeout(TIMEOUT)
    }

    /// Empty caches, each app-server run limited to `timeout` (then it is killed).
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            lists: Mutex::new(Vec::new()),
            reads: Mutex::new(Vec::new()),
            runs: Mutex::new(()),
        }
    }

    /// The turn a fork from message `message_id` of `session` keeps (`thread/fork{lastTurnId}`):
    /// a message's id is its turn's. `NotFound` for a message the thread does not hold.
    pub fn fork_point(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef, message_id: &str) -> Result<String> {
        let id = checked_id(session)?;
        let read = self.read(ctx, id, true)?;

        if read.transcript.turns.iter().any(|turn| turn == message_id) {
            return Ok(message_id.to_owned());
        }

        Err(BusinessError::NotFound(format!("message {message_id} in this chat")))
    }

    /// Every saved message of `session`, oldest first (the chat's history and last reply).
    pub fn saved_messages(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<Vec<ChatMessage>> {
        let id = checked_id(session)?;

        Ok(self.read(ctx, id, true)?.transcript.messages.clone())
    }

    /// The HTML page of `session` by the shared exporter, exported at `exported_at` (RFC 3339).
    pub fn export_html(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef, exported_at: &str) -> Result<String> {
        let id = checked_id(session)?;
        let read = self.read(ctx, id, true)?;
        let page = ExportPage {
            title: read.title.as_deref().unwrap_or(UNTITLED),
            harness: "Codex",
            session_id: id,
            exported_at,
        };

        Ok(render(&page, &read.transcript.messages))
    }

    /// The project's sessions, from the cache when fresh.
    fn sessions(&self, ctx: &CliSessionCtx<'_>) -> Result<Arc<Vec<SessionSummary>>> {
        let key = CacheKey::new(ctx, None);
        let since = Instant::now().checked_sub(LIST_TTL);

        if let Some(sessions) = cached(&self.lists, &key, since) {
            return Ok(sessions);
        }

        let _run = lock(&self.runs);

        // Listed meanwhile by the run this call waited for.
        if let Some(sessions) = cached(&self.lists, &key, since) {
            return Ok(sessions);
        }

        let sessions = self.list_now(ctx)?;

        Ok(store(&self.lists, key, sessions, MAX_LISTS))
    }

    /// Lists the project's threads.
    fn list_now(&self, ctx: &CliSessionCtx<'_>) -> Result<Vec<SessionSummary>> {
        let mut server = AppServer::start(ctx, self.timeout)?;
        let cwd = ctx.project.to_string_lossy().into_owned();

        let threads: Vec<Thread> = pages(&mut server, MAX_THREADS, |cursor| {
            Request::ThreadList(ThreadList {
                cwd: Some(cwd.clone()),
                cursor,
                limit: Some(THREAD_PAGE),
                sort_key: Some("updated_at".to_owned()),
                source_kinds: Some(SOURCE_KINDS.iter().map(|kind| (*kind).to_owned()).collect()),
                archived: Some(false),
                ..ThreadList::default()
            })
        })
        .map_err(|error| failure(error, "thread/list"))?;

        server.finish();

        let mut sessions: Vec<(u64, SessionSummary)> = threads
            .into_iter()
            .filter_map(|thread| summary_of(thread, ctx.project))
            .collect();
        sessions.sort_by_key(|(updated, _)| std::cmp::Reverse(*updated));

        Ok(sessions.into_iter().map(|(_, summary)| summary).collect())
    }

    /// The read thread `id`: from the cache unless `fresh`, which takes only a read that started
    /// after this call (one the call waited for).
    fn read(&self, ctx: &CliSessionCtx<'_>, id: &str, fresh: bool) -> Result<Arc<Read>> {
        let key = CacheKey::new(ctx, Some(id));
        let asked = Instant::now();
        let since = if fresh {
            Some(asked)
        } else {
            asked.checked_sub(READ_TTL)
        };

        if let Some(read) = cached(&self.reads, &key, since) {
            return Ok(read);
        }

        let _run = lock(&self.runs);

        if let Some(read) = cached(&self.reads, &key, since) {
            return Ok(read);
        }

        let read = self.read_now(ctx, id)?;

        Ok(store(&self.reads, key, read, MAX_READS))
    }

    /// Reads thread `id` of the project.
    fn read_now(&self, ctx: &CliSessionCtx<'_>, id: &str) -> Result<Read> {
        let mut server = AppServer::start(ctx, self.timeout)?;
        let thread = owned_thread(&mut server, ctx, id)?;

        let paged = |cursor: Option<String>, limit: u32| Paged {
            thread_id: id.to_owned(),
            cursor,
            limit: Some(limit),
            sort_direction: Some("asc".to_owned()),
            ..Paged::default()
        };

        let turns: Vec<Turn> = pages(&mut server, MAX_TURNS, |cursor| {
            Request::ThreadTurnsList(Paged {
                items_view: Some("notLoaded".to_owned()),
                ..paged(cursor, TURN_PAGE)
            })
        })
        .map_err(|error| thread_failure(error, "thread/turns/list", id))?;

        let items: Vec<ItemEntry> = pages(&mut server, MAX_ITEMS, |cursor| {
            Request::ThreadItemsList(paged(cursor, ITEM_PAGE))
        })
        .map_err(|error| thread_failure(error, "thread/items/list", id))?;

        server.finish();

        let origin = Origin {
            model: thread.model.as_deref(),
            provider: Some(thread.model_provider.as_str()).filter(|provider| !provider.is_empty()),
        };

        Ok(Read {
            transcript: transcript(&turns, &items, origin, ctx.project),
            title: title_of(&thread).or_else(|| prompt_of(&thread)),
        })
    }

    /// Drops the project's cached list and every cached read of `id`.
    fn forget(&self, ctx: &CliSessionCtx<'_>, id: &str) {
        let list = CacheKey::new(ctx, None);

        lock(&self.lists).retain(|entry| entry.key != list);
        lock(&self.reads).retain(|entry| entry.key.session.as_deref() != Some(id));
    }
}

impl CliSessions for CodexSessions {
    fn list(&self, ctx: &CliSessionCtx<'_>) -> Result<Vec<SessionSummary>> {
        Ok(self.sessions(ctx)?.as_ref().clone())
    }

    /// The first page (no cursor) reads the thread again; later pages page the same read.
    fn messages(
        &self,
        ctx: &CliSessionCtx<'_>,
        session: &SessionRef,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionMessagesPage> {
        let id = checked_id(session)?;
        let read = self.read(ctx, id, cursor.is_none())?;
        let messages = &read.transcript.messages;
        let range = page_range(cursor, limit, messages.len())?;

        Ok(SessionMessagesPage {
            messages: messages[range.clone()].to_vec(),
            next_cursor: (range.end < messages.len()).then(|| range.end.to_string()),
            total: u32::try_from(messages.len()).unwrap_or(u32::MAX),
            stats: cursor.is_none().then(|| read.transcript.stats.clone()),
        })
    }

    fn files_touched(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<Vec<String>> {
        let id = checked_id(session)?;

        Ok(self.read(ctx, id, false)?.transcript.touched.clone())
    }

    fn delete(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<()> {
        let id = checked_id(session)?;

        let deleted = {
            let _run = lock(&self.runs);
            let mut server = AppServer::start(ctx, self.timeout)?;
            owned_thread(&mut server, ctx, id)?;

            let request = Request::ThreadDelete {
                thread_id: id.to_owned(),
            };
            let deleted = server.call::<serde_json::Value>(&request);
            server.finish();

            deleted
        };

        self.forget(ctx, id);

        match deleted {
            Ok(_) => Ok(()),
            Err(CallError::Rpc(error)) if error.message.to_lowercase().contains("fork") => {
                Err(BusinessError::Conflict(FORKS_FIRST.to_owned()))
            }
            Err(error) => Err(thread_failure(error, "thread/delete", id)),
        }
    }
}

/// A read thread: its transcript and export title.
#[derive(Debug)]
struct Read {
    transcript: Transcript,
    title: Option<String>,
}

/// Every page of a paged request (`request` for a cursor), up to `max` entries.
fn pages<T: DeserializeOwned>(
    server: &mut AppServer,
    max: usize,
    request: impl Fn(Option<String>) -> Request,
) -> std::result::Result<Vec<T>, CallError> {
    let mut entries = Vec::new();
    let mut cursor = None;

    loop {
        let page: Page<T> = server.call(&request(cursor))?;
        let empty = page.data.is_empty();
        entries.extend(page.data);

        cursor = page.next_cursor;

        if cursor.is_none() || empty || entries.len() >= max {
            break;
        }
    }

    entries.truncate(max);

    Ok(entries)
}

/// Thread `id` by `thread/read`, when it belongs to the project (or a folder inside it).
fn owned_thread(server: &mut AppServer, ctx: &CliSessionCtx<'_>, id: &str) -> Result<Thread> {
    let request = Request::ThreadRead {
        thread_id: id.to_owned(),
    };
    let read: ThreadRead = server
        .call(&request)
        .map_err(|error| thread_failure(error, "thread/read", id))?;

    if read.thread.id != id || project_dir(&read.thread.cwd, ctx.project).is_none() {
        return Err(not_found(id));
    }

    Ok(read.thread)
}

/// A failed call about thread `id`: Codex's "not found" is `NotFound`. Codex 0.154 answers an
/// unknown id with "thread not loaded" (a saved thread reads fine without being loaded).
fn thread_failure(error: CallError, method: &str, id: &str) -> BusinessError {
    if let CallError::Rpc(rpc) = &error {
        let message = rpc.message.to_lowercase();

        if message.contains("not found") || message.contains("not loaded") || message.contains("no rollout") {
            return not_found(id);
        }
    }

    failure(error, method)
}

/// The id of `session`, when it is a Codex thread id.
fn checked_id(session: &SessionRef) -> Result<&str> {
    if session.harness != HarnessId::Codex {
        return Err(BusinessError::InvalidInput(
            "the session belongs to another harness".to_owned(),
        ));
    }

    if !is_safe_session_id(&session.id) {
        return Err(BusinessError::InvalidInput("invalid session id".to_owned()));
    }

    Ok(&session.id)
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}

/// A listed thread as a session with its update time (seconds), when it is a saved thread of
/// `project` (not ephemeral, not a subagent's).
fn summary_of(thread: Thread, project: &Path) -> Option<(u64, SessionSummary)> {
    if !is_safe_session_id(&thread.id) || thread.ephemeral || thread.parent_thread_id.is_some() {
        return None;
    }

    project_dir(&thread.cwd, project)?;

    let updated = thread.updated_at.or(thread.created_at).unwrap_or(0);
    let created = thread.created_at.unwrap_or(updated);
    let title = title_of(&thread);

    let summary = SessionSummary {
        title_source: title.as_ref().map(|_| SessionTitleSource::User),
        title,
        first_prompt: prompt_of(&thread),
        created_at: time_text(created),
        updated_at: time_text(updated),
        size_bytes: 0,
        model: thread.model,
        parent_session: thread.forked_from_id.filter(|parent| is_safe_session_id(parent)),
        empty_copy: false,
        routine: None,
        cwd: thread.cwd,
        session: SessionRef {
            harness: HarnessId::Codex,
            id: thread.id,
        },
    };

    Some((updated, summary))
}

/// The name the user gave the thread.
fn title_of(thread: &Thread) -> Option<String> {
    let name = thread.name.as_deref()?.trim();

    (!name.is_empty()).then(|| name.to_owned())
}

/// The thread's preview (its first prompt), cut to [`MAX_FIRST_PROMPT_CHARS`].
fn prompt_of(thread: &Thread) -> Option<String> {
    let preview = thread.preview.trim();

    (!preview.is_empty()).then(|| preview.chars().take(MAX_FIRST_PROMPT_CHARS).collect())
}

/// Seconds since the epoch as RFC 3339.
fn time_text(seconds: u64) -> String {
    rfc3339(UNIX_EPOCH + Duration::from_secs(seconds))
}
