//! `session_messages` and `session_files_touched`, read from the session file on disk.
//!
//! The transcript is the active path (root to leaf) as chat messages: `message` entries (system
//! prompts excluded), compaction and branch summaries, and displayed extension messages. It is
//! paged forward from the root: the first page holds the oldest messages, `nextCursor` points at
//! the next later page (`None` once the leaf is reached), and each page is oldest first. (The
//! live chat history of M2 pages the other way.) Two streaming passes keep memory to the
//! entry ids of the file plus one page: the first finds the path, the second normalizes only the
//! page's entries.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde_json::{Value, json};

use super::parse::{Entries, Entry, epoch_millis};
use crate::error::{BusinessError, Result};
use crate::model::{ChatMessage, MAX_SESSION_PAGE, SessionMessagesPage};
use crate::pi_family::normalize::chat_message;
use crate::pi_family::touched::touched_paths;

/// Entries indexed at most when locating the active path.
const MAX_INDEXED_ENTRIES: usize = 500_000;

/// One page of the active-path transcript of the session file at `path`.
///
/// `cursor` is `None` for the first (oldest) page or a previous page's `nextCursor`; `limit` is clamped
/// to `1..=`[`MAX_SESSION_PAGE`]. An unknown cursor fails with `InvalidInput`.
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

    for (ordinal, entry) in Entries::open(path)?.enumerate() {
        if let Some(&slot) = wanted.get(&ordinal) {
            page[slot] = transcript_message(&entry);
        }
    }

    Ok(SessionMessagesPage {
        messages: page.into_iter().flatten().collect(),
        next_cursor: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).unwrap_or(u32::MAX),
    })
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
