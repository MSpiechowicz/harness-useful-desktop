//! Tool items as tool cards: commands ([`SHELL`], args `command` and `cwd`), file changes
//! ([`APPLY_PATCH`], args `path` and `changes`), MCP calls (`mcp__<server>__<tool>`, args as
//! called) and web searches ([`WEB_SEARCH`], arg `query`).
//!
//! - `item/started` starts the card (`toolStart`), `item/completed` ends it (`toolEnd`, starting it
//!   first when its start was missed) with its redacted result: a command's output, a file
//!   change's patches, an MCP call's text content. A declined command or change fails with
//!   [`COMMAND_DECLINED`] / [`FILE_CHANGE_DECLINED`], a failed one with its output or error.
//! - A command's output streams as `toolUpdate` (the output so far) up to [`LIVE_OUTPUT_BYTES`];
//!   an MCP call's progress messages stream the same way.
//! - A file change that applied names the project files it touched (also the target of a move),
//!   inside the project only. Its files stay known from `item/started` (and
//!   `item/fileChange/patchUpdated`) until the turn ends: the approval Codex asks for names only
//!   the item.
//! - A card still running when its turn ends fails with [`UNFINISHED_TOOL`], one whose end could
//!   not be read with [`RESULT_TOO_LARGE`] (a file change still names the project files it was
//!   about, which it may have changed).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

pub use crate::claude_stream::mapper::UNFINISHED_TOOL;
use crate::codex_rpc::wire::items::{
    CommandExecutionItem, FileChangeItem, FileUpdateChange, ItemStatus, McpToolCallItem, ThreadItem,
};
use crate::model::AgentEvent;
use crate::pi_family::normalize::tool_result;
use crate::pi_family::touched::touched_paths;
use crate::security::redact_text;
use crate::signed_docs::unicode::spelled_out;

/// The card name of a command.
pub const SHELL: &str = "shell";

/// The card name of a file change.
pub const APPLY_PATCH: &str = "apply_patch";

/// The card name of a web search.
pub const WEB_SEARCH: &str = "web_search";

/// Result text of a tool whose `item/completed` was larger than the app reads.
pub const RESULT_TOO_LARGE: &str = "The result was larger than the app reads, so it is not shown.";

/// Result text of a command the user declined.
pub const COMMAND_DECLINED: &str = "The user declined this command.";

/// Result text of a file change the user declined.
pub const FILE_CHANGE_DECLINED: &str = "The user declined this change.";

/// Result text of a file change Codex could not apply.
const CHANGE_FAILED: &str = "The change could not be applied.";

/// Result text of an MCP call that failed without saying why.
const CALL_FAILED: &str = "The tool call failed.";

/// The most output a running command's card shows; past it the card waits for the result.
pub const LIVE_OUTPUT_BYTES: usize = 64 * 1024;

/// The cards of the running turn.
pub(super) struct Tools {
    project: PathBuf,
    calls: HashMap<String, Call>,
    /// Card ids in arrival order.
    order: Vec<String>,
    /// The files of each file change.
    changes: HashMap<String, Vec<FileUpdateChange>>,
}

struct Call {
    name: String,
    /// Output streamed so far (commands).
    output: String,
    /// Output still streams into the card.
    live: bool,
    ended: bool,
}

impl Tools {
    pub(super) fn new(project: PathBuf) -> Self {
        Self {
            project,
            calls: HashMap::new(),
            order: Vec::new(),
            changes: HashMap::new(),
        }
    }

    /// `item/started` of a tool item.
    pub(super) fn started(&mut self, item: &ThreadItem, out: &mut Vec<AgentEvent>) {
        let Some((name, args)) = card(item) else {
            return;
        };

        let id = item.id();

        if id.is_empty() || self.calls.contains_key(id) {
            return;
        }

        if let ThreadItem::FileChange(change) = item {
            self.changes.insert(id.to_owned(), change.changes.clone());
        }

        self.order.push(id.to_owned());
        self.calls.insert(
            id.to_owned(),
            Call {
                name: name.clone(),
                output: String::new(),
                live: true,
                ended: false,
            },
        );

        // A command's card names Codex's reading of it as a hint, above the command in full.
        let intent = match item {
            ThreadItem::CommandExecution(command) => command.reading().map(str::to_owned),
            _ => None,
        };

        out.push(AgentEvent::ToolStart {
            tool_call_id: id.to_owned(),
            name,
            args,
            intent,
        });
    }

    /// `item/commandExecution/outputDelta`.
    pub(super) fn output(&mut self, id: &str, delta: &str, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.get_mut(id).filter(|call| call.live && !call.ended) else {
            return;
        };

        if call.output.len() + delta.len() > LIVE_OUTPUT_BYTES {
            call.live = false;
            return;
        }

        call.output.push_str(delta);
        out.push(update(id, &call.output));
    }

