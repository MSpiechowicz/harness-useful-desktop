//! Memory recall: before a chat message is sent, Harness Desktop asks the installed `harness-memory`
//! server for notes about it and puts the hits in front of the message as a
//! `<harness-memory-recall>` block (format and text helpers in `recall_text`, re-exported here).
//!
//! - Scripts: the same `memory_search` reply also lists this repository's saved scripts that match
//!   (a `<saved-scripts>` block, and `scripts` in `structuredContent`), and they go into the block
//!   after the notes (`recall_scripts` reads them). One call serves both, so scripts add no round
//!   trip and no time beyond [`MemoryRecall::timeout`]. A separate `script_list` call was not used:
//!   it only filters on one substring of the name or description, which a keyword query rarely is,
//!   and it would cost a second request. A reply without scripts, or one the parser cannot read,
//!   recalls the notes alone, and scripts without a note are recalled alone.
//!
//! - Speed: [`MemoryRecall::warm`] starts the server and completes the MCP handshake ahead of time
//!   (for example while the user types), so [`MemoryRecall::recall`] only pays for one
//!   `memory_search` call. Without a warm handle it connects first.
//! - Bounded: the whole recall (connect if needed, then the call) is limited to
//!   [`MemoryRecall::timeout`]; any failure leaves the message unchanged.
//! - Only notes this checkout wrote itself are recalled: the server runs in the project directory
//!   and is asked for repository notes last written from that same checkout without
//!   `project_path` (`scope: "repo"`, `same_checkout: true`). Global notes, notes written from
//!   another checkout claiming the same `origin`, and notes written through `project_path` are
//!   never put in front of a message (agents can still search them).
//! - Dropping a [`WarmRecall`] (or a recall that timed out) kills the server's process group; after
//!   a recall the server is shut down in a background task, so the message never waits for it.
//! - Neither the query nor any note text is logged.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value, json};

use super::mcp_client::{McpClient, McpOptions};
use crate::error::BusinessError;
use crate::model::MAX_ACTION_TEXT_BYTES;
use crate::process::{ExecSpec, ProcessSpawner};

pub use super::recall_scripts::{RECALL_SCRIPTS_MAX, RecallScript, recall_scripts, scripts_of};
pub use super::recall_text::{
    MARKER_REMOVED, NOTES_BUDGET, RECALL_BUDGET, RECALL_CLOSE, RECALL_OPEN, SCRIPTS_BUDGET, STOPWORDS, recall_block,
    recall_block_with, recall_eligible, recall_note_count, recall_query, recall_script_count, strip_recall,
    with_recall,
};

/// Default limit of a whole recall.
pub const RECALL_TIMEOUT: Duration = Duration::from_millis(1500);

/// Default number of notes asked for.
pub const RECALL_LIMIT: u32 = 5;

/// Largest `memory_search` answer accepted, in bytes (the server caps its replies at 4 KiB).
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Why a message was sent without a recall block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallReason {
    /// A command, a shell line, or a message that already has a block ([`recall_eligible`]).
    Skipped,
    /// No search keyword in the message.
    NoKeywords,
    /// No matching note or script.
    NoHits,
    /// The server did not answer within the limit.
    Timeout,
    /// The server could not be started, or stopped running.
    Spawn,
    /// The server's answer did not follow the protocol.
    Protocol,
    /// `memory_search` reported an error.
    ToolError,
    /// The block would make the message longer than [`MAX_ACTION_TEXT_BYTES`].
    TooLong,
}

impl RecallReason {
    /// A short stable name, for logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skipped => "skipped",
            Self::NoKeywords => "no_keywords",
            Self::NoHits => "no_hits",
            Self::Timeout => "timeout",
            Self::Spawn => "spawn",
            Self::Protocol => "protocol",
            Self::ToolError => "tool_error",
            Self::TooLong => "too_long",
        }
    }
}

/// What [`MemoryRecall::recall`] did with a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecallOutcome {
    /// The message with a recall block in front of it, and the number of notes and scripts in the
    /// block.
    Recalled {
        message: String,
        notes: usize,
        scripts: usize,
    },
    /// The message stays as the user wrote it.
    Unchanged(RecallReason),
}

/// A server started and past its handshake, ready for one recall in `project`. Dropping it kills
/// the server.
#[derive(Debug)]
pub struct WarmRecall {
    client: McpClient,
    server: PathBuf,
    project: PathBuf,
}

/// Runs recalls against the `harness-memory` executable. See the module docs.
#[derive(Clone)]
pub struct MemoryRecall {
    /// Starts the server.
    pub spawner: Arc<dyn ProcessSpawner>,
    /// Limit of a whole recall, and of each MCP request.
    pub timeout: Duration,
    /// Notes asked for.
    pub limit: u32,
}

