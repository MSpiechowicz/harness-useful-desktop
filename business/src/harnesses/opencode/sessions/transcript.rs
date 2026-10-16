//! The JSON of `opencode export <id>` (`{info, messages: [{info, parts}]}`, the shapes of
//! `packages/schema/src/v1/session.ts`) as a transcript: chat messages, their totals and the files
//! the edit tools touched.
//!
//! - A user message keeps its typed text (synthetic and ignored text is OpenCode's own context),
//!   image attachments (MIME type only), attached files as `@name` (a file mentioned in the text is
//!   already there) and subtasks as `@agent prompt`.
//! - An assistant message keeps text, reasoning and tool calls in order; each finished tool call is
//!   followed by a tool-result message with its redacted output or error.
//! - Every other part (steps, snapshots, patches, retries, compaction markers) is skipped, as are
//!   messages left without content.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::error::{BusinessError, Result};
use crate::model::{ChatBlock, ChatMessage, MessageRole, TokenUsage, TranscriptStats};
use crate::pi_family::sessions::StatsTally;
use crate::pi_family::touched::touched_paths;
use crate::security::redact_text;

/// Tools whose `filePath` argument names a file they change.
const FILE_PATH_TOOLS: [&str; 4] = ["edit", "write", "multiedit", "patch"];

/// Tools whose `patchText` argument names the files they change.
const PATCH_TOOLS: [&str; 2] = ["apply_patch", "patch"];

/// Patch headers followed by a file path (`packages/opencode/src/patch`).
const PATCH_HEADERS: [&str; 4] = ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"];

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

/// `directory` (a session's working directory) canonicalized, when it is `project` or lies inside
/// it. A directory that no longer exists is compared as written, without `..`.
pub(super) fn project_dir(directory: &str, project: &Path) -> Option<PathBuf> {
    let path = Path::new(directory);

    if directory.is_empty() || !path.is_absolute() {
        return None;
    }

    let resolved = match dunce::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) if has_parent_component(path) => return None,
        Err(_) => path.to_path_buf(),
    };

    resolved.starts_with(project).then_some(resolved)
}

fn has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
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
        let info = record.get("info").unwrap_or(&Value::Null);
        let parts = record
            .get("parts")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);

        match info.get("role").and_then(Value::as_str) {
            Some("user") => self.add_user(info, parts),
            Some("assistant") => self.add_assistant(info, parts),
            _ => {}
        }
    }

    fn add_user(&mut self, info: &Value, parts: &[Value]) {
        let blocks: Vec<ChatBlock> = parts.iter().filter_map(user_block).collect();

        if blocks.is_empty() {
            return;
        }

        let mut message = blank_message(info, MessageRole::User);
        message.blocks = blocks;

        self.tally.note_message();
        self.tally.note_time(message.timestamp);
        self.messages.push(message);
    }

    fn add_assistant(&mut self, info: &Value, parts: &[Value]) {
        let mut message = blank_message(info, MessageRole::Assistant);
        let mut results = Vec::new();

        for part in parts {
            match part_type(part) {
                "text" => message.blocks.extend(text_block(part)),
                "reasoning" => message.blocks.push(reasoning_block(part)),
                "tool" => {
                    let (call, result) = tool_call(part);
                    message.blocks.push(call);
                    results.extend(result);

                    self.note_touched(part);
                }
                _ => {}
            }
        }

        let error = info.get("error");
        message.error_message = error
            .and_then(|error| error.pointer("/data/message"))
            .and_then(Value::as_str)
            .map(redact_text);

        if message.blocks.is_empty() && message.error_message.is_none() {
            return;
        }

        message.model = text(info, "modelID");
        message.provider = text(info, "providerID");
        message.stop_reason = text(info, "finish").or_else(|| error.map(stop_reason_of));
        message.cost_usd = info.get("cost").and_then(Value::as_f64);
        message.usage = info.get("tokens").map(token_usage);

        self.tally.note_message();
        self.tally.note_time(message.timestamp);
        note_reply(&mut self.tally, &message);
        self.messages.push(message);
        self.messages.extend(results);
    }

    /// Records the files a `tool` part may have changed (see [`touched_paths`]).
    fn note_touched(&mut self, part: &Value) {
        let tool = part.get("tool").and_then(Value::as_str).unwrap_or_default();
        let input = part.pointer("/state/input").unwrap_or(&Value::Null);
        let mut raw_paths: Vec<&str> = Vec::new();

        if is_one_of(tool, &FILE_PATH_TOOLS)
            && let Some(path) = input.get("filePath").and_then(Value::as_str)
        {
            raw_paths.push(path);
        }

        if is_one_of(tool, &PATCH_TOOLS)
            && let Some(patch) = input.get("patchText").and_then(Value::as_str)
        {
            raw_paths.extend(patch_paths(patch));
        }

        for raw in raw_paths {
            // OpenCode resolves relative paths against the session's directory, not the project.
            let absolute = self.directory.join(raw);
            let args = json!({ "path": absolute.to_string_lossy() });

            self.touched.extend(touched_paths(self.project, "edit", &args));
        }
    }
}

