//! `session_messages` and `session_files_touched` from a Claude Code session file.
//!
//! The transcript is the active path (root to leaf) as chat messages, oldest first and paged
//! forward like the PI-family transcripts. The file is read twice: once for the tree
//! ([`Index`]), once to convert only the lines on the path. Each line becomes:
//!
//! - a prompt: a user message (images keep only their MIME type), a slash command as typed
//!   (`/compact focus`);
//! - a shell run (`<bash-input>`, `<bash-stdout>`, `<bash-stderr>`): a `bash` message
//!   `$ command` then its output, as the other harnesses show `!command` runs. Output on a line of
//!   its own joins the run before it;
//! - a command's output (`<local-command-stdout>`): a system message with the plain text;
//! - tool results: one tool result message per result, named after the tool call they answer;
//! - a reply line: its blocks, added to the reply before when both are lines of one API reply
//!   (tool results of that reply may come between);
//! - the compaction summary: a compaction summary message.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::path::Path;

use serde_json::{Value, json};

use super::entries::{Index, Kind, command_text, kind, lines, tagged, text, text_kind};
use crate::claude_stream::tools::touched_paths;
use crate::error::Result;
use crate::model::{ChatBlock, ChatMessage, MessageRole, SessionMessagesPage, TokenUsage};
use crate::pi_family::normalize::plain_text;
use crate::pi_family::sessions::{StatsTally, epoch_millis, page_range};

/// Entries indexed at most when locating the active path.
const MAX_INDEXED_ENTRIES: usize = 500_000;

/// The model name Claude Code gives replies it made up itself (errors, interruptions).
const SYNTHETIC_MODEL: &str = "<synthetic>";

/// One page of the active-path transcript of the session file at `path` (see
/// [`crate::pi_family::sessions::session_messages`] for the cursor). Only the first page carries
/// the totals.
pub(in crate::harnesses::claude) fn session_messages(
    path: &Path,
    cursor: Option<&str>,
    limit: u32,
) -> Result<SessionMessagesPage> {
    let messages = transcript(path)?;
    let total = messages.len();
    let Range { start, end } = page_range(cursor, limit, total)?;

    let stats = cursor.is_none().then(|| {
        let mut tally = StatsTally::default();
        messages.iter().for_each(|message| note(&mut tally, message));
        tally.finish()
    });

    Ok(SessionMessagesPage {
        messages: messages[start..end].to_vec(),
        next_cursor: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).unwrap_or(u32::MAX),
        stats,
    })
}

/// Project-relative paths of every file an edit tool call in the session (any branch, subagent
/// lines left out) may have changed, sorted (the rules of [`touched_paths`]).
pub(in crate::harnesses::claude) fn session_files_touched(path: &Path, project: &Path) -> Result<Vec<String>> {
    let mut touched = BTreeSet::new();

    for (_, line) in lines(path)? {
        if kind(&line) != Some(Kind::Assistant) {
            continue;
        }

        for block in blocks(&line) {
            if text(block, "type") != Some("tool_use") {
                continue;
            }

            let name = text(block, "name").unwrap_or_default();
            let input = block.get("input").unwrap_or(&Value::Null);
            touched.extend(touched_paths(project, name, input));
        }
    }

    Ok(touched.into_iter().collect())
}

/// The chat messages of the active path.
pub(super) fn transcript(path: &Path) -> Result<Vec<ChatMessage>> {
    let index = Index::read(path, MAX_INDEXED_ENTRIES)?;
    let wanted: HashSet<usize> = index
        .active_path()
        .into_iter()
        .flat_map(|position| index.nodes[position].lines.iter().copied())
        .collect();

    let mut builder = Builder::default();

    // Lines on the path come in path order: an entry is only made after its parent's lines.
    for (ordinal, line) in lines(path)? {
        if wanted.contains(&ordinal)
            && let Some(kind) = kind(&line)
        {
            builder.add(kind, &line);
        }
    }

    Ok(builder.messages)
}

/// Collects the messages of one transcript.
#[derive(Default)]
struct Builder {
    messages: Vec<ChatMessage>,
    /// `message.id` of each reply in `messages`, by position.
    api_ids: HashMap<usize, String>,
    /// Tool names by tool call id.
    tools: HashMap<String, String>,
}

impl Builder {
    fn add(&mut self, kind: Kind, line: &Value) {
        let id = text(line, "uuid").map(str::to_owned);
        let timestamp = text(line, "timestamp").and_then(epoch_millis);
        let content = line.pointer("/message/content");

        match kind {
            Kind::Assistant => self.add_reply(line, id, timestamp),
            Kind::ToolResult => self.add_results(content, id, timestamp),
            Kind::Compaction => {
                let summary = compaction_summary(&user_text(content));
                self.push(message(
                    MessageRole::CompactionSummary,
                    vec![text_block(summary)],
                    id,
                    timestamp,
                ));
            }
            Kind::Bash | Kind::BashOutput => self.add_shell(&user_text(content), id, timestamp),
            Kind::LocalOutput => {
                let raw = match content {
                    Some(_) => user_text(content),
                    None => text(line, "content").unwrap_or_default().to_owned(),
                };
                let output = local_output(&raw);

                if !output.is_empty() {
                    self.push(message(MessageRole::System, vec![text_block(output)], id, timestamp));
                }
            }
            Kind::Command => {
                let typed = command_text(&user_text(content));
                self.push(message(MessageRole::User, vec![text_block(typed)], id, timestamp));
            }
            Kind::User => {
                let blocks = content_blocks(content);
                self.push(message(MessageRole::User, blocks, id, timestamp));
            }
        }
    }

