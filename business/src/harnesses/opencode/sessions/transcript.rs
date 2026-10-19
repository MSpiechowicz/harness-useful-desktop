//! The JSON of `opencode export <id>` (`{info, messages: [{info, parts}]}`) as a transcript: chat
//! messages, their totals and the files the edit tools touched. Each message is converted by the
//! shared [`mapper::message`](crate::opencode_server::mapper) rules (messages left without content
//! are skipped), the touched files by [`part_touched`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{BusinessError, Result};
use crate::model::{ChatBlock, ChatMessage, MessageRole, TranscriptStats};
use crate::opencode_server::mapper::{part_touched, part_type, project_dir, record_messages};
use crate::pi_family::sessions::StatsTally;

/// A parsed export.
#[derive(Debug)]
pub(super) struct Transcript {
    /// Chat messages in transcript order.
    pub messages: Vec<ChatMessage>,
    /// Totals of the whole transcript.
    pub stats: TranscriptStats,
    /// Project-relative files the edit tools may have changed, sorted.
    pub touched: Vec<String>,
}

/// Parses the export of session `id`. `NotFound` when the export is of another session or the
/// session's directory is not `project` (canonical) or inside it: `opencode export` finds a session
/// of any project.
pub(super) fn parse(stdout: &str, id: &str, project: &Path) -> Result<Transcript> {
    let export: Value = serde_json::from_str(stdout)
        .map_err(|_| BusinessError::Protocol("the OpenCode session export is not JSON".to_owned()))?;

    let info = export.get("info").unwrap_or(&Value::Null);
    let exported_id = info.get("id").and_then(Value::as_str);
    let directory = info
        .get("directory")
        .and_then(Value::as_str)
        .and_then(|directory| project_dir(directory, project));

    let (Some(directory), Some(exported_id)) = (directory, exported_id) else {
        return Err(not_found(id));
    };

    if exported_id != id {
        return Err(not_found(id));
    }

    let mut builder = Builder {
        project,
        directory,
        messages: Vec::new(),
        tally: StatsTally::default(),
        touched: BTreeSet::new(),
    };

    let records = export.get("messages").and_then(Value::as_array);

    for record in records.into_iter().flatten() {
        builder.add(record);
    }

    Ok(Transcript {
        messages: builder.messages,
        stats: builder.tally.finish(),
        touched: builder.touched.into_iter().collect(),
    })
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}

/// Collects the transcript of one export.
struct Builder<'a> {
    project: &'a Path,
    /// The session's working directory, which relative tool paths resolve against.
    directory: PathBuf,
    messages: Vec<ChatMessage>,
    tally: StatsTally,
    touched: BTreeSet<String>,
}

impl Builder<'_> {
    fn add(&mut self, record: &Value) {
        let parts = record.get("parts").and_then(Value::as_array).into_iter().flatten();

        for part in parts.filter(|part| part_type(part) == "tool") {
            self.touched.extend(part_touched(self.project, &self.directory, part));
        }

        for message in record_messages(record) {
            if message.role != MessageRole::ToolResult {
                self.tally.note_message();
                self.tally.note_time(message.timestamp);
            }

            if message.role == MessageRole::Assistant {
                note_reply(&mut self.tally, &message);
            }

            self.messages.push(message);
        }
    }
}

/// Adds one normalized assistant reply to `tally`.
fn note_reply(tally: &mut StatsTally, reply: &ChatMessage) {
    let tool_calls = reply
        .blocks
        .iter()
        .filter(|block| matches!(block, ChatBlock::ToolCall { .. }))
        .count();
    let context_tokens = reply.usage.map(|usage| usage.total);

    tally.note_reply(tool_calls, reply.model.as_deref(), reply.cost_usd, context_tokens);
}