fn is_one_of(tool: &str, names: &[&str]) -> bool {
    names.iter().any(|name| name.eq_ignore_ascii_case(tool))
}

/// The file paths named by the headers of an `apply_patch` patch.
fn patch_paths(patch: &str) -> impl Iterator<Item = &str> {
    patch.lines().filter_map(|line| {
        PATCH_HEADERS
            .iter()
            .find_map(|header| line.strip_prefix(header))
            .map(str::trim)
            .filter(|path| !path.is_empty())
    })
}

fn part_type(part: &Value) -> &str {
    part.get("type").and_then(Value::as_str).unwrap_or_default()
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn number(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| *number >= 0.0)
        .map_or(0, |number| number as u64)
}

/// A message of `role` with the id and creation time of `info` and no content yet.
fn blank_message(info: &Value, role: MessageRole) -> ChatMessage {
    ChatMessage {
        id: text(info, "id"),
        role,
        blocks: Vec::new(),
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: info.pointer("/time/created").and_then(Value::as_i64),
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    }
}

/// A user-visible text part; synthetic and ignored text is context OpenCode added itself.
fn text_block(part: &Value) -> Option<ChatBlock> {
    let hidden = ["synthetic", "ignored"]
        .iter()
        .any(|key| part.get(key).and_then(Value::as_bool) == Some(true));

    if hidden {
        return None;
    }

    text(part, "text").map(|text| ChatBlock::Text { text })
}

/// Reasoning; OpenCode keeps an empty text when the provider encrypted it.
fn reasoning_block(part: &Value) -> ChatBlock {
    let text = text(part, "text").unwrap_or_default();
    let redacted = text.is_empty();

    ChatBlock::Thinking { text, redacted }
}

fn user_block(part: &Value) -> Option<ChatBlock> {
    match part_type(part) {
        "text" => text_block(part),
        "file" => file_block(part),
        "subtask" => {
            let agent = text(part, "agent").unwrap_or_default();
            let prompt = text(part, "prompt").or_else(|| text(part, "description"))?;

            Some(ChatBlock::Text {
                text: format!("@{agent} {prompt}"),
            })
        }
        _ => None,
    }
}

/// An attachment: images by MIME type, other files as `@name`. The URL (possibly a `data:` URL
/// holding the content) is never used, and a file mentioned in the prompt text is skipped.
fn file_block(part: &Value) -> Option<ChatBlock> {
    let mime = text(part, "mime").unwrap_or_default();

    if mime.starts_with("image/") {
        return Some(ChatBlock::Image { mime_type: mime });
    }

    if part.get("source").is_some() {
        return None;
    }

    text(part, "filename").map(|name| ChatBlock::Text {
        text: format!("@{name}"),
    })
}

/// The tool call of a `tool` part, and its result message once the call finished.
fn tool_call(part: &Value) -> (ChatBlock, Option<ChatMessage>) {
    let id = text(part, "callID").or_else(|| text(part, "id")).unwrap_or_default();
    let name = text(part, "tool").unwrap_or_default();
    let state = part.get("state").unwrap_or(&Value::Null);

    let call = ChatBlock::ToolCall {
        id: id.clone(),
        name: name.clone(),
        args: state.get("input").cloned().unwrap_or_else(|| json!({})),
        intent: text(state, "title"),
    };

    let (output, is_error) = match state.get("status").and_then(Value::as_str) {
        Some("completed") => (text(state, "output").unwrap_or_default(), false),
        Some("error") => (text(state, "error").unwrap_or_default(), true),
        _ => return (call, None),
    };

    let result = ChatMessage {
        id: text(part, "id"),
        role: MessageRole::ToolResult,
        blocks: vec![ChatBlock::Text {
            text: redact_text(&output),
        }],
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: state.pointer("/time/end").and_then(Value::as_i64),
        tool_call_id: Some(id),
        tool_name: Some(name),
        is_error: Some(is_error),
    };

    (call, Some(result))
}

/// `tokens` of an assistant message; the total includes reasoning tokens.
fn token_usage(tokens: &Value) -> TokenUsage {
    let input = number(tokens, "/input");
    let output = number(tokens, "/output");
    let reasoning = number(tokens, "/reasoning");
    let cache_read = number(tokens, "/cache/read");
    let cache_write = number(tokens, "/cache/write");

    let summed = input + output + reasoning + cache_read + cache_write;
    let total = tokens
        .get("total")
        .and_then(Value::as_f64)
        .map_or(summed, |total| total.max(0.0) as u64);

    TokenUsage {
        input,
        output,
        cache_read,
        cache_write,
        total,
    }
}

/// The stop reason of a failed reply without `finish`.
fn stop_reason_of(error: &Value) -> String {
    let aborted = error.get("name").and_then(Value::as_str) == Some("MessageAbortedError");

    if aborted { "aborted" } else { "error" }.to_owned()
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