    fn push(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    fn add_reply(&mut self, line: &Value, id: Option<String>, timestamp: Option<i64>) {
        let reply = line.get("message").unwrap_or(&Value::Null);
        let api_id = text(reply, "id").map(str::to_owned);
        let new_blocks: Vec<ChatBlock> = blocks(line).iter().map(reply_block).collect();

        for block in &new_blocks {
            if let ChatBlock::ToolCall { id, name, .. } = block {
                self.tools.insert(id.clone(), name.clone());
            }
        }

        let target = api_id.as_ref().and_then(|api_id| self.reply_of(api_id));
        let position = match target {
            Some(position) => {
                let merged = &mut self.messages[position];
                merged.blocks.extend(new_blocks);
                merged.id = id.or(merged.id.take());
                position
            }
            None => {
                self.push(message(MessageRole::Assistant, new_blocks, id, timestamp));
                self.messages.len() - 1
            }
        };

        if let Some(api_id) = api_id {
            self.api_ids.insert(position, api_id);
        }

        let merged = &mut self.messages[position];
        let model = text(reply, "model").filter(|model| *model != SYNTHETIC_MODEL);
        merged.model = model.map(str::to_owned).or(merged.model.take());
        merged.stop_reason = text(reply, "stop_reason")
            .map(str::to_owned)
            .or(merged.stop_reason.take());
        merged.usage = reply.get("usage").map(usage).or(merged.usage);

        if line.get("isApiErrorMessage").and_then(Value::as_bool) == Some(true) {
            merged.stop_reason = Some("error".to_owned());
            merged.error_message = Some(block_text(&merged.blocks));
        }
    }

    /// The reply in `messages` with API message `api_id`, when only tool results follow it.
    fn reply_of(&self, api_id: &str) -> Option<usize> {
        for (position, message) in self.messages.iter().enumerate().rev() {
            if message.role == MessageRole::ToolResult {
                continue;
            }

            let same =
                message.role == MessageRole::Assistant && self.api_ids.get(&position).is_some_and(|id| id == api_id);
            return same.then_some(position);
        }

        None
    }

    fn add_results(&mut self, content: Option<&Value>, id: Option<String>, timestamp: Option<i64>) {
        let blocks = content.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        let mut others = Vec::new();

        for block in blocks {
            if text(block, "type") != Some("tool_result") {
                others.push(block.clone());
                continue;
            }

            let call = text(block, "tool_use_id").map(str::to_owned);
            let mut result = message(
                MessageRole::ToolResult,
                content_blocks(block.get("content")),
                id.clone(),
                timestamp,
            );
            result.tool_name = call.as_ref().and_then(|call| self.tools.get(call).cloned());
            result.tool_call_id = call;
            result.is_error = Some(block.get("is_error").and_then(Value::as_bool) == Some(true));
            self.push(result);
        }

        // Text sent along with the results (rare) is a prompt of its own.
        if !others.is_empty() {
            let blocks = content_blocks(Some(&Value::Array(others)));
            self.push(message(MessageRole::User, blocks, id, timestamp));
        }
    }

    fn add_shell(&mut self, raw: &str, id: Option<String>, timestamp: Option<i64>) {
        let output = shell_output(raw);

        if text_kind(raw) == Kind::BashOutput
            && let Some(ChatMessage {
                role: MessageRole::Bash,
                blocks,
                ..
            }) = self.messages.last_mut()
            && let Some(ChatBlock::Text { text }) = blocks.first_mut()
            && text.ends_with('\n')
        {
            text.push_str(&output);
            return;
        }

        let shown = match tagged(raw, "bash-input") {
            Some(command) => format!("$ {}\n{output}", command.trim()),
            None => output,
        };

        self.push(message(MessageRole::Bash, vec![text_block(shown)], id, timestamp));
    }
}

/// The content blocks of a line's message.
fn blocks(line: &Value) -> &[Value] {
    line.pointer("/message/content")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn message(role: MessageRole, blocks: Vec<ChatBlock>, id: Option<String>, timestamp: Option<i64>) -> ChatMessage {
    ChatMessage {
        id,
        role,
        blocks,
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp,
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    }
}

fn text_block(text: String) -> ChatBlock {
    ChatBlock::Text { text }
}

/// The text of user content (a string or its text blocks).
fn user_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(raw)) => raw.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| text(block, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn block_text(blocks: &[ChatBlock]) -> String {
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    texts.join("\n")
}

/// The blocks of a prompt or a tool result's `content`: a string, or text and image blocks
/// (images keep their MIME type only).
fn content_blocks(content: Option<&Value>) -> Vec<ChatBlock> {
    match content {
        Some(Value::String(raw)) => vec![text_block(raw.clone())],
        Some(Value::Array(blocks)) => blocks.iter().map(content_block).collect(),
        _ => Vec::new(),
    }
}

/// A text or image block; anything else is kept as its type.
fn content_block(block: &Value) -> ChatBlock {
    match text(block, "type").unwrap_or_default() {
        "text" => text_block(text(block, "text").unwrap_or_default().to_owned()),
        "image" => ChatBlock::Image {
            mime_type: block
                .pointer("/source/media_type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        },
        other => ChatBlock::Other { kind: other.to_owned() },
    }
}

/// A reply block: text, reasoning (empty reasoning counts as withheld) and tool calls.
fn reply_block(block: &Value) -> ChatBlock {
    match text(block, "type").unwrap_or_default() {
        "thinking" => {
            let thinking = text(block, "thinking").unwrap_or_default().to_owned();
            let redacted = thinking.is_empty();

            ChatBlock::Thinking {
                text: thinking,
                redacted,
            }
        }
        "redacted_thinking" => ChatBlock::Thinking {
            text: String::new(),
            redacted: true,
        },
        "tool_use" | "server_tool_use" => ChatBlock::ToolCall {
            id: text(block, "id").unwrap_or_default().to_owned(),
            name: text(block, "name").unwrap_or_default().to_owned(),
            args: block.get("input").cloned().unwrap_or_else(|| json!({})),
            intent: None,
        },
        _ => content_block(block),
    }
}

/// Token counts of a reply's `usage`.
fn usage(value: &Value) -> TokenUsage {
    let count = |key: &str| value.get(key).and_then(Value::as_u64).unwrap_or(0);

    let input = count("input_tokens");
    let output = count("output_tokens");
    let cache_read = count("cache_read_input_tokens");
    let cache_write = count("cache_creation_input_tokens");

    TokenUsage {
        input,
        output,
        cache_read,
        cache_write,
        total: input + output + cache_read + cache_write,
    }
}

/// `stdout` then `stderr` of a shell run, each without terminal escapes, joined by a line break.
fn shell_output(raw: &str) -> String {
    let parts: Vec<String> = ["bash-stdout", "bash-stderr"]
        .iter()
        .filter_map(|tag| tagged(raw, tag))
        .map(|part| plain_text(part.trim_end()))
        .filter(|part| !part.is_empty())
        .collect();

    parts.join("\n")
}

/// The plain text of a command's output.
fn local_output(raw: &str) -> String {
    let inner = tagged(raw, "local-command-stdout")
        .or_else(|| tagged(raw, "local-command-stderr"))
        .unwrap_or(raw);

    plain_text(inner.trim())
}

/// The summary of a compaction message: the part after its `Summary:` line when Claude Code's
/// framing is there, else the whole text.
fn compaction_summary(raw: &str) -> String {
    const START: &str = "\nSummary:\n";
    const END: &str = "\n\nIf you need specific details from before compaction";

    let Some(start) = raw.find(START).map(|start| start + START.len()) else {
        return raw.trim().to_owned();
    };

    let rest = &raw[start..];
    let summary = rest.find(END).map_or(rest, |end| &rest[..end]);

    summary.trim().to_owned()
}

/// Adds one message to the totals: tool results are not messages of their own.
fn note(tally: &mut StatsTally, message: &ChatMessage) {
    if message.role == MessageRole::ToolResult {
        return;
    }

    tally.note_time(message.timestamp);

    if !matches!(message.role, MessageRole::User | MessageRole::Assistant) {
        return;
    }

    tally.note_message();

    if message.role != MessageRole::Assistant {
        return;
    }

    let tool_calls = message
        .blocks
        .iter()
        .filter(|block| matches!(block, ChatBlock::ToolCall { .. }))
        .count();
    let context_tokens = message.usage.map(|usage| usage.total);

    tally.note_reply(tool_calls, message.model.as_deref(), None, context_tokens);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_drops_claude_codes_framing() {
        let raw = "This session is being continued.\n\n<tools-used-before-summary></tools-used-before-summary>\nSummary:\npong\nping\n\nIf you need specific details from before compaction, read the transcript.";
        assert_eq!(compaction_summary(raw), "pong\nping");
        assert_eq!(compaction_summary("  just a summary "), "just a summary");
    }

    #[test]
    fn shell_output_joins_stdout_and_stderr() {
        let raw = "<bash-stdout>out\n</bash-stdout><bash-stderr>\u{1b}[31merr\u{1b}[0m</bash-stderr>";
        assert_eq!(shell_output(raw), "out\nerr");
        assert_eq!(
            shell_output("<bash-stdout></bash-stdout><bash-stderr></bash-stderr>"),
            ""
        );
    }
}
