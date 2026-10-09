//! `session_tree`: every visible entry of a session with the active path marked.
//!
//! The leaf is the last entry in the file: OMP and PI rebuild the active branch from the last
//! appended entry when a session is loaded, and every append goes to the current leaf. At most
//! [`MAX_TREE_NODES`] entries are read; a longer session is cut (`truncated`) and its leaf is
//! then the last entry that was kept.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::parse::{Entries, Entry, preview_text};
use crate::error::Result;
use crate::model::{MAX_NODE_PREVIEW_CHARS, MAX_TREE_NODES, SessionNode, SessionNodeKind, SessionRef, SessionTree};

/// A tree with the number of lines that could not be read.
#[derive(Debug, Clone)]
pub struct TreeReport {
    /// The tree.
    pub tree: SessionTree,
    /// Skipped or malformed lines.
    pub bad_lines: usize,
}

/// The tree of the session file at `path`.
pub fn session_tree(path: &Path, session: &SessionRef) -> Result<SessionTree> {
    read_tree(path, session).map(|report| report.tree)
}

/// [`session_tree`] with the count of unreadable lines (for diagnostics).
pub fn read_tree(path: &Path, session: &SessionRef) -> Result<TreeReport> {
    let mut entries = Entries::open(path)?;
    let mut nodes: Vec<SessionNode> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut labels: HashMap<String, Option<String>> = HashMap::new();
    let mut truncated = false;

    for entry in entries.by_ref() {
        if nodes.len() >= MAX_TREE_NODES {
            truncated = true;
            break;
        }

        // The latest label entry for a target wins; an empty or missing label clears it.
        if entry.kind() == "label"
            && let Some(target) = entry.text("targetId")
        {
            let label = entry.text("label").filter(|label| !label.is_empty());
            labels.insert(target.to_owned(), label.map(str::to_owned));
        }

        let parent_id = entry.parent_id.clone().filter(|parent| index.contains_key(parent));
        if let Some(parent) = &parent_id {
            nodes[index[parent]].child_count += 1;
        }

        index.insert(entry.id.clone(), nodes.len());
        nodes.push(node(&entry, parent_id));
    }

    for (target, label) in labels {
        if let Some(&position) = index.get(&target) {
            nodes[position].label = label;
        }
    }

    let leaf_id = nodes.last().map(|node| node.id.clone());
    mark_active_path(&mut nodes, &index, leaf_id.as_deref());

    let branch_count = nodes.iter().filter(|node| node.child_count > 1).count();
    let tree = SessionTree {
        session: session.clone(),
        nodes,
        leaf_id,
        branch_count: u32::try_from(branch_count).unwrap_or(u32::MAX),
        truncated,
    };

    Ok(TreeReport {
        tree,
        bad_lines: entries.bad_lines(),
    })
}

/// What an entry is in the tree.
pub fn node_kind(entry: &Entry) -> SessionNodeKind {
    use SessionNodeKind as K;

    match entry.kind() {
        "message" => match entry.role().unwrap_or_default() {
            "user" => K::User,
            "assistant" => K::Assistant,
            "toolResult" => K::ToolResult,
            "bashExecution" => K::Bash,
            "branchSummary" => K::BranchSummary,
            "compactionSummary" => K::Compaction,
            _ => K::Other,
        },
        "compaction" => K::Compaction,
        "branch_summary" => K::BranchSummary,
        "label" => K::Label,
        "model_change" => K::ModelChange,
        _ => K::Other,
    }
}

fn node(entry: &Entry, parent_id: Option<String>) -> SessionNode {
    let kind = node_kind(entry);

    let preview = match kind {
        SessionNodeKind::User | SessionNodeKind::Assistant => entry
            .message()
            .and_then(|message| preview_text(message.get("content"), MAX_NODE_PREVIEW_CHARS)),
        _ => None,
    };

    let label = match kind {
        SessionNodeKind::Label => entry.text("label").map(str::to_owned),
        _ => None,
    };

    SessionNode {
        id: entry.id.clone(),
        parent_id,
        kind,
        preview,
        timestamp: entry.text("timestamp").map(str::to_owned),
        label,
        on_active_path: false,
        child_count: 0,
    }
}

/// Marks the chain from `leaf` up to its root.
fn mark_active_path(nodes: &mut [SessionNode], index: &HashMap<String, usize>, leaf: Option<&str>) {
    let mut visited = HashSet::new();
    let mut current = leaf.and_then(|id| index.get(id).copied());

    while let Some(position) = current {
        if !visited.insert(position) {
            break;
        }

        nodes[position].on_active_path = true;
        current = nodes[position]
            .parent_id
            .as_deref()
            .and_then(|parent| index.get(parent).copied());
    }
}
