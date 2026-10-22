//! `system` frames by subtype. A subtype this app does not know is [`System::Unknown`].

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::control::SlashCommand;
use super::nullable;

/// A `system` frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemFrame {
    pub session_id: Option<String>,
    pub uuid: Option<String>,
    pub kind: System,
}

/// What a `system` frame reports.
#[derive(Debug, Clone, PartialEq)]
pub enum System {
    /// The start of every turn (also of a quiet run).
    Init(Box<Init>),
    /// `requesting` a model call, `compacting`, a compaction's outcome or a permission mode change.
    Status(Status),
    /// A compaction replaced the history.
    CompactBoundary(CompactBoundary),
    TaskStarted(TaskStarted),
    TaskProgress(TaskProgress),
    TaskUpdated(TaskUpdated),
    TaskNotification(TaskNotification),
    /// The session was renamed (`rename_session`).
    SessionTitleChanged {
        title: String,
    },
    /// The slash-command catalog changed (`reload_plugins`, a skill added).
    CommandsChanged {
        commands: Vec<SlashCommand>,
    },
    /// A hook started, made progress or ended.
    Hook(Hook),
    /// The running estimate of withheld thinking.
    ThinkingTokens {
        estimated_tokens: u64,
    },
    /// A model call failed and will be tried again.
    ApiRetry(ApiRetry),
    /// `idle`, `running` or `requires_action`.
    SessionStateChanged {
        state: String,
    },
    /// Output of a local slash command.
    LocalCommandOutput {
        content: String,
    },
    /// A notification for the user.
    Notification(Notification),
    /// A tool call was denied without asking.
    PermissionDenied {
        tool_name: String,
        tool_use_id: String,
    },
    /// The background task list changed.
    BackgroundTasksChanged,
    /// A subtype this app does not know.
    Unknown {
        subtype: String,
    },
}

/// `system/init`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Init {
    #[serde(deserialize_with = "nullable")]
    pub model: String,
    #[serde(deserialize_with = "nullable")]
    pub cwd: String,
    #[serde(deserialize_with = "nullable")]
    pub tools: Vec<String>,
    #[serde(rename = "permissionMode")]
    pub permission_mode: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub slash_commands: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub skills: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub agents: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub mcp_servers: Vec<McpServerState>,
    /// Which kind of login is in use (`none` for a claude.ai subscription); no credential.
    #[serde(rename = "apiKeySource")]
    pub api_key_source: Option<String>,
    pub claude_code_version: Option<String>,
    pub output_style: Option<String>,
    /// Feature flags (`interrupt_receipt_v1`, …).
    #[serde(deserialize_with = "nullable")]
    pub capabilities: Vec<String>,
    /// The effort level (`low` … `max`).
    pub effort: Option<String>,
}

/// An MCP server of `system/init`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct McpServerState {
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub status: String,
    pub source: Option<String>,
}

/// `system/status`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Status {
    /// `requesting`, `compacting` or none.
    pub status: Option<String>,
    #[serde(rename = "permissionMode")]
    pub permission_mode: Option<String>,
    /// `success` or `failed` once a compaction ended.
    pub compact_result: Option<String>,
    pub compact_error: Option<String>,
    /// The prompts the starting request serves.
    #[serde(deserialize_with = "nullable")]
    pub user_message_uuids: Vec<String>,
}

/// `system/compact_boundary`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct CompactBoundary {
    #[serde(deserialize_with = "nullable")]
    pub compact_metadata: CompactMetadata,
}

/// What a compaction did.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct CompactMetadata {
    /// `manual` or `auto`.
    pub trigger: Option<String>,
    pub pre_tokens: Option<u64>,
    pub post_tokens: Option<u64>,
}

/// `system/task_started`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskStarted {
    #[serde(deserialize_with = "nullable")]
    pub task_id: String,
    pub tool_use_id: Option<String>,
    pub description: Option<String>,
    /// The agent type of a `Task` subagent.
    pub subagent_type: Option<String>,
    /// `local_agent` for a subagent, `local_bash` for a shell, …
    pub task_type: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub is_backgrounded: bool,
    pub prompt: Option<String>,
}

/// `system/task_progress`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskProgress {
    #[serde(deserialize_with = "nullable")]
    pub task_id: String,
    pub tool_use_id: Option<String>,
    pub description: Option<String>,
    pub subagent_type: Option<String>,
    pub usage: Option<TaskUsage>,
    pub last_tool_name: Option<String>,
    pub summary: Option<String>,
}

/// What a task used so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskUsage {
    #[serde(deserialize_with = "nullable")]
    pub total_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    pub tool_uses: u64,
    #[serde(deserialize_with = "nullable")]
    pub duration_ms: u64,
}

