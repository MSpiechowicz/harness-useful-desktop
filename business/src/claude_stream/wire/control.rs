//! The control protocol as the CLI speaks it: the requests it makes of the app
//! ([`ControlRequestFrame`]: permission prompts, hook callbacks, dialogs), the replies to the app's
//! own requests ([`ControlResponse`]) and the payloads of those replies.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::nullable;

/// A `control_request` from the CLI.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ControlRequestFrame {
    /// The CLI's id (a UUID), echoed in the app's `control_response`.
    #[serde(default, deserialize_with = "nullable")]
    pub request_id: String,
    pub request: CliRequest,
}

/// What the CLI asks.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "Value")]
pub enum CliRequest {
    /// May a tool run? (also `AskUserQuestion`, answered with the answers).
    CanUseTool(Box<PermissionRequest>),
    /// One of the app's hook callbacks (registered with `initialize`) fired.
    HookCallback(HookCallback),
    /// A dialog of a kind the app declared in `initialize`.
    RequestUserDialog(UserDialog),
    /// An MCP server asks the user for input.
    Elicitation(Elicitation),
    /// A request this app does not handle (`mcp_message`, …); answer it with an error.
    Unknown { subtype: String },
}

impl From<Value> for CliRequest {
    fn from(request: Value) -> Self {
        let subtype = request
            .get("subtype")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();

        let read = || -> Option<Self> {
            Some(match subtype.as_str() {
                "can_use_tool" => Self::CanUseTool(Box::new(parse(&request)?)),
                "hook_callback" => Self::HookCallback(parse(&request)?),
                "request_user_dialog" => Self::RequestUserDialog(parse(&request)?),
                "elicitation" => Self::Elicitation(parse(&request)?),
                _ => return None,
            })
        };

        read().unwrap_or_else(|| Self::Unknown {
            subtype: subtype.clone(),
        })
    }
}

/// `can_use_tool`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct PermissionRequest {
    #[serde(deserialize_with = "nullable")]
    pub tool_name: String,
    pub display_name: Option<String>,
    pub title: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub input: Value,
    /// A short description of the call (a file name, a command's description).
    pub description: Option<String>,
    /// Why the CLI asks, in words.
    pub decision_reason: Option<String>,
    /// Why the CLI asks (`hook`, `rule`, `mode`, `safetyCheck`, …).
    pub decision_reason_type: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub tool_use_id: String,
    /// Rules an "always allow" answer may echo back as `updatedPermissions`.
    #[serde(deserialize_with = "nullable")]
    pub permission_suggestions: Vec<Value>,
    pub blocked_path: Option<String>,
    /// True when no persistent "always allow" may be offered.
    #[serde(deserialize_with = "nullable")]
    pub suppress_always_allow_rule: bool,
    /// True when the user answers on the card itself (`AskUserQuestion`).
    #[serde(deserialize_with = "nullable")]
    pub requires_user_interaction: bool,
    /// Set when a subagent asks.
    pub agent_id: Option<String>,
}

/// `hook_callback`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct HookCallback {
    /// One of the ids the app registered.
    #[serde(deserialize_with = "nullable")]
    pub callback_id: String,
    /// The hook input (`hook_event_name`, `tool_name`, `tool_input`, …).
    #[serde(deserialize_with = "nullable")]
    pub input: Value,
    pub tool_use_id: Option<String>,
}

/// `request_user_dialog`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct UserDialog {
    #[serde(deserialize_with = "nullable")]
    pub dialog_kind: String,
    #[serde(deserialize_with = "nullable")]
    pub payload: Value,
    pub tool_use_id: Option<String>,
}

/// `elicitation`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Elicitation {
    #[serde(deserialize_with = "nullable")]
    pub mcp_server_name: String,
    #[serde(deserialize_with = "nullable")]
    pub message: String,
    /// `form` or `url`.
    pub mode: Option<String>,
    pub url: Option<String>,
    pub requested_schema: Option<Value>,
    pub title: Option<String>,
}

/// The reply to one of the app's control requests.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlResponse {
    /// The app's request id.
    pub request_id: String,
    /// The payload (`{}` for a bare acknowledgement), or the CLI's error text.
    pub outcome: Result<Value, String>,
}

impl ControlResponse {
    /// The success payload as `T`; `None` for an error or a payload of another shape.
    pub fn parse<T: DeserializeOwned>(&self) -> Option<T> {
        self.outcome.as_ref().ok().and_then(parse)
    }
}

