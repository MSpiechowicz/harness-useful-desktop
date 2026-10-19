//! OpenCode's permission asks and questions as chat dialogs, and the answers to them.
//!
//! - **Permission** (`permission.asked`): a `select` with `Allow once`, `Always allow` and
//!   `Reject`, marked as an approval and bound to the tool call that asks (`tool.callID`, the id
//!   of its `toolStart`). It is titled `Allow <tool>: <subject>?` (`for a subagent` when a
//!   subagent session asks), the subject in full on one line. Answers: `once`, `always`,
//!   `reject`; a dismissed dialog rejects.
//! - **Questions** (`question.asked`) are marked as questions, so the UI never shows one as an
//!   approval card, and shown one dialog per question, in order: a `select` for a single choice,
//!   with a trailing [`OWN_ANSWER`] option unless the question sets `custom: false` (choosing it
//!   asks for the text in an `input`), an `input` for several choices (comma-separated, typed
//!   answers too) or a free answer. The answers go out together after the last one; a dismissed
//!   dialog rejects the whole request.
//! - `permission.replied` (answered elsewhere, or rejected along with another ask of the session),
//!   `question.replied` and `question.rejected` close the dialog (`uiCancel`), as does the end of
//!   the turn (OpenCode drops its open asks when a turn stops, without an event).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::message::text;
use super::question::{Question, question, typed};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse};
use crate::security::redact_text;

/// The answers of a permission dialog, in order.
pub const PERMISSION_OPTIONS: [&str; 3] = ["Allow once", "Always allow", "Reject"];

/// What OpenCode is told for each of [`PERMISSION_OPTIONS`].
const PERMISSION_REPLIES: [&str; 3] = ["once", "always", "reject"];

/// What `Allow once` means for a `task` ask without an approval mode: the subagent's own tools
/// follow OpenCode's rules.
const TASK_DETAIL: &str = "The subagent then runs its own tools under OpenCode's permission settings.";

/// What `Allow once` means for a `task` ask with an approval mode, which every agent follows.
const TASK_DETAIL_MODE: &str = "The subagent's own tools then follow this chat's approval mode.";

/// What "Always allow" also means: OpenCode checks it after every rule, for every agent of the
/// chat's server (`permission/index.ts`, the instance's approved list).
const OVERRIDES: &str = ", for every agent and even where one denies it (the plan agent too)";

/// What answering a dialog means for OpenCode.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Nothing to send yet: the next question of the request is shown.
    Next(Vec<AgentEvent>),
    /// Send this reply.
    Send(Reply),
}

/// A reply to an ask of `session`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The session that asked (its directory goes with the reply).
    pub session: String,
    /// What to send.
    pub kind: ReplyKind,
}

/// The route and body of a [`Reply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyKind {
    /// `POST /permission/{id}/reply {reply}` with `once`, `always` or `reject`.
    Permission {
        /// The permission request (`per_…`).
        id: String,
        /// `once`, `always` or `reject`.
        reply: &'static str,
    },
    /// `POST /question/{id}/reply {answers}`: the chosen labels (or typed answers) of each
    /// question.
    Question {
        /// The question request (`que_…`).
        id: String,
        /// The answers, in question order.
        answers: Vec<Vec<String>>,
    },
    /// `POST /question/{id}/reject`.
    QuestionReject {
        /// The question request.
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
    Question(QuestionAsk),
}

struct QuestionAsk {
    id: String,
    questions: Vec<Question>,
    answers: Vec<Vec<String>>,
    /// The current question was answered with the typed-answer option: its typed answer comes
    /// next.
    typing: bool,
}