impl std::fmt::Debug for MemoryRecall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryRecall")
            .field("timeout", &self.timeout)
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}

impl MemoryRecall {
    /// A recall with the default limit ([`RECALL_TIMEOUT`]) and note count ([`RECALL_LIMIT`]).
    pub fn new(spawner: Arc<dyn ProcessSpawner>) -> Self {
        Self {
            spawner,
            timeout: RECALL_TIMEOUT,
            limit: RECALL_LIMIT,
        }
    }

    /// Starts `server mcp` in `project` and completes the handshake; `None` when that fails.
    pub async fn warm(&self, server: &Path, project: &Path) -> Option<WarmRecall> {
        match self.connect(server, project).await {
            Ok(client) => Some(WarmRecall {
                client,
                server: server.to_path_buf(),
                project: project.to_path_buf(),
            }),
            Err(error) => {
                tracing::debug!(reason = reason_of(&error).as_str(), "memory recall: warm-up failed");
                None
            }
        }
    }

    /// `message` with the notes `harness-memory` finds for it in front, or the reason it stays
    /// unchanged. Uses `warm` when it was started for the same `server` and `project` and is still
    /// usable; otherwise starts the server first.
    pub async fn recall(
        &self,
        warm: Option<WarmRecall>,
        server: &Path,
        project: &Path,
        message: &str,
    ) -> RecallOutcome {
        let outcome = self.run(warm, server, project, message).await;

        if let RecallOutcome::Unchanged(reason) = &outcome {
            tracing::debug!(reason = reason.as_str(), "memory recall: message unchanged");
        }

        outcome
    }

    async fn run(&self, warm: Option<WarmRecall>, server: &Path, project: &Path, message: &str) -> RecallOutcome {
        if !recall_eligible(message) {
            return RecallOutcome::Unchanged(RecallReason::Skipped);
        }

        let Some(query) = recall_query(message) else {
            return RecallOutcome::Unchanged(RecallReason::NoKeywords);
        };

        let warm = warm.filter(|warm| warm.client.is_usable() && warm.server == server && warm.project == project);
        let arguments = search_arguments(&query, self.limit);

        let searched = tokio::time::timeout(self.timeout, async {
            let mut client = match warm {
                Some(warm) => warm.client,
                None => self.connect(server, project).await?,
            };

            let output = client.call_tool("memory_search", arguments).await;
            Ok::<_, BusinessError>((client, output))
        })
        .await;

        let output = match searched {
            Err(_elapsed) => return RecallOutcome::Unchanged(RecallReason::Timeout),
            Ok(Err(error)) => return RecallOutcome::Unchanged(reason_of(&error)),
            Ok(Ok((client, output))) => {
                tokio::spawn(client.shutdown());
                output
            }
        };

        let output = match output {
            Ok(output) if output.is_error => return RecallOutcome::Unchanged(RecallReason::ToolError),
            Ok(output) => output,
            Err(error) => return RecallOutcome::Unchanged(reason_of(&error)),
        };

        let text = output.text.as_deref().unwrap_or_default();
        let scripts = scripts_of(output.structured.as_ref(), text);

        let Some(block) = recall_block_with(text, &scripts) else {
            return RecallOutcome::Unchanged(RecallReason::NoHits);
        };

        let recalled = with_recall(message, &block);

        if recalled.len() > MAX_ACTION_TEXT_BYTES {
            return RecallOutcome::Unchanged(RecallReason::TooLong);
        }

        RecallOutcome::Recalled {
            message: recalled,
            notes: recall_note_count(&block),
            scripts: recall_script_count(&block),
        }
    }

    async fn connect(&self, server: &Path, project: &Path) -> crate::error::Result<McpClient> {
        let spec = ExecSpec::new(server).args(["mcp"]).cwd(project);
        let options = McpOptions {
            request_timeout: self.timeout,
            max_response_bytes: MAX_RESPONSE_BYTES,
            ..McpOptions::default()
        };

        McpClient::connect(self.spawner.as_ref(), &spec, options).await
    }
}

/// `{query, limit, scope: "repo", same_checkout: true}`: this checkout's own repository notes.
fn search_arguments(query: &str, limit: u32) -> Map<String, Value> {
    let mut arguments = Map::new();
    arguments.insert("query".to_owned(), json!(query));
    arguments.insert("limit".to_owned(), json!(limit));
    arguments.insert("scope".to_owned(), json!("repo"));
    arguments.insert("same_checkout".to_owned(), json!(true));
    arguments
}

fn reason_of(error: &BusinessError) -> RecallReason {
    match error {
        BusinessError::Timeout(_) => RecallReason::Timeout,
        BusinessError::Protocol(_) => RecallReason::Protocol,
        _ => RecallReason::Spawn,
    }
}
