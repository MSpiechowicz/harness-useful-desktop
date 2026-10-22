//! The app's requests to Codex ([`Request`], written with [`Request::frame`] under an id of
//! [`next_request_id`]). Optional params left `None` are not sent, so Codex applies the user's
//! `config.toml`. The answers to Codex's own requests are in [`answers`](super::answers).

use std::sync::atomic::{AtomicI64, Ordering};

use serde::Serialize;
use serde_json::{Map, Value, json};

use super::RequestId;
use super::items::UserInput;
use super::notifications::FOLDED;

/// The next id for one of the app's requests: a number, unique within this app process.
pub fn next_request_id() -> RequestId {
    static NEXT: AtomicI64 = AtomicI64::new(1);

    RequestId::Number(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// A request of the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// The handshake; the `initialized` notification follows its result.
    Initialize(Initialize),
    ThreadStart(ThreadStart),
    ThreadResume(ThreadResume),
    ThreadFork(ThreadFork),
    ThreadNameSet {
        thread_id: String,
        name: String,
    },
    /// Runs as its own turn with a `contextCompaction` item.
    ThreadCompactStart {
        thread_id: String,
    },
    /// A `!command`: runs as its own turn, unsandboxed, its output lands in the thread.
    ThreadShellCommand {
        thread_id: String,
        command: String,
    },
    ThreadList(ThreadList),
    /// The thread's metadata (its turns are paged with `thread/turns/list` and `thread/items/list`).
    ThreadRead {
        thread_id: String,
    },
    ThreadTurnsList(Paged),
    ThreadItemsList(Paged),
    ThreadDelete {
        thread_id: String,
    },
    TurnStart(Box<TurnStart>),
    TurnSteer(TurnSteer),
    TurnInterrupt {
        thread_id: String,
        turn_id: String,
    },
    ModelList {
        include_hidden: bool,
    },
    SkillsList {
        cwds: Vec<String>,
    },
    /// Whether there is a login and which kind; never refreshes tokens.
    AccountRead,
    /// The MCP servers with their tools and auth state (not their resources).
    McpServerStatusList {
        thread_id: Option<String>,
    },
}

impl Request {
    pub fn method(&self) -> &'static str {
        match self {
            Self::Initialize(_) => "initialize",
            Self::ThreadStart(_) => "thread/start",
            Self::ThreadResume(_) => "thread/resume",
            Self::ThreadFork(_) => "thread/fork",
            Self::ThreadNameSet { .. } => "thread/name/set",
            Self::ThreadCompactStart { .. } => "thread/compact/start",
            Self::ThreadShellCommand { .. } => "thread/shellCommand",
            Self::ThreadList(_) => "thread/list",
            Self::ThreadRead { .. } => "thread/read",
            Self::ThreadTurnsList(_) => "thread/turns/list",
            Self::ThreadItemsList(_) => "thread/items/list",
            Self::ThreadDelete { .. } => "thread/delete",
            Self::TurnStart(_) => "turn/start",
            Self::TurnSteer(_) => "turn/steer",
            Self::TurnInterrupt { .. } => "turn/interrupt",
            Self::ModelList { .. } => "model/list",
            Self::SkillsList { .. } => "skills/list",
            Self::AccountRead => "account/read",
            Self::McpServerStatusList { .. } => "mcpServerStatus/list",
        }
    }

    pub fn params(&self) -> Value {
        match self {
            Self::Initialize(params) => to_value(params),
            Self::ThreadStart(params) => to_value(params),
            Self::ThreadResume(params) => to_value(params),
            Self::ThreadFork(params) => to_value(params),
            Self::ThreadNameSet { thread_id, name } => json!({ "threadId": thread_id, "name": name }),
            Self::ThreadCompactStart { thread_id } | Self::ThreadDelete { thread_id } => {
                json!({ "threadId": thread_id })
            }
            Self::ThreadShellCommand { thread_id, command } => json!({ "threadId": thread_id, "command": command }),
            Self::ThreadList(params) => to_value(params),
            Self::ThreadRead { thread_id } => json!({ "threadId": thread_id, "includeTurns": false }),
            Self::ThreadTurnsList(params) | Self::ThreadItemsList(params) => to_value(params),
            Self::TurnStart(params) => to_value(params),
            Self::TurnSteer(params) => to_value(params),
            Self::TurnInterrupt { thread_id, turn_id } => json!({ "threadId": thread_id, "turnId": turn_id }),
            Self::ModelList { include_hidden } => json!({ "includeHidden": include_hidden }),
            Self::SkillsList { cwds } => json!({ "cwds": cwds }),
            Self::AccountRead => json!({ "refreshToken": false }),
            Self::McpServerStatusList { thread_id } => {
                let mut params = json!({ "detail": "toolsAndAuthOnly" });
                if let Some(thread_id) = thread_id {
                    params["threadId"] = json!(thread_id);
                }
                params
            }
        }
    }

    /// The request frame under `id`.
    pub fn frame(&self, id: &RequestId) -> Value {
        json!({ "id": id, "method": self.method(), "params": self.params() })
    }
}

