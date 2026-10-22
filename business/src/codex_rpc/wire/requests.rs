//! What Codex asks the app (`ServerRequest`), by `method`, each answered under its id with an
//! encoder of [`outbound`](super::outbound): approvals of a command, a file change or extra
//! permissions, questions of the model (`request_user_input`, Plan mode only), an MCP server's
//! elicitation, and requests the app always refuses (dynamic tool calls, ChatGPT token refreshes).
//!
//! A request whose params do not fit, or of a method this app does not know (`attestation/
//! generate`, the legacy `applyPatchApproval` / `execCommandApproval`), is
//! [`ServerRequestKind::Unknown`] and answered with an error.

use serde::Deserialize;
use serde_json::Value;

use super::items::{CommandAction, shown_command};
use super::{RequestId, params};
use crate::claude_stream::wire::nullable;

/// One request of Codex.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerRequest {
    /// Answer under this id.
    pub id: RequestId,
    pub method: String,
    pub kind: ServerRequestKind,
}

/// What a request asks.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerRequestKind {
    /// May a command run? `item/commandExecution/requestApproval`.
    CommandApproval(Box<CommandApproval>),
    /// May a patch be applied? `item/fileChange/requestApproval` (its files are in the item).
    FileChangeApproval(FileChangeApproval),
    /// May the turn (or session) get extra permissions? `item/permissions/requestApproval`.
    PermissionsApproval(Box<PermissionsApproval>),
    /// The model asks the user questions: `item/tool/requestUserInput`.
    UserInput(UserInputRequest),
    /// An MCP server asks for input or to visit a URL: `mcpServer/elicitation/request`.
    Elicitation(Box<Elicitation>),
    /// A dynamic tool the app never registers: `item/tool/call`.
    DynamicToolCall,
    /// ChatGPT tokens the app never holds: `account/chatgptAuthTokens/refresh`.
    AuthTokensRefresh,
    /// A method this app does not handle, or params that do not fit.
    Unknown,
}

/// Reads request `method` with `params` under `id`.
pub(super) fn decode(id: RequestId, method: &str, raw: Value) -> ServerRequest {
    let kind = match method {
        "item/commandExecution/requestApproval" => {
            params(raw).map(|params| ServerRequestKind::CommandApproval(Box::new(params)))
        }
        "item/fileChange/requestApproval" => params(raw).map(ServerRequestKind::FileChangeApproval),
        "item/permissions/requestApproval" => {
            params(raw).map(|params| ServerRequestKind::PermissionsApproval(Box::new(params)))
        }
        "item/tool/requestUserInput" => params(raw).map(ServerRequestKind::UserInput),
        "mcpServer/elicitation/request" => params(raw).map(|params| ServerRequestKind::Elicitation(Box::new(params))),
        "item/tool/call" => Ok(ServerRequestKind::DynamicToolCall),
        "account/chatgptAuthTokens/refresh" => Ok(ServerRequestKind::AuthTokensRefresh),
        _ => Ok(ServerRequestKind::Unknown),
    };

    ServerRequest {
        id,
        method: method.to_owned(),
        kind: kind.unwrap_or(ServerRequestKind::Unknown),
    }
}

/// `item/commandExecution/requestApproval`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandApproval {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
    /// The `commandExecution` item it is about.
    #[serde(deserialize_with = "nullable")]
    pub item_id: String,
    /// `command` (or a newer kind, such as a stdin approval).
    pub kind: Option<String>,
    /// Why Codex asks (network access, …).
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub command_actions: Vec<CommandAction>,
    /// The rule prefix an "always allow" answer adds to the user's rules.
    pub proposed_execpolicy_amendment: Option<Vec<String>>,
    /// The host a network approval is about.
    pub network_approval_context: Option<NetworkContext>,
    /// The answers Codex accepts (`"accept"`, `{"acceptWithExecpolicyAmendment": …}`, …);
    /// empty when it does not say.
    #[serde(deserialize_with = "nullable")]
    pub available_decisions: Vec<Value>,
}

