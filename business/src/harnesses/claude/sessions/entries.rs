//! The lines of a Claude Code session file and the tree they form (see the module docs of
//! [`super`]), plus `session_tree`.
//!
//! [`Index::read`] streams the file once and keeps, per shown entry, its id, parent, kind and the
//! file lines it is made of: consecutive `assistant` lines of one API reply become one entry named
//! after its last line, so forking from it keeps the whole reply. Hidden lines are spliced out of
//! the chain. A parent that has not appeared earlier in the file is unknown, so the entry is a root
//! (this also breaks the cycle a cloned compaction would form: there the preserved reply follows
//! the boundary that names it).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use crate::error::Result;
use crate::model::{MAX_NODE_PREVIEW_CHARS, MAX_TREE_NODES, SessionNode, SessionNodeKind, SessionRef, SessionTree};
use crate::pi_family::sessions::{Lines, preview_text};

/// What a shown line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// A prompt the user typed.
    User,
    /// A slash command the user ran (`<command-name>`).
    Command,
    /// A shell command the user ran (`<bash-input>`, maybe with its output).
    Bash,
    /// The output of the shell command before it (`<bash-stdout>`, `<bash-stderr>`).
    BashOutput,
    /// The output of a slash command (`<local-command-stdout>`, a `local_command` line).
    LocalOutput,
    /// Tool results.
    ToolResult,
    /// A model reply (one content block per line).
    Assistant,
    /// The compaction summary.
    Compaction,
}

impl Kind {
    fn node_kind(self) -> SessionNodeKind {
        match self {
            Self::User | Self::Command => SessionNodeKind::User,
            Self::Bash | Self::BashOutput => SessionNodeKind::Bash,
            Self::LocalOutput => SessionNodeKind::Other,
            Self::ToolResult => SessionNodeKind::ToolResult,
            Self::Assistant => SessionNodeKind::Assistant,
            Self::Compaction => SessionNodeKind::Compaction,
        }
    }
}

/// A string field.
pub(super) fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool) == Some(true)
}

/// The kind of a shown line; `None` for every line that is not shown (see the module docs).
pub(super) fn kind(line: &Value) -> Option<Kind> {
    if flag(line, "isSidechain") || flag(line, "isMeta") {
        return None;
    }

    match text(line, "type")? {
        "user" if flag(line, "isCompactSummary") => Some(Kind::Compaction),
        "user" => user_kind(line.pointer("/message/content")?),
        "assistant" => line.get("message").map(|_| Kind::Assistant),
        "system" if text(line, "subtype") == Some("local_command") => Some(Kind::LocalOutput),
        _ => None,
    }
}

fn user_kind(content: &Value) -> Option<Kind> {
    match content {
        Value::String(raw) => Some(text_kind(raw)),
        Value::Array(blocks) => {
            let block_type = |block: &Value| text(block, "type").unwrap_or_default().to_owned();

            if blocks.iter().any(|block| block_type(block) == "tool_result") {
                return Some(Kind::ToolResult);
            }

            let first_text = blocks
                .iter()
                .find(|block| block_type(block) == "text")
                .and_then(|block| text(block, "text"));

            match first_text {
                Some(raw) => Some(text_kind(raw)),
                None => (!blocks.is_empty()).then_some(Kind::User),
            }
        }
        _ => None,
    }
}

/// The kind of user text by the tag it starts with.
pub(super) fn text_kind(raw: &str) -> Kind {
    let start = raw.trim_start();
    let starts = |tags: &[&str]| tags.iter().any(|tag| start.starts_with(tag));

    if starts(&["<bash-input>"]) {
        Kind::Bash
    } else if starts(&["<bash-stdout>", "<bash-stderr>"]) {
        Kind::BashOutput
    } else if starts(&["<command-name>", "<command-message>"]) {
        Kind::Command
    } else if starts(&["<local-command-stdout>", "<local-command-stderr>"]) {
        Kind::LocalOutput
    } else {
        Kind::User
    }
}

/// The text between `<tag>` and `</tag>` in `raw`.
pub(super) fn tagged<'a>(raw: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = raw.find(&open)? + open.len();
    let end = raw[start..].find(&close)? + start;

    Some(&raw[start..end])
}

