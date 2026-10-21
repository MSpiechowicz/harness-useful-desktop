//! The JSON of OpenCode 2's `opencode session export --standalone <id>` (`{info, messages}`, the
//! `SessionTransfer.Data` of `packages/schema/src/session-transfer.ts`) as a transcript: chat
//! messages, their totals and the files the edit tools touched. Each message is converted by the
//! shared [`SessionMessage`] rules, the touched files by [`ToolCall::touched`](crate::opencode_server_v2::messages::ToolCall::touched).

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::transcript::Transcript;
use crate::error::{BusinessError, Result};
use crate::model::{ChatBlock, ChatMessage, MessageRole};
use crate::opencode_server::mapper::project_dir;
use crate::opencode_server_v2::messages::SessionMessage;
use crate::pi_family::sessions::StatsTally;

/// Parses the export of session `id`. `NotFound` when the export is of another session or the
/// session's directory (`info.location.directory`) is not `project` (canonical) or inside it:
/// the export finds a session of any project.
pub(super) fn parse(stdout: &str, id: &str, project: &Path) -> Result<Transcript> {
    let export: Value = serde_json::from_str(stdout)
        .map_err(|_| BusinessError::Protocol("the OpenCode session export is not JSON".to_owned()))?;

    let info = export.get("info").unwrap_or(&Value::Null);
    let exported_id = info.get("id").and_then(Value::as_str);
    let directory = info
        .pointer("/location/directory")
        .and_then(Value::as_str)
        .and_then(|directory| project_dir(directory, project));

    let (Some(directory), Some(exported_id)) = (directory, exported_id) else {
        return Err(not_found(id));
    };

    if exported_id != id {
        return Err(not_found(id));
    }

    let mut messages = Vec::new();
    let mut tally = StatsTally::default();
    let mut touched = BTreeSet::new();

    let records = export.get("messages").and_then(Value::as_array);

    for record in records.into_iter().flatten() {
        let message = SessionMessage::parse(record);

        for call in message.tool_calls() {
            touched.extend(call.touched(project, &directory));
        }

        for chat in message.chat_messages() {
            note(&mut tally, &chat);
            messages.push(chat);
        }
    }

    Ok(Transcript {
        messages,
        stats: tally.finish(),
        touched: touched.into_iter().collect(),
    })
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}

/// Adds one chat message to `tally`: tool results are not messages of their own.
fn note(tally: &mut StatsTally, message: &ChatMessage) {
    if message.role == MessageRole::ToolResult {
        return;
    }

    tally.note_message();
    tally.note_time(message.timestamp);

    if message.role != MessageRole::Assistant {
        return;
    }

    let tool_calls = message
        .blocks
        .iter()
        .filter(|block| matches!(block, ChatBlock::ToolCall { .. }))
        .count();
    let context_tokens = message.usage.map(|usage| usage.total);

    tally.note_reply(tool_calls, message.model.as_deref(), message.cost_usd, context_tokens);
}
