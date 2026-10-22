//! What Codex asks a [`CodexSession`](super::CodexSession) as chat dialogs, under Codex's request
//! id, answered with the encoders of [`answers`](crate::codex_rpc::wire::answers).
//!
//! - **Approvals** (a command, a file change, extra permissions): a `select` with `Allow once`,
//!   `Always allow` and `Reject`, marked as an approval and bound to the item's tool card. `Allow
//!   once` accepts (permissions for the turn), `Always allow` is Codex's `acceptForSession`
//!   (permissions for the session), offered for a command only when Codex lists it, so nothing
//!   outlives the chat: the app never answers with a rule amendment, which Codex would save.
//!   `Reject` or a dismissed dialog declines (the turn goes on without it). Titles and details are
//!   redacted.
//! - **Questions** of the model and an MCP server's form: the question dialog (`forms.rs`). A
//!   dismissed dialog answers nothing (the questions) or declines (the form). A request to visit a
//!   URL is declined at once with a notice: the chat cannot open it yet.
//! - Codex withdraws an ask by resolving it elsewhere (`serverRequest/resolved`): its dialog
//!   closes. An abort cancels every open ask (`cancel`, no permissions, no answers).
//! - A helper chat (AI naming) declines every ask at once, as does any chat past [`MAX_OPEN_ASKS`]
//!   open ones.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::Chat;
use super::forms::{ACCEPT, Asked, Form, form, questions};
use crate::claude_stream::permissions::{ALLOW_ONCE, ALWAYS_ALLOW, Choice, REJECT, choice};
use crate::codex_rpc::mapper::{Ask, AskKind};
use crate::codex_rpc::wire::RequestId;
use crate::codex_rpc::wire::answers::{
    Decision, ElicitationAction, GrantScope, command_decision, elicitation_answer, file_change_decision,
    permissions_deny, permissions_grant, user_input_answers,
};
use crate::codex_rpc::wire::items::FileUpdateChange;
use crate::codex_rpc::wire::requests::{CommandApproval, PermissionsApproval};
use crate::error::{BusinessError, Result};
use crate::model::{
    AgentEvent, MAX_ACTION_TEXT_BYTES, NoticeLevel, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse,
};
use crate::security::redact_text;

/// The most asks open at once; Codex's next one is declined.
pub(super) const MAX_OPEN_ASKS: usize = 256;

/// The `available_decisions` name of Codex's session-wide approval.
const FOR_SESSION: &str = "acceptForSession";

/// The files a file change's title names before it counts the rest.
const NAMED_FILES: usize = 3;

/// The longest subject a title shows, in characters.
const MAX_SUBJECT_CHARS: usize = 300;

/// The open asks of a chat.
pub(super) struct Dialogs {
    project: PathBuf,
    /// A helper chat, which nobody watches: every ask is declined at once.
    helper: bool,
    open: Vec<Open>,
}

/// An ask whose dialog is on screen.
struct Open {
    request_id: RequestId,
    /// The id of the dialog on screen (the request id, then one per question).
    dialog: String,
    kind: OpenKind,
}

enum OpenKind {
    Command { always: bool },
    FileChange,
    Permissions { requested: Value },
    Questions(Asked),
    Form { asked: Asked, form: Form },
}

/// What the chat emits and writes after an ask or an answer.
#[derive(Default)]
pub(super) struct Outcome {
    pub(super) events: Vec<AgentEvent>,
    pub(super) answers: Vec<Value>,
}

