//! Permission prompts, questions and the other asks of a [`ClaudeSession`]: the dialogs are those
//! of [`permissions`](crate::claude_stream::permissions) and
//! [`questions`](crate::claude_stream::questions).
//!
//! - **Permission prompts** (also a subagent's): one the chat's own "Always allow" answers cover
//!   is allowed at once (never one the CLI suppresses lasting rules for, blocked on a path or
//!   flagged by a safety check), any other shows its dialog until the user answers it, the CLI
//!   withdraws it (`control_cancel_request`: the dialog closes) or the user stops the turn
//!   (denied, see `prompts.rs`). An "Always allow" answer the chat keeps also allows the open prompts it
//!   covers, and their dialogs close.
//! - **Questions:** one dialog per question; the answers go back after the last one, a dismissed
//!   dialog denies the call.
//! - **Other asks:** the app declares no dialog kinds, so a `request_user_dialog` is refused at
//!   once, and an MCP server's elicitation is declined at once: the chat cannot show either yet,
//!   and says so in a notice. The CLI never waits on them.
//! - **Always ask:** the hook callback answers `ask` for every tool call; callbacks of other ids
//!   get no opinion.
//! - **Helper chats** (AI naming) run without tools and nobody sees their dialogs: a permission
//!   prompt or question is denied at once.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::Chat;
use crate::claude_stream::allow_list::AllowList;
use crate::claude_stream::mapper::{Ask, PermissionAsk, QuestionAsk};
use crate::claude_stream::permissions::{ASK_ALL_CALLBACK, Always, Choice, DECLINED, choice, permission_dialog};
use crate::claude_stream::questions::QuestionDialog;
use crate::claude_stream::wire::control::HookCallback;
use crate::claude_stream::wire::outbound::{
    ask_hook_output, control_error, control_success, hook_output, permission_allow, permission_deny, question_answers,
};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, MAX_ACTION_TEXT_BYTES, NoticeLevel, UiOutcome, UiResponse};
use crate::security::redact_text;

/// What the model is told when the user dismissed its questions.
const DISMISSED: &str = "The user dismissed the questions without answering.";

/// What the model is told about questions with nothing to ask.
const NO_QUESTIONS: &str = "The questions could not be shown: they were empty.";

/// What a helper chat answers every permission prompt and question with.
const HELPER: &str = "This helper chat runs no tools.";

/// The open asks of a chat and its own "Always allow" answers.
pub(super) struct Dialogs {
    project: PathBuf,
    /// The chat runs the Always ask hook.
    ask_all: bool,
    /// A helper chat, which nobody watches: every ask is denied at once.
    helper: bool,
    open: Vec<Open>,
    allowed: AllowList,
}

/// An ask whose dialog is on screen.
enum Open {
    Permission {
        ask: Box<PermissionAsk>,
        always: Option<Always>,
    },
    Question {
        ask: QuestionAsk,
        /// The id of the dialog on screen (the request id, then one per question).
        dialog: String,
        questions: QuestionDialog,
    },
}

impl Open {
    fn dialog(&self) -> &str {
        match self {
            Self::Permission { ask, .. } => &ask.request_id,
            Self::Question { dialog, .. } => dialog,
        }
    }

    fn request_id(&self) -> &str {
        match self {
            Self::Permission { ask, .. } => &ask.request_id,
            Self::Question { ask, .. } => &ask.request_id,
        }
    }
}

/// What the chat emits and writes after an ask or an answer.
#[derive(Default)]
pub(super) struct Outcome {
    pub(super) events: Vec<AgentEvent>,
    pub(super) answers: Vec<Value>,
}