/// The JSON lines of the session file at `path` with their position in the file; lines that are
/// not JSON are skipped (and still counted).
pub(super) fn lines(path: &Path) -> Result<impl Iterator<Item = (usize, Value)>> {
    let lines = Lines::open(path, u64::MAX)?;

    Ok(lines
        .enumerate()
        .filter_map(|(ordinal, line)| Some((ordinal, serde_json::from_str::<Value>(&line).ok()?))))
}

/// One shown entry.
#[derive(Debug)]
pub(super) struct Node {
    /// The `uuid` of its last line.
    pub id: String,
    pub parent: Option<usize>,
    pub kind: Kind,
    /// The file lines it is made of, in file order.
    pub lines: Vec<usize>,
    /// `message.id` of a reply.
    pub api_id: Option<String>,
    pub children: u32,
    pub preview: Option<String>,
    pub timestamp: Option<String>,
}

/// The shown entries of a session file.
#[derive(Debug, Default)]
pub(super) struct Index {
    pub nodes: Vec<Node>,
    /// The entry of the last message line.
    pub leaf: Option<usize>,
    /// True when the file had more than the allowed entries.
    pub truncated: bool,
}

impl Index {
    /// Reads at most `max_nodes` entries of the session file at `path`.
    pub(super) fn read(path: &Path, max_nodes: usize) -> Result<Self> {
        let mut index = Self::default();
        // Every line id seen: shown ones name their entry, hidden ones their shown ancestor.
        let mut ids: HashMap<String, Option<usize>> = HashMap::new();

        for (ordinal, line) in lines(path)? {
            let Some(uuid) = text(&line, "uuid").map(str::to_owned) else {
                continue;
            };

            if ids.contains_key(&uuid) {
                continue;
            }

            let parent = parent_uuid(&line).and_then(|parent| ids.get(parent).copied().flatten());

            let Some(kind) = kind(&line) else {
                ids.insert(uuid, parent);
                continue;
            };

            if let Some(position) = index.absorb(parent, kind, &line, &uuid, ordinal) {
                ids.insert(uuid, Some(position));
                continue;
            }

            if index.nodes.len() >= max_nodes {
                index.truncated = true;
                break;
            }

            if let Some(parent) = parent {
                index.nodes[parent].children += 1;
            }

            ids.insert(uuid.clone(), Some(index.nodes.len()));
            index.leaf = Some(index.nodes.len());
            index.nodes.push(Node {
                id: uuid,
                parent,
                kind,
                lines: vec![ordinal],
                api_id: api_id(&line),
                children: 0,
                preview: preview(kind, &line),
                timestamp: text(&line, "timestamp").map(str::to_owned),
            });
        }

        Ok(index)
    }

    /// Adds a reply line to its parent entry when that is an earlier line of the same reply
    /// without other children; returns the entry.
    fn absorb(&mut self, parent: Option<usize>, kind: Kind, line: &Value, uuid: &str, ordinal: usize) -> Option<usize> {
        let position = parent?;
        let node = &mut self.nodes[position];
        let same_reply = node.api_id.is_some() && node.api_id == api_id(line);

        if kind != Kind::Assistant || node.kind != Kind::Assistant || !same_reply || node.children > 0 {
            return None;
        }

        uuid.clone_into(&mut node.id);
        node.lines.push(ordinal);
        node.preview = node.preview.take().or_else(|| preview(kind, line));
        self.leaf = Some(position);

        Some(position)
    }

    /// The entries from the root to the leaf.
    pub(super) fn active_path(&self) -> Vec<usize> {
        let mut path = Vec::new();
        let mut visited = HashSet::new();
        let mut current = self.leaf;

        while let Some(position) = current {
            if !visited.insert(position) {
                break;
            }

            path.push(position);
            current = self.nodes[position].parent;
        }

        path.reverse();
        path
    }
}

/// The parent a line names: `parentUuid`, or for a compaction boundary the message before it.
fn parent_uuid(line: &Value) -> Option<&str> {
    text(line, "parentUuid").or_else(|| {
        let boundary = text(line, "type") == Some("system") && text(line, "subtype") == Some("compact_boundary");
        boundary.then(|| text(line, "logicalParentUuid")).flatten()
    })
}