fn to_value(params: &impl Serialize) -> Value {
    serde_json::to_value(params).unwrap_or_else(|_| Value::Object(Map::new()))
}

/// `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Initialize {
    pub client_info: ClientInfo,
    pub capabilities: Capabilities,
}

impl Initialize {
    /// The desktop app at `version`: experimental API on (question dialogs), the folded noise
    /// opted out of.
    pub fn desktop(version: &str) -> Self {
        Self {
            client_info: ClientInfo {
                name: "harness_desktop".to_owned(),
                title: "Harness Desktop".to_owned(),
                version: version.to_owned(),
            },
            capabilities: Capabilities {
                experimental_api: true,
                request_attestation: false,
                opt_out_notification_methods: FOLDED.iter().map(|method| (*method).to_owned()).collect(),
            },
        }
    }
}

/// Who the app is; Codex names it in its user agent and in the threads it starts (`originator`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

/// What the app takes part in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub experimental_api: bool,
    pub request_attestation: bool,
    /// Notifications Codex leaves out for this connection.
    pub opt_out_notification_methods: Vec<String>,
}

/// When Codex asks before it acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    /// Before anything that is not known to be safe.
    Untrusted,
    /// When the model asks to leave the sandbox.
    OnRequest,
    Never,
}

/// What the sandbox allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// Settings of a thread being started, resumed or forked; `None` keeps the configured one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    /// `-c`-style config overrides for this thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Map<String, Value>>,
}

/// `thread/start`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStart {
    #[serde(flatten)]
    pub settings: ThreadSettings,
    /// Not saved, not listed (AI naming).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
}

/// `thread/resume`: without its turns (they are paged).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResume {
    pub thread_id: String,
    #[serde(flatten)]
    pub settings: ThreadSettings,
    pub exclude_turns: bool,
}

/// `thread/fork`: a fork keeps the turns up to and including `last_turn_id`, a clone (`None`)
/// all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadFork {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_turn_id: Option<String>,
    #[serde(flatten)]
    pub settings: ThreadSettings,
    pub exclude_turns: bool,
}

/// `thread/list`: newest first, archived threads left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadList {
    /// Only threads started in this folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// `created_at`, `updated_at`, `recency_at`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_key: Option<String>,
    /// A part of the thread's title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
    /// Where the threads came from (`cli`, `vscode`, `exec`, `appServer`, `subAgent…`,
    /// `unknown`); Codex lists only interactive ones without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kinds: Option<Vec<String>>,
    /// `Some(true)` lists only archived threads; otherwise only the others.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
}

/// `thread/turns/list` (newest first unless `asc`) or `thread/items/list` (oldest first unless
/// `desc`, of one turn when `turn_id` is set).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paged {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// `asc` or `desc`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_direction: Option<String>,
    /// How much of each turn's items `thread/turns/list` includes: `notLoaded`, `summary` (the
    /// default) or `full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_view: Option<String>,
}

/// `turn/start`: a prompt as a new turn; the overrides hold for the turns that follow too.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStart {
    pub thread_id: String,
    /// Comes back as the `clientId` of the turn's `userMessage` item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_user_message_id: Option<String>,
    pub input: Vec<UserInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The reasoning effort (`low`, `medium`, `high`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// The reasoning summary (`auto`, `concise`, `detailed`, `none`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalPolicy>,
    /// A sandbox policy object (`{"type": "workspaceWrite", …}`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<Value>,
}

/// `turn/steer`: input into the running turn `expected_turn_id` (fails when no turn runs).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteer {
    pub thread_id: String,
    pub expected_turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_user_message_id: Option<String>,
    pub input: Vec<UserInput>,
}
