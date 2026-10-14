//! `session_messages` and `session_files_touched`, read from the session file on disk.
//!
//! The transcript is the active path (root to leaf) as chat messages: `message` entries (system
//! prompts excluded), compaction and branch summaries, and displayed extension messages. It is
//! paged forward from the root: the first page holds the oldest messages, `nextCursor` points at
//! the next later page (`None` once the leaf is reached), and each page is oldest first. (The
//! live chat history of M2 pages the other way.) Two streaming passes keep memory to the
//! entry ids of the file plus one page: the first finds the path, the second normalizes only the
//! page's entries and, for the first page, tallies the [`TranscriptStats`] of the whole path.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde_json::{Value, json};

use super::parse::{Entries, Entry, epoch_millis};
use crate::error::{BusinessError, Result};
use crate::model::{ChatMessage, MAX_SESSION_PAGE, ModelTurns, SessionMessagesPage, TranscriptStats};
use crate::pi_family::normalize::chat_message;
use crate::pi_family::touched::touched_paths;

/// Entries indexed at most when locating the active path.
const MAX_INDEXED_ENTRIES: usize = 500_000;

/// One page of the active-path transcript of the session file at `path`.
///
/// `cursor` is `None` for the first (oldest) page or a previous page's `nextCursor`; `limit` is clamped
/// to `1..=`[`MAX_SESSION_PAGE`]. An unknown cursor fails with `InvalidInput`. Only the first page
/// carries `stats`.
pub fn session_messages(path: &Path, cursor: Option<&str>, limit: u32) -> Result<SessionMessagesPage> {
    let transcript = transcript_entries(path)?;
    let total = transcript.len();

    let start = match cursor {
        None => 0,
        Some(raw) => raw
            .parse::<usize>()
            .ok()
            .filter(|start| *start <= total)
            .ok_or_else(|| BusinessError::InvalidInput("unknown transcript cursor".to_owned()))?,
    };

    let limit = limit.clamp(1, MAX_SESSION_PAGE) as usize;
    let end = start.saturating_add(limit).min(total);
    let wanted: HashMap<usize, usize> = transcript[start..end]
        .iter()
        .enumerate()
        .map(|(slot, ordinal)| (*ordinal, slot))
        .collect();

    let mut page: Vec<Option<ChatMessage>> = vec![None; end - start];
    let mut tally = cursor.is_none().then(Tally::default);

    // Path ordinals ascend (parents precede children), so one index walks the path.
    let mut next_on_path = 0;

    for (ordinal, entry) in Entries::open(path)?.enumerate() {
        if let Some(&slot) = wanted.get(&ordinal) {
            page[slot] = transcript_message(&entry);
        }

        if transcript.get(next_on_path) == Some(&ordinal) {
            next_on_path += 1;

            if let Some(tally) = tally.as_mut() {
                tally.note(&entry);
            }
        }
    }

    Ok(SessionMessagesPage {
        messages: page.into_iter().flatten().collect(),
        next_cursor: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).unwrap_or(u32::MAX),
        stats: tally.map(Tally::finish),
    })
}

/// Running totals of the transcript entries on the active path.
#[derive(Default)]
struct Tally {
    messages: u32,
    tool_calls: u32,
    context_tokens: Option<u64>,
    cost_usd: Option<f64>,
    models: Vec<ModelTurns>,
    started_at: Option<i64>,
    ended_at: Option<i64>,
}

impl Tally {
    fn note(&mut self, entry: &Entry) {
        let message = entry.message();
        let message_time = message
            .and_then(|message| message.get("timestamp"))
            .and_then(Value::as_i64);
        let time = entry.text("timestamp").and_then(epoch_millis).or(message_time);

        self.started_at = self.started_at.or(time);
        self.ended_at = time.or(self.ended_at);

        match (entry.role(), message) {
            (Some("user"), _) => self.messages += 1,
            (Some("assistant"), Some(reply)) => {
                self.messages += 1;
                self.note_reply(reply);
            }
            _ => {}
        }
    }

    fn note_reply(&mut self, reply: &Value) {
        let blocks = reply.get("content").and_then(Value::as_array);
        let tool_calls = blocks
            .into_iter()
            .flatten()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("toolCall"))
            .count();
        self.tool_calls = self
            .tool_calls
            .saturating_add(u32::try_from(tool_calls).unwrap_or(u32::MAX));

        if let Some(model) = reply.get("model").and_then(Value::as_str) {
            match self.models.iter_mut().find(|turns| turns.model == model) {
                Some(turns) => turns.turns += 1,
                None => self.models.push(ModelTurns {
                    model: model.to_owned(),
                    turns: 1,
                }),
            }
        }