impl Dialogs {
    pub(super) fn new(project: &Path, helper: bool) -> Self {
        Self {
            project: project.to_path_buf(),
            helper,
            open: Vec::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// Codex asks `ask` (see the module docs).
    pub(super) fn asked(&mut self, ask: Ask, out: &mut Outcome) {
        if self.helper || self.open.len() >= MAX_OPEN_ASKS {
            out.answers.push(declined(&ask));
            return;
        }

        let request_id = ask.request_id.clone();
        let dialog = request_id.to_string();
        let tool_call_id = ask.item_id().map(str::to_owned);

        let (event, kind) = match ask.kind {
            AskKind::Command(approval) => {
                let always = approval.offers(FOR_SESSION);
                let event = command_dialog(&dialog, &approval, always);
                (event, OpenKind::Command { always })
            }
            AskKind::FileChange { approval, changes } => {
                let subject = files(&changes, &self.project).or_else(|| approval.grant_root.clone());
                let title = titled("apply_patch", subject.as_deref());
                let always = "Allows changes to these files again until this chat ends";
                let event = approval_dialog(&dialog, &title, approval.reason.as_deref(), Some(always));
                (event, OpenKind::FileChange)
            }
            AskKind::Permissions(approval) => {
                let event = permissions_dialog(&dialog, &approval);
                let requested = approval.permissions.clone();
                (event, OpenKind::Permissions { requested })
            }
            AskKind::Questions(request) => match questions(&dialog, &request) {
                Some(asked) => (asked.dialog.first(), OpenKind::Questions(asked)),
                None => {
                    out.answers.push(declined(&Ask {
                        request_id,
                        kind: AskKind::Questions(request),
                    }));
                    return;
                }
            },
            AskKind::Elicitation(elicitation) => {
                let shown = (!elicitation.is_url()).then(|| form(&dialog, &elicitation)).flatten();

                let Some((asked, form)) = shown else {
                    out.events.push(notice(format!(
                        "The MCP server {} asked to open a web page, which this chat cannot show yet. It was declined.",
                        redact_text(&elicitation.server_name)
                    )));
                    out.answers
                        .push(elicitation_answer(&request_id, ElicitationAction::Decline, None));
                    return;
                };

                (asked.dialog.first(), OpenKind::Form { asked, form })
            }
        };

        let event = bound(event, tool_call_id);

        self.forget(&request_id);
        out.events.push(event);
        self.open.push(Open {
            request_id,
            dialog,
            kind,
        });
    }

    /// Codex resolved request `request_id`: closes its dialog.
    pub(super) fn withdrawn(&mut self, request_id: &RequestId) -> Option<AgentEvent> {
        let position = self.open.iter().position(|open| open.request_id == *request_id)?;
        let open = self.open.remove(position);

        Some(cancel(&open.dialog))
    }

    /// The user's answer to dialog `response.id`; fails for a dialog that is not open.
    pub(super) fn answer(&mut self, response: &UiResponse, out: &mut Outcome) -> Result<()> {
        let position = self
            .open
            .iter()
            .position(|open| open.dialog == response.id)
            .ok_or_else(|| BusinessError::NotFound(format!("open dialog {}", response.id)))?;

        let open = &mut self.open[position];

        if let OpenKind::Questions(asked) | OpenKind::Form { asked, .. } = &mut open.kind
            && let UiOutcome::Value { value } = &response.outcome
            && let Some((next, event)) = asked.dialog.answer(value)?
        {
            open.dialog = next;
            out.events.push(event);
            return Ok(());
        }

        // A wrong option fails before the ask leaves the list, so its dialog stays open.
        let picked = match &open.kind {
            OpenKind::Command { always } => Some(choice(&response.outcome, *always)?),
            OpenKind::FileChange | OpenKind::Permissions { .. } => Some(choice(&response.outcome, true)?),
            OpenKind::Questions(_) | OpenKind::Form { .. } => None,
        };

        let open = self.open.remove(position);
        let given = matches!(response.outcome, UiOutcome::Value { .. });
        out.answers.push(answer(&open, picked.unwrap_or(Choice::Reject), given));

        Ok(())
    }

    /// Takes every open ask out: its dialog closes and Codex is told it was cancelled.
    pub(super) fn cancel_all(&mut self, out: &mut Outcome) {
        for open in self.open.drain(..) {
            out.events.push(cancel(&open.dialog));
            out.answers.push(cancelled(&open));
        }
    }

    /// Drops the open ask of `request_id` (asked again).
    fn forget(&mut self, request_id: &RequestId) {
        self.open.retain(|open| open.request_id != *request_id);
    }
}

impl Chat {
    /// Answers an open dialog; fails for ids that are not open (answered, withdrawn or unknown).
    pub(super) async fn answer_dialog(&self, response: UiResponse) -> Result<()> {
        let size = match &response.outcome {
            UiOutcome::Value { value } => value.len(),
            _ => 0,
        };

        if size > MAX_ACTION_TEXT_BYTES {
            let error = format!("value exceeds {MAX_ACTION_TEXT_BYTES} bytes");
            return Err(BusinessError::InvalidInput(error));
        }

        let answers = {
            let mut live = self.lock();
            let mut out = Outcome::default();
            live.dialogs.answer(&response, &mut out)?;

            // Under the lock: the next question keeps its place among the reader's events.
            out.events.into_iter().for_each(|event| self.shared.emit(event));
            out.answers
        };

        for answer in answers {
            self.write(&answer).await?;
        }

        Ok(())
    }
}

/// The answer to `open` for the user's `picked` approval choice, or its questions (`given`: the
/// last one was answered, not dismissed).
fn answer(open: &Open, picked: Choice, given: bool) -> Value {
    let id = &open.request_id;
    let decision = match picked {
        Choice::Once => Decision::Accept,
        Choice::Always => Decision::AcceptForSession,
        Choice::Reject => Decision::Decline,
    };

    match &open.kind {
        OpenKind::Command { .. } => command_decision(id, decision),
        OpenKind::FileChange => file_change_decision(id, decision),
        OpenKind::Permissions { requested } => match picked {
            Choice::Once => permissions_grant(id, requested, GrantScope::Turn),
            Choice::Always => permissions_grant(id, requested, GrantScope::Session),
            Choice::Reject => permissions_deny(id),
        },
        OpenKind::Questions(asked) => {
            let answers = if given { asked.answers() } else { Vec::new() };
            user_input_answers(id, &answers.into_iter().collect())
        }
        OpenKind::Form { asked, form } => {
            let answers = asked.answers();
            let accepted = given && (!form.is_empty() || answers.iter().any(|(_, labels)| labels == &[ACCEPT]));

            if accepted {
                elicitation_answer(id, ElicitationAction::Accept, Some(form.content(&answers)))
            } else {
                elicitation_answer(id, ElicitationAction::Decline, None)
            }
        }
    }
}

/// The answer to `open` when the user stopped the turn.
fn cancelled(open: &Open) -> Value {
    let id = &open.request_id;

    match &open.kind {
        OpenKind::Command { .. } => command_decision(id, Decision::Cancel),
        OpenKind::FileChange => file_change_decision(id, Decision::Cancel),
        OpenKind::Permissions { .. } => permissions_deny(id),
        OpenKind::Questions(_) => user_input_answers(id, &Default::default()),
        OpenKind::Form { .. } => elicitation_answer(id, ElicitationAction::Cancel, None),
    }
}

/// The answer that declines `ask` without a dialog.
fn declined(ask: &Ask) -> Value {
    let id = &ask.request_id;

    match &ask.kind {
        AskKind::Command(_) => command_decision(id, Decision::Decline),
        AskKind::FileChange { .. } => file_change_decision(id, Decision::Decline),
        AskKind::Permissions(_) => permissions_deny(id),
        AskKind::Questions(_) => user_input_answers(id, &Default::default()),
        AskKind::Elicitation(_) => elicitation_answer(id, ElicitationAction::Decline, None),
    }
}

/// The dialog of a command: the command (or the host of a network approval) in its title.
fn command_dialog(dialog: &str, approval: &CommandApproval, always: bool) -> AgentEvent {
    let host = approval
        .network_approval_context
        .as_ref()
        .map(|context| context.host.as_str())
        .filter(|host| !host.is_empty());

    let title = match host {
        Some(host) => format!("Allow network access to {}?", one_line(host)),
        None => titled("shell", approval.shown_command()),
    };
    let always = always.then_some("Allows this command again until this chat ends");

    approval_dialog(dialog, &title, approval.reason.as_deref(), always)
}

/// The dialog of extra permissions: what they add in its title.
fn permissions_dialog(dialog: &str, approval: &PermissionsApproval) -> AgentEvent {
    let mut parts = Vec::new();

    if approval.network() {
        parts.push("network access".to_owned());
    }

    for (access, paths) in [("reading", approval.read_paths()), ("writing", approval.write_paths())] {
        if !paths.is_empty() {
            parts.push(format!("{access} {}", paths.join(", ")));
        }
    }

    let subject = (!parts.is_empty()).then(|| parts.join(", "));
    let title = titled("extra permissions", subject.as_deref());

    approval_dialog(
        dialog,
        &title,
        approval.reason.as_deref(),
        Some("Keeps these permissions until this chat ends"),
    )
}

/// `Allow <label>: <subject>?` on one line, or `Allow <label>?`.
fn titled(label: &str, subject: Option<&str>) -> String {
    match subject.map(one_line).filter(|subject| !subject.is_empty()) {
        Some(subject) => format!("Allow {label}: {subject}?"),
        None => format!("Allow {label}?"),
    }
}

/// The project files of `changes` (relative to `project`), the first few and a count of the rest.
fn files(changes: &[FileUpdateChange], project: &Path) -> Option<String> {
    let names: Vec<String> = changes
        .iter()
        .map(|change| {
            let path = Path::new(&change.path);
            let shown = path.strip_prefix(project).unwrap_or(path);
            shown.to_string_lossy().into_owned()
        })
        .collect();

    let (named, rest) = names.split_at(names.len().min(NAMED_FILES));
    let listed = named.join(", ");

    match rest.len() {
        0 if listed.is_empty() => None,
        0 => Some(listed),
        more => Some(format!("{listed} and {more} more")),
    }
}

/// An approval `select` as `dialog`, titled `title`, with `reason` as the detail of `Allow once`
/// and `always` as the one of `Always allow` (left out without it).
fn approval_dialog(dialog: &str, title: &str, reason: Option<&str>, always: Option<&str>) -> AgentEvent {
    let mut options = vec![ALLOW_ONCE.to_owned()];
    let mut details = vec![reason.map(redact_text)];

    if let Some(always) = always {
        options.push(ALWAYS_ALLOW.to_owned());
        details.push(Some(always.to_owned()));
    }

    options.push(REJECT.to_owned());
    details.push(None);

    let kind = UiRequestKind::Select {
        title: redact_text(title),
        options,
        details,
        timeout_ms: None,
    };

    AgentEvent::UiRequest {
        request: UiRequest {
            purpose: Some(UiPurpose::Approval),
            ..UiRequest::new(dialog, kind)
        },
    }
}

/// `event` (a dialog) bound to the tool card `tool_call_id` when it is an approval.
fn bound(event: AgentEvent, tool_call_id: Option<String>) -> AgentEvent {
    match event {
        AgentEvent::UiRequest { mut request } => {
            if request.purpose == Some(UiPurpose::Approval) {
                request.tool_call_id = tool_call_id;
            }

            AgentEvent::UiRequest { request }
        }
        other => other,
    }
}

/// `text` on one line, at most [`MAX_SUBJECT_CHARS`] characters.
fn one_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");

    match line.char_indices().nth(MAX_SUBJECT_CHARS) {
        Some((end, _)) => format!("{}…", &line[..end]),
        None => line,
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