impl Asks {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            open: Vec::new(),
            mode_applied: false,
        }
    }

    /// Whether the chat's server applies an approval mode to every agent (the `task` detail).
    pub(super) fn set_mode_applied(&mut self, applied: bool) {
        self.mode_applied = applied;
    }

    /// A `permission.asked` of `session`; `subagent` when a subagent session asks.
    pub(super) fn permission_asked(&mut self, properties: &Value, subagent: bool, out: &mut Vec<AgentEvent>) {
        let (Some(id), Some(session)) = (text(properties, "id"), text(properties, "sessionID")) else {
            return;
        };

        let permission = text(properties, "permission").unwrap_or_else(|| "a tool".to_owned());
        let subject = self.subject(&permission, properties);
        let asker = if subagent { " for a subagent" } else { "" };
        let title = match subject {
            Some(subject) => format!("Allow {permission}{asker}: {subject}?"),
            None => format!("Allow {permission}{asker}?"),
        };

        let always = strings(properties, "always");
        let always_detail = match always.as_slice() {
            [] => None,
            [only] if only == "*" => Some(format!(
                "Allows every {permission} call until this chat ends{OVERRIDES}"
            )),
            patterns => Some(format!(
                "Also allows {} until this chat ends{OVERRIDES}",
                patterns.join(", ")
            )),
        };
        let task_detail = if self.mode_applied {
            TASK_DETAIL_MODE
        } else {
            TASK_DETAIL
        };
        let once_detail = (permission == "task").then(|| task_detail.to_owned());

        let kind = UiRequestKind::Select {
            title,
            options: PERMISSION_OPTIONS.map(str::to_owned).to_vec(),
            details: vec![once_detail, always_detail.map(|detail| redact_text(&detail)), None],
            timeout_ms: None,
        };
        // A subagent's tool calls are not in the chat's transcript: no card to bind to.
        let tool_call_id = properties
            .get("tool")
            .and_then(|tool| text(tool, "callID"))
            .filter(|_| !subagent);

        out.push(AgentEvent::UiRequest {
            request: UiRequest {
                purpose: Some(UiPurpose::Approval),
                tool_call_id,
                ..UiRequest::new(id.clone(), kind)
            },
        });

        self.open.push(Ask {
            dialog: id.clone(),
            session,
            kind: AskKind::Permission { id },
        });
    }

    /// A `question.asked`: shows its first question.
    pub(super) fn question_asked(&mut self, properties: &Value, out: &mut Vec<AgentEvent>) {
        let (Some(id), Some(session)) = (text(properties, "id"), text(properties, "sessionID")) else {
            return;
        };

        let questions: Vec<Question> = properties
            .get("questions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(question)
            .collect();

        if questions.is_empty() {
            return;
        }

        let ask = QuestionAsk {
            id: id.clone(),
            questions,
            answers: Vec::new(),
            typing: false,
        };
        out.push(ask.dialog(&id));

        self.open.push(Ask {
            dialog: id,
            session,
            kind: AskKind::Question(ask),
        });
    }

    /// `permission.replied`, `question.replied` or `question.rejected` of request `id`: closes its
    /// dialog if it is still open.
    pub(super) fn answered_elsewhere(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let position = self.open.iter().position(|ask| ask.request_id() == id);

        if let Some(position) = position {
            let ask = self.open.remove(position);
            out.push(AgentEvent::UiCancel { request_id: ask.dialog });
        }
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
                reply: permission_reply(&response.outcome)?,
            },
            AskKind::Question(question) => match &response.outcome {
                UiOutcome::Value { value } => {
                    if let Some(id) = question.answer(value)? {
                        let next = question.dialog(&id);
                        ask.dialog = id;

                        return Ok(Answer::Next(vec![next]));
                    }

                    ReplyKind::Question {
                        id: question.id.clone(),
                        answers: std::mem::take(&mut question.answers),
                    }
                }
                _ => ReplyKind::QuestionReject {
                    id: question.id.clone(),
                },
            },
        };

        let ask = self.open.remove(position);

        Ok(Answer::Send(Reply {
            session: ask.session,
            kind,
        }))
    }

    /// What a permission ask is about, in full on one redacted line: the command, the file
    /// (relative to the session's directory), the URL, or the patterns.
    fn subject(&self, permission: &str, properties: &Value) -> Option<String> {
        let metadata = properties.get("metadata").unwrap_or(&Value::Null);
        let field = |keys: &[&str]| keys.iter().find_map(|key| text(metadata, key));

        let named = match permission {
            "bash" => field(&["command"]),
            "webfetch" | "websearch" => field(&["url", "query"]),
            "task" => field(&["description", "subagent_type"]),
            _ => field(&["filepath", "filePath", "path"]).map(|path| self.relative(&path)),
        };

        let subject = named.unwrap_or_else(|| strings(properties, "patterns").join(", "));
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
            AskKind::Question(question) => &question.id,
        }
    }
}

impl QuestionAsk {
    fn current(&self) -> Result<&Question> {
        self.questions
            .get(self.answers.len())
            .ok_or_else(|| BusinessError::Internal("question without a current step".to_owned()))
    }

    /// Takes `value` as the answer to the current question. Returns the id of the dialog that comes
    /// next (the typed answer, or the next question), `None` once every question is answered.
    fn answer(&mut self, value: &str) -> Result<Option<String>> {
        let current = self.current()?;
        let step = self.answers.len() + 1;

        if !self.typing && current.is_own_answer(value) {
            self.typing = true;

            return Ok(Some(format!("{}:{step}:own", self.id)));
        }

        let answer = if self.typing {
            typed(value)
        } else {
            current.answer(value)?
        };
        self.typing = false;
        self.answers.push(answer);

        if self.answers.len() < self.questions.len() {
            return Ok(Some(format!("{}:{}", self.id, self.answers.len() + 1)));
        }

        Ok(None)
    }

    /// The dialog of the current question (or of its typed answer), as `id`.
    fn dialog(&self, id: &str) -> AgentEvent {
        let step = self.answers.len();
        let current = &self.questions[step];

        let title = match self.questions.len() {
            1 => current.text.clone(),
            count => format!("Question {} of {count}: {}", step + 1, current.text),
        };

        let kind = if self.typing {
            UiRequestKind::Input {
                title,
                placeholder: None,
                timeout_ms: None,
            }
        } else {
            current.kind(title)
        };

        AgentEvent::UiRequest {
            request: UiRequest {
                purpose: Some(UiPurpose::Question),
                ..UiRequest::new(id, kind)
            },
        }
    }
}

/// What OpenCode is told for a permission dialog's answer; a dismissed one rejects.
fn permission_reply(outcome: &UiOutcome) -> Result<&'static str> {
    match outcome {
        UiOutcome::Value { value } => PERMISSION_OPTIONS
            .iter()
            .position(|option| option == value)
            .map(|index| PERMISSION_REPLIES[index])
            .ok_or_else(|| BusinessError::InvalidInput(format!("{value} is not one of the options"))),
        UiOutcome::Confirmed { confirmed: true } => Ok("once"),
        _ => Ok("reject"),
    }
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