    /// `item/mcpToolCall/progress`.
    pub(super) fn progress(&self, id: &str, message: &str, out: &mut Vec<AgentEvent>) {
        if self.calls.get(id).is_some_and(|call| !call.ended) && !message.trim().is_empty() {
            out.push(update(id, message));
        }
    }

    /// `item/fileChange/patchUpdated`.
    pub(super) fn patch_updated(&mut self, id: &str, changes: &[FileUpdateChange]) {
        if !id.is_empty() {
            self.changes.insert(id.to_owned(), changes.to_vec());
        }
    }

    /// The files of file change `id`, as known so far.
    pub(super) fn file_changes(&self, id: &str) -> &[FileUpdateChange] {
        self.changes.get(id).map(Vec::as_slice).unwrap_or_default()
    }

    /// `item/completed` of a tool item.
    pub(super) fn completed(&mut self, item: &ThreadItem, out: &mut Vec<AgentEvent>) {
        self.started(item, out);

        let id = item.id();
        let live = match self.calls.get_mut(id).filter(|call| !call.ended) {
            Some(call) => {
                call.ended = true;
                std::mem::take(&mut call.output)
            }
            None => return,
        };

        let Some((is_error, text, touched)) = self.result(item, live) else {
            return;
        };

        let name = self.calls.get(id).map(|call| call.name.clone()).unwrap_or_default();
        out.push(ended(id, name, is_error, &text, touched));
    }

    /// Ends running card `id`, whose `item/completed` could not be read, with [`RESULT_TOO_LARGE`].
    pub(super) fn lost(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.get_mut(id).filter(|call| !call.ended) else {
            return;
        };

        call.ended = true;
        let name = call.name.clone();
        let touched = if name == APPLY_PATCH {
            self.touched(self.file_changes(id))
        } else {
            Vec::new()
        };

        out.push(ended(id, name, true, RESULT_TOO_LARGE, touched));
    }

    /// Ends every card still running (its turn ended) with [`UNFINISHED_TOOL`].
    pub(super) fn end_unfinished(&mut self, out: &mut Vec<AgentEvent>) {
        for id in &self.order {
            let Some(call) = self.calls.get_mut(id).filter(|call| !call.ended) else {
                continue;
            };

            call.ended = true;
            out.push(ended(id, call.name.clone(), true, UNFINISHED_TOOL, Vec::new()));
        }
    }

    /// Forgets the turn's cards.
    pub(super) fn forget(&mut self) {
        self.calls.clear();
        self.order.clear();
        self.changes.clear();
    }

    /// Whether the finished tool `item` failed, its result text and the project files it touched
    /// (`live`: a command's output streamed so far); `None` for other items.
    fn result(&self, item: &ThreadItem, live: String) -> Option<(bool, String, Vec<String>)> {
        let result = match item {
            ThreadItem::CommandExecution(command) => command_result(command, live),
            ThreadItem::FileChange(change) => self.file_result(change),
            ThreadItem::McpToolCall(call) => mcp_result(call),
            ThreadItem::WebSearch(search) => (false, search.query.clone(), Vec::new()),
            _ => return None,
        };

        Some(result)
    }

    /// The result of a file change: its patches and the project files it touched.
    fn file_result(&self, change: &FileChangeItem) -> (bool, String, Vec<String>) {
        let changes = if change.changes.is_empty() {
            self.file_changes(&change.id)
        } else {
            &change.changes
        };

        match change.status {
            ItemStatus::Declined => (true, FILE_CHANGE_DECLINED.to_owned(), Vec::new()),
            ItemStatus::Failed => (true, CHANGE_FAILED.to_owned(), Vec::new()),
            _ => (false, patches(changes), self.touched(changes)),
        }
    }

    fn touched(&self, changes: &[FileUpdateChange]) -> Vec<String> {
        let mut touched: Vec<String> = Vec::new();

        for change in changes {
            let args = json!({ "path": change.path, "moveTo": change.kind.move_path });

            for path in touched_paths(&self.project, "edit", &args) {
                if !touched.contains(&path) {
                    touched.push(path);
                }
            }
        }

        touched
    }
}

/// A tool item of a saved thread (`thread/items/list`) as a finished card.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SavedCard {
    pub name: String,
    pub args: Value,
    pub is_error: bool,
    /// The result text, redacted.
    pub result: String,
    /// The project files a file change touched, project-relative.
    pub touched: Vec<String>,
}

/// The card of the saved tool `item` of a thread in `project`, as the live chat ends it; an item
/// that never finished (its turn was cut short) fails with [`UNFINISHED_TOOL`]. `None` for items
/// that are not tools.
pub(crate) fn saved_card(item: &ThreadItem, project: &Path) -> Option<SavedCard> {
    let (name, args) = card(item)?;

    let (is_error, text, touched) = if status(item) == Some(ItemStatus::InProgress) {
        (true, UNFINISHED_TOOL.to_owned(), Vec::new())
    } else {
        Tools::new(project.to_path_buf()).result(item, String::new())?
    };

    Some(SavedCard {
        name,
        args,
        is_error,
        result: redact_text(&text),
        touched,
    })
}

