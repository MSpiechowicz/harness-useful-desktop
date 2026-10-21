//! OpenCode 2 permission requests and forms as chat dialogs, and the answers to them, shown as the
//! OpenCode 1 mapper shows its permission asks and questions.
//!
//! - **Permission** (`permission.asked`): a `select` with `Allow once`, `Always allow` and
//!   `Reject`, marked as an approval and bound to the tool call that asks (`source.id`, the id of
//!   its `toolStart`). It is titled `Allow <action>: <subject>?` (`for a subagent` when a subagent
//!   session asks), the subject in full on one line: the command of a `shell` call, the URL or
//!   query of a web call, the description of a `subagent` call, else the requested resources
//!   (files relative to the session's directory). Answers: `once`, `always`, `reject`; a
//!   dismissed dialog rejects. The chat keeps an `always` answer to itself (the engine tells
//!   OpenCode `once`, which would save `always` for the project), so the `Always allow` detail
//!   says it lasts until the chat ends.
//! - **Forms** (`form.created`, the `question` tool's questions among them) are marked as
//!   questions, so the UI never shows one as an approval card, and shown one dialog per field, in
//!   order (see [`super::form`]). The answers go out together after the last one, as
//!   `{answer: {key: value}}`; a dismissed dialog cancels the form.
//! - `permission.replied` (answered elsewhere, or rejected along with another ask of the session),
//!   `form.replied` and `form.cancelled` close the dialog (`uiCancel`), as does the end of the
//!   turn.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::form::FormAsk;
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse};
use crate::opencode_server_v2::wire::{Decision, Form, FormReplyBody, PermissionRequest};
use crate::security::redact_text;

/// The answers of a permission dialog, in order.
pub const PERMISSION_OPTIONS: [&str; 3] = ["Allow once", "Always allow", "Reject"];

/// What OpenCode is told for each of [`PERMISSION_OPTIONS`].
const DECISIONS: [Decision; 3] = [Decision::Once, Decision::Always, Decision::Reject];

/// What `Allow once` means for a `subagent` ask without an approval mode.
const SUBAGENT_DETAIL: &str = "The subagent then runs its own tools under OpenCode's permission settings.";

/// What `Allow once` means for a `subagent` ask with an approval mode.
const SUBAGENT_DETAIL_MODE: &str = "The subagent's own tools then follow this chat's approval mode.";

/// How long an `always` answer applies: the chat keeps it, OpenCode saves nothing.
const UNTIL_END: &str = " until this chat ends";

/// What answering a dialog means for OpenCode.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Nothing to send yet: the next field of the form is shown.
    Next(Vec<AgentEvent>),
    /// Send this reply.
    Send(Reply),
}

/// A reply to an ask of `session`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// The session that asked (the reply routes are session-scoped).
    pub session: String,
    /// What to send.
    pub kind: ReplyKind,
}

/// The route and body of a [`Reply`].
#[derive(Debug, Clone, PartialEq)]
pub enum ReplyKind {
    /// `POST …/permission/{id}/reply {decision}`.
    Permission {
        /// The permission request (`per_…`).
        id: String,
        /// `once`, `always` or `reject`.
        decision: Decision,
    },
    /// `POST …/form/{id}/reply {answer}`.
    Form {
        /// The form (`frm_…`).
        id: String,
        /// The answers by field key.
        body: FormReplyBody,
    },
    /// `POST …/form/{id}/cancel`.
    FormCancel {
        /// The form.
        id: String,
    },
}

/// The open dialogs of a chat.
pub(super) struct Asks {
    directory: PathBuf,
    open: Vec<Ask>,
    /// The chat's server applies an approval mode to every agent.
    mode_applied: bool,
}

struct Ask {
    /// The id of the dialog on screen.
    dialog: String,
    session: String,
    kind: AskKind,
}

enum AskKind {
    Permission { id: String },
    Form(FormAsk),
}

