//! Thread items (`ThreadItem`), the turns that hold them and the user input the app sends.
//!
//! An item type this app does not know is [`ThreadItem::Unknown`] (with its type and id); a known
//! one reads every field leniently (`null` and absent fields default).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::claude_stream::wire::{lenient_option, nullable};

/// One item of a thread.
#[derive(Debug, Clone, PartialEq)]
pub enum ThreadItem {
    /// What the user sent (a prompt, a steer).
    UserMessage(UserMessageItem),
    /// The model's text: commentary between tool calls or the final answer.
    AgentMessage(AgentMessageItem),
    /// The model's reasoning, often empty (no summary was asked for).
    Reasoning(ReasoningItem),
    /// A shell command (the model's, or the user's `thread/shellCommand`).
    CommandExecution(Box<CommandExecutionItem>),
    /// A patch to files.
    FileChange(FileChangeItem),
    /// A call of an MCP server's tool.
    McpToolCall(Box<McpToolCallItem>),
    /// A web search.
    WebSearch(WebSearchItem),
    /// A compaction of the thread's context.
    ContextCompaction { id: String },
    /// An item type this app does not show (plans, subagents, images, reviews, …).
    Unknown { kind: String, id: String },
}

impl ThreadItem {
    /// The item's id.
    pub fn id(&self) -> &str {
        match self {
            Self::UserMessage(item) => &item.id,
            Self::AgentMessage(item) => &item.id,
            Self::Reasoning(item) => &item.id,
            Self::CommandExecution(item) => &item.id,
            Self::FileChange(item) => &item.id,
            Self::McpToolCall(item) => &item.id,
            Self::WebSearch(item) => &item.id,
            Self::ContextCompaction { id } | Self::Unknown { id, .. } => id,
        }
    }
}

impl<'de> Deserialize<'de> for ThreadItem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let kind = text("type");

        let item = match kind.as_str() {
            "userMessage" => Self::UserMessage(typed(value)?),
            "agentMessage" => Self::AgentMessage(typed(value)?),
            "reasoning" => Self::Reasoning(typed(value)?),
            "commandExecution" => Self::CommandExecution(Box::new(typed(value)?)),
            "fileChange" => Self::FileChange(typed(value)?),
            "mcpToolCall" => Self::McpToolCall(Box::new(typed(value)?)),
            "webSearch" => Self::WebSearch(typed(value)?),
            "contextCompaction" => Self::ContextCompaction { id: text("id") },
            _ => Self::Unknown { id: text("id"), kind },
        };

        Ok(item)
    }
}

fn typed<T: DeserializeOwned, E: serde::de::Error>(value: Value) -> Result<T, E> {
    serde_json::from_value(value).map_err(E::custom)
}

/// What the user sent.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserMessageItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The `clientUserMessageId` the app gave `turn/start` or `turn/steer`.
    pub client_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub content: Vec<UserInput>,
}

impl UserMessageItem {
    /// The text parts, one per line.
    pub fn text(&self) -> String {
        let parts: Vec<&str> = self
            .content
            .iter()
            .filter_map(|input| match input {
                UserInput::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        parts.join("\n")
    }
}

/// A part of what the user sends, or sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserInput {
    Text {
        #[serde(default, deserialize_with = "nullable")]
        text: String,
        /// Spans of special elements in `text` (the app sends none).
        #[serde(default, deserialize_with = "nullable")]
        text_elements: Vec<Value>,
    },
    /// An image by URL (`data:` URLs too).
    Image {
        #[serde(default, deserialize_with = "nullable")]
        url: String,
    },
    /// An image file Codex reads itself.
    LocalImage {
        #[serde(default, deserialize_with = "nullable")]
        path: String,
    },
    /// A skill to use (`skills/list` names its path).
    Skill {
        #[serde(default, deserialize_with = "nullable")]
        name: String,
        #[serde(default, deserialize_with = "nullable")]
        path: String,
    },
    /// A file or folder the user mentions.
    Mention {
        #[serde(default, deserialize_with = "nullable")]
        name: String,
        #[serde(default, deserialize_with = "nullable")]
        path: String,
    },
    /// An input type this app does not know.
    #[serde(other)]
    Other,
}

impl UserInput {
    /// Plain text.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }
    }
}

/// The model's text.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentMessageItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The full text (empty while it streams).
    #[serde(deserialize_with = "nullable")]
    pub text: String,
    /// `None` when the model does not say.
    pub phase: Option<MessagePhase>,
}

/// Whether an agent message is interim commentary or the final answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagePhase {
    Commentary,
    FinalAnswer,
    #[serde(other)]
    Other,
}

impl MessagePhase {
    /// The phase as Codex names it.
    pub fn wire(self) -> &'static str {
        match self {
            Self::Commentary => "commentary",
            Self::FinalAnswer => "final_answer",
            Self::Other => "other",
        }
    }
}

/// The model's reasoning: summary paragraphs and raw reasoning text, either may be empty.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ReasoningItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub summary: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub content: Vec<String>,
}

/// Where an item is (commands, file changes, MCP calls).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
    /// The user declined its approval.
    Declined,
    #[serde(other)]
    Other,
}

/// A shell command.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandExecutionItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The command line as run (the user's shell with `-lc '…'`).
    #[serde(deserialize_with = "nullable")]
    pub command: String,
    #[serde(deserialize_with = "nullable")]
    pub cwd: String,
    /// `agent`, `userShell` (`thread/shellCommand`), `unifiedExecStartup`, ….
    pub source: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub status: ItemStatus,
    /// Codex's own reading of the command line.
    #[serde(deserialize_with = "nullable")]
    pub command_actions: Vec<CommandAction>,
    /// Its output, stdout and stderr together.
    pub aggregated_output: Option<String>,
    pub exit_code: Option<i64>,
    #[serde(deserialize_with = "lenient_option")]
    pub duration_ms: Option<u64>,
}