        let usage = reply.get("usage").unwrap_or(&Value::Null);

        if let Some(cost) = usage.pointer("/cost/total").and_then(Value::as_f64) {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + cost);
        }

        // Aborted and failed replies report no usage; the context is the last one that does.
        if let Some(tokens) = total_tokens(usage).filter(|tokens| *tokens > 0) {
            self.context_tokens = Some(tokens);
        }
    }

    fn finish(mut self) -> TranscriptStats {
        self.models.sort_by_key(|turns| std::cmp::Reverse(turns.turns));

        TranscriptStats {
            messages: self.messages,
            tool_calls: self.tool_calls,
            context_tokens: self.context_tokens,
            cost_usd: self.cost_usd,
            models: self.models,
            started_at: self.started_at,
            ended_at: self.ended_at,
        }
    }
}

/// `totalTokens` of a message's usage, or the sum of its parts.
fn total_tokens(usage: &Value) -> Option<u64> {
    if let Some(total) = usage.get("totalTokens").and_then(Value::as_u64) {
        return Some(total);
    }

    let parts = ["input", "output", "cacheRead", "cacheWrite"].map(|key| usage.get(key).and_then(Value::as_u64));
    parts
        .iter()
        .any(Option::is_some)
        .then(|| parts.into_iter().flatten().sum())
}

/// Project-relative paths of every file an edit tool call in the session (any branch) may have
/// changed, deduplicated and sorted. Uses the rules of [`touched_paths`].
pub fn session_files_touched(path: &Path, project: &Path) -> Result<Vec<String>> {
    let mut touched = BTreeSet::new();

    for entry in Entries::open(path)? {
        let Some(message) = entry.message() else {
            continue;
        };

        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }

        let blocks = message.get("content").and_then(Value::as_array);
        for block in blocks.into_iter().flatten() {
            if block.get("type").and_then(Value::as_str) != Some("toolCall") {
                continue;
            }

            let name = block.get("name").and_then(Value::as_str).unwrap_or_default();
            let args = block.get("arguments").unwrap_or(&Value::Null);
            touched.extend(touched_paths(project, name, args));
        }
    }

    Ok(touched.into_iter().collect())
}

/// The file ordinals (among visible entries) of the transcript messages on the active path, in
/// path order.
fn transcript_entries(path: &Path) -> Result<Vec<usize>> {
    let mut parents: Vec<Option<usize>> = Vec::new();
    let mut shown: Vec<bool> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();

    for entry in Entries::open(path)?.take(MAX_INDEXED_ENTRIES) {
        let parent = entry.parent_id.as_ref().and_then(|id| positions.get(id).copied());

        positions.insert(entry.id.clone(), parents.len());
        parents.push(parent);
        shown.push(is_transcript(&entry));
    }

    let mut path_ordinals = Vec::new();
    let mut visited = HashSet::new();
    let mut current = parents.len().checked_sub(1);

    while let Some(ordinal) = current {
        if !visited.insert(ordinal) {
            break;
        }

        if shown[ordinal] {
            path_ordinals.push(ordinal);
        }
        current = parents[ordinal];
    }

    path_ordinals.reverse();
    Ok(path_ordinals)
}

/// True for entries shown in the transcript (see the module docs).
fn is_transcript(entry: &Entry) -> bool {
    match entry.kind() {
        "message" => !matches!(entry.role(), Some("system" | "developer") | None),
        "compaction" => true,
        "branch_summary" => entry.text("summary").is_some_and(|summary| !summary.is_empty()),
        "custom_message" => entry.value.get("display").and_then(Value::as_bool) != Some(false),
        _ => false,
    }
}

/// The chat message of a transcript entry, with the entry id.
fn transcript_message(entry: &Entry) -> Option<ChatMessage> {
    let timestamp = entry.text("timestamp").and_then(epoch_millis);

    let raw = match entry.kind() {
        "message" => entry.message()?.clone(),
        "compaction" => json!({ "role": "compactionSummary", "summary": entry.value.get("summary") }),
        "branch_summary" => json!({ "role": "branchSummary", "summary": entry.value.get("summary") }),
        "custom_message" => json!({ "role": "custom", "content": entry.value.get("content") }),
        _ => return None,
    };

    let mut message = chat_message(&raw);
    message.id = Some(entry.id.clone());
    message.timestamp = message.timestamp.or(timestamp);

    Some(message)
}