impl Asks {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            open: Vec::new(),
            mode_applied: false,
        }
    }

    /// Whether the chat's server applies an approval mode to every agent (the `subagent` detail).
    pub(super) fn set_mode_applied(&mut self, applied: bool) {
        self.mode_applied = applied;
    }

    /// A `permission.asked`; `subagent` when a subagent session asks, `input` the input of the
    /// chat's tool call that asks, when known.
    pub(super) fn permission_asked(
        &mut self,
        permission: &PermissionRequest,
        subagent: bool,
        input: Option<&Value>,
        out: &mut Vec<AgentEvent>,
    ) {
        if permission.id.is_empty() || permission.session_id.is_empty() {
            return;
        }

        let action = if permission.action.is_empty() {
            "a tool"
        } else {
            permission.action.as_str()
        };
        let asker = if subagent { " for a subagent" } else { "" };
        let title = match self.subject(action, permission, input) {
            Some(subject) => format!("Allow {action}{asker}: {subject}?"),
            None => format!("Allow {action}{asker}?"),
        };

        let always_detail = match permission.save.as_slice() {
            [] => None,
            [only] if only == "*" => Some(format!("Allows every {action} call{UNTIL_END}")),
            patterns => Some(format!("Also allows {}{UNTIL_END}", patterns.join(", "))),
        };
        let subagent_detail = if self.mode_applied {
            SUBAGENT_DETAIL_MODE
        } else {
            SUBAGENT_DETAIL
        };
        let once_detail = matches!(action, "subagent" | "task").then(|| subagent_detail.to_owned());

        let kind = UiRequestKind::Select {
            title,
            options: PERMISSION_OPTIONS.map(str::to_owned).to_vec(),
            details: vec![once_detail, always_detail.map(|detail| redact_text(&detail)), None],
            timeout_ms: None,
        };
        // A subagent's tool calls are not in the chat's transcript: no card to bind to.
        let tool_call_id = permission
            .source
            .as_ref()
            .filter(|source| source.kind == "tool" && !source.id.is_empty() && !subagent)
            .map(|source| source.id.clone());

        out.push(AgentEvent::UiRequest {
            request: UiRequest {
                purpose: Some(UiPurpose::Approval),
                tool_call_id,
                ..UiRequest::new(permission.id.clone(), kind)
            },
        });

        self.forget(&permission.id);
        self.open.push(Ask {
            dialog: permission.id.clone(),
            session: permission.session_id.clone(),
            kind: AskKind::Permission {
                id: permission.id.clone(),
            },
        });
    }

    /// A `form.created`: shows its first field. A form with a field the chat cannot ask (an
    /// `external` sign-in) is not shown.
    pub(super) fn form_created(&mut self, form: &Form, out: &mut Vec<AgentEvent>) {
        if form.id.is_empty() || form.session_id.is_empty() {
            return;
        }

        let Some(ask) = FormAsk::new(form) else {
            return;
        };

        out.push(ask.dialog(&form.id));

        self.forget(&form.id);
        self.open.push(Ask {
            dialog: form.id.clone(),
            session: form.session_id.clone(),
            kind: AskKind::Form(ask),
        });
    }

    /// `permission.replied`, `form.replied` or `form.cancelled` of request `id`: closes its
    /// dialog if it is still open.
    pub(super) fn answered_elsewhere(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let position = self.open.iter().position(|ask| ask.request_id() == id);

        if let Some(position) = position {
            let ask = self.open.remove(position);
            out.push(AgentEvent::UiCancel { request_id: ask.dialog });
        }
    }

    /// Drops the open dialog of request `id` (it is shown again).
    fn forget(&mut self, id: &str) {
        self.open.retain(|ask| ask.request_id() != id);
    }

    /// Closes every open dialog (the turn ended).
    pub(super) fn cancel_all(&mut self, out: &mut Vec<AgentEvent>) {
        for ask in self.open.drain(..) {
            out.push(AgentEvent::UiCancel { request_id: ask.dialog });
        }
    }

    /// The answer to dialog `response.id` (see the module docs).
    pub(super) fn answer(&mut self, response: &UiResponse) -> Result<Answer> {
        let position = self
            .open
            .iter()
            .position(|ask| ask.dialog == response.id)
            .ok_or_else(|| BusinessError::NotFound(format!("open dialog {}", response.id)))?;

        let ask = &mut self.open[position];

        let kind = match &mut ask.kind {
            AskKind::Permission { id } => ReplyKind::Permission {
                id: id.clone(),
                decision: permission_decision(&response.outcome)?,
            },
            AskKind::Form(form) => match &response.outcome {
                UiOutcome::Value { value } => {
                    if let Some(next_id) = form.answer(value)? {
                        let next = form.dialog(&next_id);
                        ask.dialog = next_id;

                        return Ok(Answer::Next(vec![next]));
                    }

                    ReplyKind::Form {
                        id: form.id.clone(),
                        body: form.reply(),
                    }
                }
                _ => ReplyKind::FormCancel { id: form.id.clone() },
            },
        };

        let ask = self.open.remove(position);

        Ok(Answer::Send(Reply {
            session: ask.session,
            kind,
        }))
    }

    /// What a permission request is about, in full on one redacted line (see the module docs).
    fn subject(&self, action: &str, permission: &PermissionRequest, input: Option<&Value>) -> Option<String> {
        let field = |keys: &[&str]| {
            let input = input?;
            keys.iter().find_map(|key| text(input, key))
        };

        let named = match action {
            "shell" => field(&["command"]),
            "webfetch" | "websearch" => field(&["url", "query"]),
            "subagent" | "task" => field(&["description", "subagent_type", "agent"]),
            _ => None,
        };

        let subject = named.unwrap_or_else(|| {
            let resources: Vec<String> = permission
                .resources
                .iter()
                .filter(|resource| resource.as_str() != "*")
                .map(|resource| self.relative(resource))
                .collect();

            resources.join(", ")
        });
        let line: String = subject.split_whitespace().collect::<Vec<_>>().join(" ");

        if line.is_empty() {
            return None;
        }

        Some(redact_text(&line))
    }

    fn relative(&self, path: &str) -> String {
        Path::new(path)
            .strip_prefix(&self.directory)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .ok()
            .filter(|relative| !relative.is_empty())
            .unwrap_or_else(|| path.to_owned())
    }
}

impl Ask {
    fn request_id(&self) -> &str {
        match &self.kind {
            AskKind::Permission { id } => id,
            AskKind::Form(form) => &form.id,
        }
    }
}

/// What OpenCode is told for a permission dialog's answer; a dismissed one rejects.
fn permission_decision(outcome: &UiOutcome) -> Result<Decision> {
    match outcome {
        UiOutcome::Value { value } => PERMISSION_OPTIONS
            .iter()
            .position(|option| option == value)
            .map(|index| DECISIONS[index])
            .ok_or_else(|| BusinessError::InvalidInput(format!("{value} is not one of the options"))),
        UiOutcome::Confirmed { confirmed: true } => Ok(Decision::Once),
        _ => Ok(Decision::Reject),
    }
}

/// A non-empty string field.
pub(super) fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