/// `message.id` of an `assistant` line.
fn api_id(line: &Value) -> Option<String> {
    line.pointer("/message/id").and_then(Value::as_str).map(str::to_owned)
}

/// The preview of a prompt, command or reply line.
fn preview(kind: Kind, line: &Value) -> Option<String> {
    let content = line.pointer("/message/content");

    match kind {
        Kind::User | Kind::Assistant => preview_text(content, MAX_NODE_PREVIEW_CHARS),
        Kind::Command => {
            let raw = content
                .and_then(Value::as_str)
                .or_else(|| content?.as_array()?.iter().find_map(|block| text(block, "text")))?;

            preview_text(Some(&Value::String(command_text(raw))), MAX_NODE_PREVIEW_CHARS)
        }
        _ => None,
    }
}

/// A slash command line as typed: `/name args`.
pub(super) fn command_text(raw: &str) -> String {
    let name = tagged(raw, "command-name").map(str::trim);
    let name = name
        .or_else(|| tagged(raw, "command-message").map(str::trim))
        .unwrap_or_default();
    let args = tagged(raw, "command-args").map(str::trim).unwrap_or_default();

    let name = if name.starts_with('/') || name.is_empty() {
        name.to_owned()
    } else {
        format!("/{name}")
    };

    if args.is_empty() {
        name
    } else {
        format!("{name} {args}")
    }
}

/// The branch tree of the session file at `path`, the active path marked.
pub(in crate::harnesses::claude) fn session_tree(path: &Path, session: &SessionRef) -> Result<SessionTree> {
    let index = Index::read(path, MAX_TREE_NODES)?;
    let on_path: HashSet<usize> = index.active_path().into_iter().collect();

    let nodes: Vec<SessionNode> = index
        .nodes
        .iter()
        .enumerate()
        .map(|(position, node)| SessionNode {
            id: node.id.clone(),
            parent_id: node.parent.map(|parent| index.nodes[parent].id.clone()),
            kind: node.kind.node_kind(),
            preview: node.preview.clone(),
            timestamp: node.timestamp.clone(),
            label: None,
            on_active_path: on_path.contains(&position),
            child_count: node.children,
        })
        .collect();

    let branch_count = nodes.iter().filter(|node| node.child_count > 1).count();

    Ok(SessionTree {
        session: session.clone(),
        leaf_id: index.leaf.map(|leaf| index.nodes[leaf].id.clone()),
        nodes,
        branch_count: u32::try_from(branch_count).unwrap_or(u32::MAX),
        truncated: index.truncated,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn user_text_is_told_apart_by_its_leading_tag() {
        assert_eq!(text_kind("Fix the parser"), Kind::User);
        assert_eq!(text_kind("<bash-input>ls</bash-input>"), Kind::Bash);
        assert_eq!(
            text_kind("<bash-stdout>a</bash-stdout><bash-stderr></bash-stderr>"),
            Kind::BashOutput
        );
        assert_eq!(text_kind("<command-name>/compact</command-name>"), Kind::Command);
        assert_eq!(
            text_kind("<local-command-stdout>Compacted</local-command-stdout>"),
            Kind::LocalOutput
        );
    }

    #[test]
    fn hidden_lines_have_no_kind() {
        let user = |extra: Value| {
            let mut line = json!({"type": "user", "uuid": "u", "message": {"role": "user", "content": "hi"}});
            line.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            line
        };

        assert_eq!(kind(&user(json!({}))), Some(Kind::User));
        assert_eq!(kind(&user(json!({"isSidechain": true}))), None);
        assert_eq!(kind(&user(json!({"isMeta": true}))), None);
        assert_eq!(kind(&json!({"type": "attachment", "uuid": "a"})), None);
        assert_eq!(kind(&json!({"type": "api-request-blob", "uuid": "b"})), None);
        assert_eq!(kind(&json!({"type": "system", "subtype": "compact_boundary"})), None);
    }

    #[test]
    fn commands_read_as_typed() {
        let raw = "<command-name>/model</command-name>\n<command-message>model</command-message>\n<command-args>sonnet</command-args>";
        assert_eq!(command_text(raw), "/model sonnet");
        assert_eq!(command_text("<command-message>compact</command-message>"), "/compact");
    }
}
