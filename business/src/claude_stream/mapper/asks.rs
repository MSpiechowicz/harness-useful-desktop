//! What the CLI asks the user, as data for the engine's dialogs: permission prompts
//! (`can_use_tool`), `AskUserQuestion` questions (a `can_use_tool` for that tool, answered with
//! [`question_answers`](crate::claude_stream::wire::outbound::question_answers)), dialogs
//! (`request_user_dialog`) and MCP elicitations. Inputs are kept as sent: an approval echoes the
//! input back, so whatever shows them must redact them.

use serde::Deserialize;
use serde_json::Value;

use crate::claude_stream::tools::ASK_USER_QUESTION;
use crate::claude_stream::wire::control::{CliRequest, Elicitation, PermissionRequest, UserDialog};
use crate::claude_stream::wire::nullable;

/// Something the CLI waits for the user to answer, under the CLI's `request_id`.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// May a tool run? Answer with `permission_allow` or `permission_deny`.
    Permission(Box<PermissionAsk>),
    /// `AskUserQuestion`: answer with `question_answers` (or deny to dismiss).
    Question(QuestionAsk),
    /// A dialog of a kind the app declared in `initialize`.
    Dialog { request_id: String, dialog: UserDialog },
    /// An MCP server asks for input.
    Elicitation {
        request_id: String,
        elicitation: Box<Elicitation>,
    },
}

impl Ask {
    /// The CLI's request id, the one its answer and its `control_cancel_request` carry.
    pub fn request_id(&self) -> &str {
        match self {
            Self::Permission(ask) => &ask.request_id,
            Self::Question(ask) => &ask.request_id,
            Self::Dialog { request_id, .. } | Self::Elicitation { request_id, .. } => request_id,
        }
    }
}

/// A permission prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct PermissionAsk {
    pub request_id: String,
    /// The tool call it is about (matches the `toolStart`).
    pub tool_call_id: String,
    pub tool: String,
    /// What to call the tool in the dialog (`display_name`, else the tool id).
    pub label: String,
    /// The call's input, as the allow answer echoes it.
    pub input: Value,
    /// A short description of the call (a file name, a command's description).
    pub description: Option<String>,
    /// Why the CLI asks, in words, when it says.
    pub reason: Option<String>,
    /// The rules an "always allow" answer echoes as `updatedPermissions`; empty when the CLI
    /// suggests none.
    pub suggestions: Vec<Value>,
    /// False when no persistent "always allow" may be offered.
    pub always_allowed: bool,
    /// True when a subagent asks.
    pub from_subagent: bool,
}

/// `AskUserQuestion` questions, asked together.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionAsk {
    pub request_id: String,
    pub tool_call_id: String,
    /// The tool input, which the answer echoes with the answers added.
    pub input: Value,
    pub questions: Vec<Question>,
}

/// One question.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Question {
    /// The question text; the answers are keyed by it.
    #[serde(deserialize_with = "nullable")]
    pub question: String,
    /// A short label for it.
    #[serde(deserialize_with = "nullable")]
    pub header: String,
    #[serde(deserialize_with = "nullable")]
    pub options: Vec<QuestionOption>,
    /// True when several options may be picked.
    #[serde(deserialize_with = "nullable")]
    pub multi_select: bool,
}

/// An option of a question.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct QuestionOption {
    /// What the answer names.
    #[serde(deserialize_with = "nullable")]
    pub label: String,
    pub description: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Questions {
    #[serde(deserialize_with = "nullable")]
    questions: Vec<Question>,
}

/// What the CLI's request `request_id` asks; `None` for one that is not a question to the user.
pub(super) fn ask(request_id: &str, request: &CliRequest) -> Option<Ask> {
    let ask = match request {
        CliRequest::CanUseTool(permission) if permission.tool_name == ASK_USER_QUESTION => {
            Ask::Question(question(request_id, permission))
        }
        CliRequest::CanUseTool(permission) => Ask::Permission(Box::new(permission_ask(request_id, permission))),
        CliRequest::RequestUserDialog(dialog) => Ask::Dialog {
            request_id: request_id.to_owned(),
            dialog: dialog.clone(),
        },
        CliRequest::Elicitation(elicitation) => Ask::Elicitation {
            request_id: request_id.to_owned(),
            elicitation: Box::new(elicitation.clone()),
        },
        CliRequest::HookCallback(_) | CliRequest::Unknown { .. } => return None,
    };

    Some(ask)
}

fn permission_ask(request_id: &str, permission: &PermissionRequest) -> PermissionAsk {
    let label = permission
        .display_name
        .clone()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| permission.tool_name.clone());

    PermissionAsk {
        request_id: request_id.to_owned(),
        tool_call_id: permission.tool_use_id.clone(),
        tool: permission.tool_name.clone(),
        label,
        input: permission.input.clone(),
        description: permission.description.clone().filter(|text| !text.is_empty()),
        reason: permission.decision_reason.clone().filter(|text| !text.is_empty()),
        suggestions: permission.permission_suggestions.clone(),
        always_allowed: !permission.suppress_always_allow_rule,
        from_subagent: permission.agent_id.is_some(),
    }
}

fn question(request_id: &str, permission: &PermissionRequest) -> QuestionAsk {
    let questions = Questions::deserialize(&permission.input)
        .map(|questions| questions.questions)
        .unwrap_or_default();

    QuestionAsk {
        request_id: request_id.to_owned(),
        tool_call_id: permission.tool_use_id.clone(),
        input: permission.input.clone(),
        questions,
    }
}