/// Reads a `control_response` frame; `None` without a request id.
pub(super) fn decode_response(frame: Value) -> Option<ControlResponse> {
    let response = frame.get("response")?;
    let request_id = response.get("request_id").and_then(Value::as_str)?.to_owned();
    let subtype = response.get("subtype").and_then(Value::as_str).unwrap_or_default();

    let outcome = if subtype == "error" {
        let error = response.get("error").and_then(Value::as_str).unwrap_or_default();
        Err(error.to_owned())
    } else {
        let payload = response.get("response").filter(|payload| !payload.is_null());
        Ok(payload.cloned().unwrap_or_else(|| Value::Object(Default::default())))
    };

    Some(ControlResponse { request_id, outcome })
}

fn parse<T: DeserializeOwned>(value: &Value) -> Option<T> {
    T::deserialize(value).ok()
}

/// A slash command (`initialize`, `commands_changed`, `reload_plugins`, `reload_skills`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SlashCommand {
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
    #[serde(deserialize_with = "nullable")]
    pub argument_hint: String,
    #[serde(deserialize_with = "nullable")]
    pub aliases: Vec<String>,
    pub builtin: Option<bool>,
}

/// A subagent type the session knows.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct AgentInfo {
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
}

/// A model the session can switch to (`initialize`, `list_models`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelInfo {
    /// What `set_model` takes (`default`, `opus`, `sonnet`, …).
    #[serde(deserialize_with = "nullable")]
    pub value: String,
    /// The model id it stands for.
    pub resolved_model: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub supports_effort: bool,
    /// `low`, `medium`, `high`, `xhigh`, `max`.
    #[serde(deserialize_with = "nullable")]
    pub supported_effort_levels: Vec<String>,
    pub supports_thinking_off: Option<bool>,
}

/// The reply to `initialize` (the account is never read).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct InitializeReply {
    #[serde(deserialize_with = "nullable")]
    pub commands: Vec<SlashCommand>,
    #[serde(deserialize_with = "nullable")]
    pub agents: Vec<AgentInfo>,
    #[serde(deserialize_with = "nullable")]
    pub models: Vec<ModelInfo>,
    pub output_style: Option<String>,
    pub claude_code_version: Option<String>,
    pub current_permission_mode: Option<String>,
    /// `idle`, `running` or `requires_action`.
    pub session_state: Option<String>,
    /// Permission prompts still open (a session joined mid-turn).
    #[serde(deserialize_with = "nullable")]
    pub pending_permission_requests: Vec<Value>,
}

/// The reply to `list_models`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ModelsReply {
    #[serde(deserialize_with = "nullable")]
    pub models: Vec<ModelInfo>,
}

/// The reply to `get_context_usage`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContextUsageReply {
    #[serde(deserialize_with = "nullable")]
    pub total_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    pub max_tokens: u64,
    pub raw_max_tokens: Option<u64>,
    /// Rounded percentage of `maxTokens`.
    pub percentage: Option<f64>,
    pub model: Option<String>,
    pub is_auto_compact_enabled: Option<bool>,
    pub auto_compact_threshold: Option<u64>,
}

/// The reply to `interrupt`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct InterruptReply {
    /// Queued messages that survive the interrupt.
    #[serde(deserialize_with = "nullable")]
    pub still_queued: Vec<String>,
    /// Queued messages the interrupt cancelled (`cancel_queued`).
    #[serde(deserialize_with = "nullable")]
    pub cancelled: Vec<String>,
}

/// The reply to `cancel_async_message`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct CancelReply {
    /// False when the message already ran.
    #[serde(deserialize_with = "nullable")]
    pub cancelled: bool,
}

/// The reply to `get_session_cost`: the `/cost` text.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct SessionCostReply {
    #[serde(deserialize_with = "nullable")]
    pub text: String,
}

/// The reply to `reload_skills`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct SkillsReply {
    #[serde(deserialize_with = "nullable")]
    pub skills: Vec<SlashCommand>,
}

/// The reply to `reload_plugins`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct PluginsReply {
    #[serde(deserialize_with = "nullable")]
    pub commands: Vec<SlashCommand>,
    #[serde(deserialize_with = "nullable")]
    pub agents: Vec<AgentInfo>,
    #[serde(deserialize_with = "nullable")]
    pub error_count: u32,
}

/// The reply to `set_permission_mode`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct PermissionModeReply {
    /// The mode now in force, as the CLI names it (`manual` is reported as `default`).
    pub mode: Option<String>,
}