/// The status of a tool item that has one.
fn status(item: &ThreadItem) -> Option<ItemStatus> {
    match item {
        ThreadItem::CommandExecution(command) => Some(command.status),
        ThreadItem::FileChange(change) => Some(change.status),
        ThreadItem::McpToolCall(call) => Some(call.status),
        _ => None,
    }
}

/// The card name and args of a tool item; `None` for other items.
fn card(item: &ThreadItem) -> Option<(String, Value)> {
    let card = match item {
        // Hidden characters spelled out, as in the approval's title (`dialog_text.rs`).
        ThreadItem::CommandExecution(command) => (
            SHELL.to_owned(),
            json!({ "command": spelled_out(command.shown_command()), "cwd": command.cwd }),
        ),
        ThreadItem::FileChange(change) => (APPLY_PATCH.to_owned(), file_args(&change.changes)),
        ThreadItem::McpToolCall(call) => {
            let args = match &call.arguments {
                Value::Object(_) => call.arguments.clone(),
                _ => json!({}),
            };

            (format!("mcp__{}__{}", call.server, call.tool), args)
        }
        ThreadItem::WebSearch(search) => (WEB_SEARCH.to_owned(), json!({ "query": search.query })),
        _ => return None,
    };

    Some(card)
}

/// A file change's args: its first file as `path`, every file with its kind (and where a move
/// takes it, `moveTo`) as `changes`.
fn file_args(changes: &[FileUpdateChange]) -> Value {
    let listed: Vec<Value> = changes
        .iter()
        .map(
            |change| match change.kind.move_path.as_deref().filter(|to| !to.is_empty()) {
                Some(to) => json!({ "path": change.path, "kind": change.kind.kind, "moveTo": to }),
                None => json!({ "path": change.path, "kind": change.kind.kind }),
            },
        )
        .collect();

    match changes.first() {
        Some(first) => json!({ "path": first.path, "changes": listed }),
        None => json!({ "changes": listed }),
    }
}

/// Each file's kind and path (and where a move takes it), then its patch.
fn patches(changes: &[FileUpdateChange]) -> String {
    let parts: Vec<String> = changes
        .iter()
        .map(
            |change| match change.kind.move_path.as_deref().filter(|to| !to.is_empty()) {
                Some(to) => format!("{} {} (moved to {to})\n{}", change.kind.kind, change.path, change.diff),
                None => format!("{} {}\n{}", change.kind.kind, change.path, change.diff),
            },
        )
        .collect();

    parts.join("\n")
}

/// The result of a command: its output (`live` when Codex sent none), or why it did not run.
fn command_result(command: &CommandExecutionItem, live: String) -> (bool, String, Vec<String>) {
    let output = command
        .aggregated_output
        .clone()
        .filter(|output| !output.is_empty())
        .unwrap_or(live);

    let failed = match command.status {
        ItemStatus::Declined => return (true, COMMAND_DECLINED.to_owned(), Vec::new()),
        ItemStatus::Failed => true,
        _ => command.exit_code.is_some_and(|code| code != 0),
    };

    if failed && output.trim().is_empty() {
        let text = match command.exit_code {
            Some(code) => format!("The command failed with exit code {code}."),
            None => "The command failed.".to_owned(),
        };

        return (true, text, Vec::new());
    }

    (failed, output, Vec::new())
}

/// The result of an MCP call: its text content, or its error.
fn mcp_result(call: &McpToolCallItem) -> (bool, String, Vec<String>) {
    if let Some(error) = &call.error {
        let text = if error.message.trim().is_empty() {
            CALL_FAILED
        } else {
            &error.message
        };
        return (true, text.to_owned(), Vec::new());
    }

    let texts: Vec<&str> = call
        .result
        .iter()
        .flat_map(|result| &result.content)
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect();

    let failed = call.status == ItemStatus::Failed;
    let text = if failed && texts.is_empty() {
        CALL_FAILED.to_owned()
    } else {
        texts.join("\n")
    };

    (failed, text, Vec::new())
}

fn update(id: &str, text: &str) -> AgentEvent {
    AgentEvent::ToolUpdate {
        tool_call_id: id.to_owned(),
        partial: tool_result(Some(&Value::String(redact_text(text)))),
    }
}

fn ended(id: &str, name: String, is_error: bool, text: &str, touched_paths: Vec<String>) -> AgentEvent {
    AgentEvent::ToolEnd {
        tool_call_id: id.to_owned(),
        name,
        is_error,
        result: tool_result(Some(&Value::String(redact_text(text)))),
        touched_paths,
    }
}