impl Dialogs {
    pub(super) fn new(project: &Path, ask_all: bool, helper: bool) -> Self {
        Self {
            project: project.to_path_buf(),
            ask_all,
            helper,
            open: Vec::new(),
            allowed: AllowList::new(project),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// The CLI asks `ask` (see the module docs).
    pub(super) fn asked(&mut self, ask: Ask, out: &mut Outcome) {
        match ask {
            Ask::Permission(ask) if self.helper => out.answers.push(permission_deny(&ask.request_id, HELPER)),
            Ask::Question(ask) if self.helper => out.answers.push(permission_deny(&ask.request_id, HELPER)),
            Ask::Permission(ask) => self.permission(ask, out),
            Ask::Question(ask) => match QuestionDialog::new(&ask) {
                Some(questions) => {
                    out.events.push(questions.first());
                    self.forget(&ask.request_id);
                    self.open.push(Open::Question {
                        dialog: ask.request_id.clone(),
                        ask,
                        questions,
                    });
                }
                None => out.answers.push(permission_deny(&ask.request_id, NO_QUESTIONS)),
            },
            Ask::Dialog { request_id, dialog } => {
                let kind = redact_text(&dialog.dialog_kind);
                out.answers.push(control_error(
                    &request_id,
                    &format!("the desktop app shows no {kind} dialogs"),
                ));
                out.events.push(notice(format!(
                    "Claude Code asked for its {kind} dialog, which this chat cannot show yet."
                )));
            }
            Ask::Elicitation {
                request_id,
                elicitation,
            } => {
                out.answers
                    .push(control_success(&request_id, json!({ "action": "decline" })));
                out.events.push(notice(format!(
                    "The MCP server {} asked for input, which this chat cannot show yet. It was declined.",
                    redact_text(&elicitation.mcp_server_name)
                )));
            }
        }
    }

    fn permission(&mut self, ask: Box<PermissionAsk>, out: &mut Outcome) {
        if self.allowed.covers(&ask) {
            out.answers.push(permission_allow(&ask.request_id, &ask.input, &[]));
            return;
        }

        let always = Always::of(&ask, &self.project, self.ask_all);
        out.events.push(permission_dialog(&ask, &self.project, always.as_ref()));

        self.forget(&ask.request_id);
        self.open.push(Open::Permission { ask, always });
    }

    /// The CLI withdrew its request `request_id`: closes its dialog.
    pub(super) fn withdrawn(&mut self, request_id: &str) -> Option<AgentEvent> {
        let position = self.open.iter().position(|open| open.request_id() == request_id)?;
        let open = self.open.remove(position);

        Some(cancel(open.dialog()))
    }

    /// The user's answer to dialog `response.id`; fails for a dialog that is not open.
    pub(super) fn answer(&mut self, response: &UiResponse, out: &mut Outcome) -> Result<()> {
        let position = self
            .open
            .iter()
            .position(|open| open.dialog() == response.id)
            .ok_or_else(|| BusinessError::NotFound(format!("open dialog {}", response.id)))?;

        if let Open::Question { dialog, questions, .. } = &mut self.open[position]
            && let UiOutcome::Value { value } = &response.outcome
            && let Some((next, event)) = questions.answer(value)?
        {
            *dialog = next;
            out.events.push(event);
            return Ok(());
        }

        // A wrong option fails before the ask leaves the list, so its dialog stays open.
        let picked = match &self.open[position] {
            Open::Permission { always, .. } => Some(choice(&response.outcome, always.is_some())?),
            Open::Question { .. } => None,
        };

        match self.open.remove(position) {
            Open::Permission { ask, always } => {
                let picked = picked.unwrap_or(Choice::Reject);
                self.permission_answer(&ask, picked, always, out);
            }
            Open::Question { ask, questions, .. } => {
                let answer = match &response.outcome {
                    UiOutcome::Value { .. } => question_answers(&ask.request_id, &ask.input, &questions.answers()),
                    _ => permission_deny(&ask.request_id, DISMISSED),
                };
                out.answers.push(answer);
            }
        }

        Ok(())
    }

    fn permission_answer(&mut self, ask: &PermissionAsk, picked: Choice, always: Option<Always>, out: &mut Outcome) {
        let answer = match (picked, always) {
            (Choice::Reject, _) => permission_deny(&ask.request_id, DECLINED),
            (Choice::Always, Some(Always::Rules(rules))) => permission_allow(&ask.request_id, &ask.input, &rules),
            (Choice::Always, Some(Always::Key(key))) => {
                self.allowed.remember(key);
                permission_allow(&ask.request_id, &ask.input, &[])
            }
            _ => permission_allow(&ask.request_id, &ask.input, &[]),
        };

        out.answers.push(answer);

        if picked == Choice::Always {
            self.allow_covered(out);
        }
    }

    /// Allows the open permission prompts the chat's answers now cover; their dialogs close.
    fn allow_covered(&mut self, out: &mut Outcome) {
        let (covered, kept): (Vec<Open>, Vec<Open>) = std::mem::take(&mut self.open)
            .into_iter()
            .partition(|open| matches!(open, Open::Permission { ask, .. } if self.allowed.covers(ask)));
        self.open = kept;

        for open in covered {
            if let Open::Permission { ask, .. } = open {
                out.events.push(cancel(&ask.request_id));
                out.answers.push(permission_allow(&ask.request_id, &ask.input, &[]));
            }
        }
    }

    /// The answer to a hook callback (see the module docs).
    pub(super) fn hook_answer(&self, request_id: &str, callback: &HookCallback) -> Value {
        if self.ask_all && callback.callback_id == ASK_ALL_CALLBACK {
            return hook_output(request_id, ask_hook_output());
        }

        hook_output(request_id, json!({}))
    }

    /// Takes every open ask out: its dialog and the CLI's request id.
    pub(super) fn drain(&mut self) -> Vec<(String, String)> {
        self.open
            .drain(..)
            .map(|open| (open.dialog().to_owned(), open.request_id().to_owned()))
            .collect()
    }

    /// Drops the open ask of `request_id` (asked again).
    fn forget(&mut self, request_id: &str) {
        self.open.retain(|open| open.request_id() != request_id);
    }
}

impl Chat {
    /// Answers an open dialog; fails for ids that are not open (answered, withdrawn or unknown).
    pub(super) async fn answer_dialog(&self, response: UiResponse) -> Result<()> {
        if let UiOutcome::Value { value } = &response.outcome
            && value.len() > MAX_ACTION_TEXT_BYTES
        {
            return Err(BusinessError::InvalidInput(format!(
                "value exceeds {MAX_ACTION_TEXT_BYTES} bytes"
            )));
        }

        let mut out = Outcome::default();

        {
            let mut live = self.lock();
            live.dialogs.answer(&response, &mut out)?;

            for event in out.events {
                self.shared.emit(event);
            }
        }

        for answer in &out.answers {
            self.write(answer).await?;
        }

        Ok(())
    }
}

fn cancel(dialog: &str) -> AgentEvent {
    AgentEvent::UiCancel {
        request_id: dialog.to_owned(),
    }
}

fn notice(message: String) -> AgentEvent {
    AgentEvent::Notice {
        level: NoticeLevel::Warning,
        message,
    }
}