impl CommandApproval {
    /// The command to show: Codex's reading of it when it found one command, else the line as run.
    pub fn shown_command(&self) -> Option<&str> {
        let command = self.command.as_deref()?;

        Some(shown_command(command, &self.command_actions))
    }

    /// True when Codex accepts the answer named `decision` (`accept`, `acceptForSession`,
    /// `acceptWithExecpolicyAmendment`, `decline`, `cancel`); every answer when it does not say.
    pub fn offers(&self, decision: &str) -> bool {
        self.available_decisions.is_empty()
            || self.available_decisions.iter().any(|offered| match offered {
                Value::String(name) => name == decision,
                Value::Object(fields) => fields.contains_key(decision),
                _ => false,
            })
    }
}

/// The host and protocol of a network approval.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct NetworkContext {
    #[serde(deserialize_with = "nullable")]
    pub host: String,
    #[serde(deserialize_with = "nullable")]
    pub protocol: String,
}

/// `item/fileChange/requestApproval`: the files are those of the `fileChange` item `item_id`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileChangeApproval {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
    #[serde(deserialize_with = "nullable")]
    pub item_id: String,
    pub reason: Option<String>,
    /// A folder the agent asks to write below for the rest of the session.
    pub grant_root: Option<String>,
}

/// `item/permissions/requestApproval`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionsApproval {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
    #[serde(deserialize_with = "nullable")]
    pub item_id: String,
    #[serde(deserialize_with = "nullable")]
    pub cwd: String,
    pub reason: Option<String>,
    /// The requested profile (`network`, `fileSystem`), echoed by a grant.
    pub permissions: Value,
}

impl PermissionsApproval {
    /// True when network access is asked for.
    pub fn network(&self) -> bool {
        self.permissions.pointer("/network/enabled") == Some(&Value::Bool(true))
    }

    /// The paths asked to be readable.
    pub fn read_paths(&self) -> Vec<String> {
        self.paths("read")
    }

    /// The paths asked to be writable.
    pub fn write_paths(&self) -> Vec<String> {
        self.paths("write")
    }

    fn paths(&self, access: &str) -> Vec<String> {
        let listed = self.permissions.pointer(&format!("/fileSystem/{access}"));

        listed
            .and_then(Value::as_array)
            .map(|paths| paths.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default()
    }
}

/// `item/tool/requestUserInput`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserInputRequest {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
    #[serde(deserialize_with = "nullable")]
    pub item_id: String,
    #[serde(deserialize_with = "nullable")]
    pub questions: Vec<UserQuestion>,
    /// True when the turn waits for the answers.
    pub is_blocking: bool,
}

/// One question; the answers are keyed by its `id`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserQuestion {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// A short label.
    #[serde(deserialize_with = "nullable")]
    pub header: String,
    #[serde(deserialize_with = "nullable")]
    pub question: String,
    /// True when a free-text answer is allowed besides the options.
    pub is_other: bool,
    /// True when the answer is a secret (never shown or kept).
    pub is_secret: bool,
    #[serde(deserialize_with = "nullable")]
    pub options: Vec<UserOption>,
}

/// An option of a question.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct UserOption {
    #[serde(deserialize_with = "nullable")]
    pub label: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
}

/// `mcpServer/elicitation/request`: a form (`form`, `openai/form`, `openaiForm`) or a URL.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Elicitation {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    pub turn_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub server_name: String,
    #[serde(deserialize_with = "nullable")]
    pub mode: String,
    #[serde(deserialize_with = "nullable")]
    pub message: String,
    /// The form's JSON schema.
    pub requested_schema: Option<Value>,
    /// The URL to visit.
    pub url: Option<String>,
    pub elicitation_id: Option<String>,
}

impl Elicitation {
    /// True when it asks to visit a URL rather than to fill a form.
    pub fn is_url(&self) -> bool {
        self.mode == "url"
    }
}