/// `system/task_updated`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskUpdated {
    #[serde(deserialize_with = "nullable")]
    pub task_id: String,
    #[serde(deserialize_with = "nullable")]
    pub patch: TaskPatch,
}

/// The fields of a task that changed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskPatch {
    /// `pending`, `running`, `completed`, `failed`, `killed` or `paused`.
    pub status: Option<String>,
    pub description: Option<String>,
    pub error: Option<String>,
    pub is_backgrounded: Option<bool>,
}

/// `system/task_notification`: a task ended.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskNotification {
    #[serde(deserialize_with = "nullable")]
    pub task_id: String,
    pub tool_use_id: Option<String>,
    /// `completed`, `failed` or `stopped`.
    pub status: Option<String>,
    pub summary: Option<String>,
    pub usage: Option<TaskUsage>,
}

/// `system/hook_started`, `hook_progress` and `hook_response` (outputs are not kept).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Hook {
    pub hook_id: Option<String>,
    pub hook_name: Option<String>,
    pub hook_event: Option<String>,
    /// `success`, `error`, … on `hook_response`.
    pub outcome: Option<String>,
    pub exit_code: Option<i64>,
}

/// `system/api_retry`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ApiRetry {
    pub attempt: Option<u32>,
    pub max_retries: Option<u32>,
    pub retry_delay_ms: Option<u64>,
    pub error_status: Option<u16>,
    /// The error kind (`rate_limit`, `overloaded`, `server_error`, …).
    pub error: Option<String>,
}

/// `system/notification`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub key: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub text: String,
    /// `low`, `medium`, `high` or `immediate`.
    pub priority: Option<String>,
}

/// Fields every subtype payload is read with.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Head {
    subtype: Option<String>,
    session_id: Option<String>,
    uuid: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Title {
    #[serde(deserialize_with = "nullable")]
    title: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Commands {
    #[serde(deserialize_with = "nullable")]
    commands: Vec<SlashCommand>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Fields {
    #[serde(deserialize_with = "nullable")]
    estimated_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    state: String,
    #[serde(deserialize_with = "nullable")]
    content: String,
    #[serde(deserialize_with = "nullable")]
    tool_name: String,
    #[serde(deserialize_with = "nullable")]
    tool_use_id: String,
}

/// Reads a `system` frame; the error is the subtype whose shape did not fit.
pub(super) fn decode(frame: Value) -> Result<SystemFrame, Option<String>> {
    let head: Head = serde_json::from_value(frame.clone()).map_err(|_| None::<String>)?;
    let subtype = head.subtype.unwrap_or_default();

    let kind = match subtype.as_str() {
        "init" => System::Init(Box::new(read(frame, &subtype)?)),
        "status" => System::Status(read(frame, &subtype)?),
        "compact_boundary" => System::CompactBoundary(read(frame, &subtype)?),
        "task_started" => System::TaskStarted(read(frame, &subtype)?),
        "task_progress" => System::TaskProgress(read(frame, &subtype)?),
        "task_updated" => System::TaskUpdated(read(frame, &subtype)?),
        "task_notification" => System::TaskNotification(read(frame, &subtype)?),
        "session_title_changed" => System::SessionTitleChanged {
            title: read::<Title>(frame, &subtype)?.title,
        },
        "commands_changed" => System::CommandsChanged {
            commands: read::<Commands>(frame, &subtype)?.commands,
        },
        "hook_started" | "hook_progress" | "hook_response" => System::Hook(read(frame, &subtype)?),
        "api_retry" => System::ApiRetry(read(frame, &subtype)?),
        "notification" => System::Notification(read(frame, &subtype)?),
        "background_tasks_changed" => System::BackgroundTasksChanged,
        "thinking_tokens" | "session_state_changed" | "local_command_output" | "permission_denied" => {
            fields(&subtype, read(frame, &subtype)?)
        }
        _ => System::Unknown { subtype },
    };

    Ok(SystemFrame {
        session_id: head.session_id,
        uuid: head.uuid,
        kind,
    })
}

/// `frame` as `T`; the error names `subtype`.
fn read<T: DeserializeOwned>(frame: Value, subtype: &str) -> Result<T, Option<String>> {
    serde_json::from_value(frame).map_err(|_| Some(subtype.to_owned()))
}

/// The subtypes read from a few plain fields.
fn fields(subtype: &str, fields: Fields) -> System {
    match subtype {
        "thinking_tokens" => System::ThinkingTokens {
            estimated_tokens: fields.estimated_tokens,
        },
        "session_state_changed" => System::SessionStateChanged { state: fields.state },
        "local_command_output" => System::LocalCommandOutput {
            content: fields.content,
        },
        _ => System::PermissionDenied {
            tool_name: fields.tool_name,
            tool_use_id: fields.tool_use_id,
        },
    }
}