impl CommandExecutionItem {
    /// The command to show: what runs, in full (see [`shown_command`]).
    pub fn shown_command(&self) -> &str {
        shown_command(&self.command)
    }

    /// Codex's reading of the command, a hint next to it (see [`command_reading`]).
    pub fn reading(&self) -> Option<&str> {
        command_reading(&self.command, &self.command_actions)
    }
}

/// The command to show for `command`, everything that runs: the script of a plain
/// `<shell> -lc '<script>'` (or `-c`) wrapper of `bash`, `sh` or `zsh` whose script holds no `'`
/// (so it runs exactly as written), else the line as run, unchanged. Never Codex's reading of it
/// (`commandActions`), which leaves parts of a pipeline or a sequence out.
pub fn shown_command(command: &str) -> &str {
    unwrapped_script(command).unwrap_or(command)
}

/// The script of a plain shell wrapper (see [`shown_command`]).
fn unwrapped_script(command: &str) -> Option<&str> {
    let (program, rest) = command.split_once(' ')?;
    let shell = program.rsplit('/').next().unwrap_or(program);

    if !matches!(shell, "bash" | "sh" | "zsh") {
        return None;
    }

    let quoted = rest.strip_prefix("-lc ").or_else(|| rest.strip_prefix("-c "))?;
    let script = quoted.strip_prefix('\'')?.strip_suffix('\'')?;

    (!script.contains('\'') && !script.trim().is_empty()).then_some(script)
}

/// Codex's reading of `command` (`actions`) when it found one command that is not what
/// [`shown_command`] shows: a hint only, as it can leave parts out.
pub fn command_reading<'a>(command: &str, actions: &'a [CommandAction]) -> Option<&'a str> {
    match actions {
        [single] if !single.command.trim().is_empty() && single.command != shown_command(command) => {
            Some(&single.command)
        }
        _ => None,
    }
}

/// What a command does, as Codex reads it (`read`, `listFiles`, `search`, `unknown`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct CommandAction {
    #[serde(rename = "type", deserialize_with = "nullable")]
    pub kind: String,
    /// The part of the command line it is.
    #[serde(deserialize_with = "nullable")]
    pub command: String,
    pub path: Option<String>,
}

/// A patch to files.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct FileChangeItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub changes: Vec<FileUpdateChange>,
    #[serde(deserialize_with = "nullable")]
    pub status: ItemStatus,
}

/// One file of a patch.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct FileUpdateChange {
    /// Absolute.
    #[serde(deserialize_with = "nullable")]
    pub path: String,
    #[serde(deserialize_with = "nullable")]
    pub kind: PatchChangeKind,
    #[serde(deserialize_with = "nullable")]
    pub diff: String,
}

/// `add`, `delete` or `update` (with the path a move goes to).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct PatchChangeKind {
    #[serde(rename = "type", deserialize_with = "nullable")]
    pub kind: String,
    pub move_path: Option<String>,
}

/// A call of an MCP server's tool.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpToolCallItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub server: String,
    #[serde(deserialize_with = "nullable")]
    pub tool: String,
    #[serde(deserialize_with = "nullable")]
    pub status: ItemStatus,
    pub arguments: Value,
    pub result: Option<McpToolResult>,
    pub error: Option<McpToolError>,
}

/// What an MCP tool returned: MCP content blocks.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct McpToolResult {
    #[serde(deserialize_with = "nullable")]
    pub content: Vec<Value>,
}

/// Why an MCP tool call failed.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct McpToolError {
    #[serde(deserialize_with = "nullable")]
    pub message: String,
}

/// A web search.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct WebSearchItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub query: String,
}

/// A turn: a prompt (or a `!command`, a compaction) and everything the model did for it.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Turn {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// Its items, as far as the request asked for them (`itemsView`).
    #[serde(deserialize_with = "nullable")]
    pub items: Vec<ThreadItem>,
    #[serde(deserialize_with = "nullable")]
    pub status: TurnStatus,
    /// Set when it failed.
    pub error: Option<TurnError>,
    /// Seconds since the epoch.
    #[serde(deserialize_with = "lenient_option")]
    pub started_at: Option<u64>,
    #[serde(deserialize_with = "lenient_option")]
    pub completed_at: Option<u64>,
    #[serde(deserialize_with = "lenient_option")]
    pub duration_ms: Option<u64>,
}

/// How a turn is or ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnStatus {
    #[default]
    InProgress,
    Completed,
    Interrupted,
    Failed,
    #[serde(other)]
    Other,
}

/// Why a turn failed (also in the `error` notification).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TurnError {
    #[serde(deserialize_with = "nullable")]
    pub message: String,
    /// The error kind: a string (`unauthorized`, `usageLimitExceeded`, …) or an object keyed by
    /// it (`{"httpConnectionFailed": {…}}`).
    pub codex_error_info: Option<Value>,
}

impl TurnError {
    /// The error kind (see [`codex_error_info`](Self::codex_error_info)).
    pub fn kind(&self) -> Option<&str> {
        match self.codex_error_info.as_ref()? {
            Value::String(kind) => Some(kind),
            Value::Object(fields) => fields.keys().next().map(String::as_str),
            _ => None,
        }
    }
}
