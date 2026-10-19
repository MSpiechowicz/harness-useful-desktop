//! OpenCode's permission asks and questions as chat dialogs, and the answers to them.
//!
//! - **Permission** (`permission.asked`): a `select` with `Allow once`, `Always allow` and
//!   `Reject` (the UI shows it as an approval card), titled `Allow <tool>: <subject>?` (`for a
//!   subagent` when a subagent session asks). Answers: `once`, `always`, `reject`; a dismissed
//!   dialog rejects.
//! - **Questions** (`question.asked`) are shown one dialog per question, in order: a `select` for a
//!   single choice (labels that would read as an approval are numbered, so it stays a question),
//!   an `input` for several choices (comma-separated) or a free answer. The answers go out
//!   together after the last one; a dismissed dialog rejects the whole request.
//! - `permission.replied` (answered elsewhere, or rejected along with another ask of the session),
//!   `question.replied` and `question.rejected` close the dialog (`uiCancel`), as does the end of
//!   the turn (OpenCode drops its open asks when a turn stops, without an event).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::message::text;
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, UiOutcome, UiRequest, UiRequestKind, UiResponse};
use crate::security::redact_text;

/// The answers of a permission dialog, in order.
pub const PERMISSION_OPTIONS: [&str; 3] = ["Allow once", "Always allow", "Reject"];

/// What OpenCode is told for each of [`PERMISSION_OPTIONS`].
const PERMISSION_REPLIES: [&str; 3] = ["once", "always", "reject"];

/// Longest subject shown in a permission title, in characters.
const MAX_SUBJECT: usize = 200;

/// Words that make a `select` read as an approval in the UI (`ui/src/lib/chat/approval.ts`).
const APPROVE_WORDS: [&str; 4] = ["approve", "allow", "accept", "yes"];
const DENY_WORDS: [&str; 5] = ["deny", "reject", "decline", "block", "no"];

/// Most options the UI shows as an approval card.
const MAX_APPROVAL_OPTIONS: usize = 4;

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
}

struct Question {
    text: String,
    options: Vec<(String, Option<String>)>,
    multiple: bool,
}

impl Asks {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            open: Vec::new(),
        }
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
            [only] if only == "*" => Some(format!("Allows every {permission} call until this chat ends")),
            patterns => Some(format!("Also allows {} until this chat ends", patterns.join(", "))),
        };

        out.push(AgentEvent::UiRequest {
            request: UiRequest {
                id: id.clone(),
                kind: UiRequestKind::Select {
                    title,
                    options: PERMISSION_OPTIONS.map(str::to_owned).to_vec(),
                    details: vec![None, always_detail.map(|detail| redact_text(&detail)), None],
                    timeout_ms: None,
                },
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
                    let answer = question.current()?.answer(value)?;
                    question.answers.push(answer);

                    if question.answers.len() < question.questions.len() {
                        let id = format!("{}:{}", question.id, question.answers.len() + 1);
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

    /// What a permission ask is about, as one redacted line: the command, the file (relative to the
    /// session's directory), the URL, or the patterns.
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

        Some(redact_text(&truncate(&line, MAX_SUBJECT)))
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

    /// The dialog of the current question, as `id`.
    fn dialog(&self, id: &str) -> AgentEvent {
        let step = self.answers.len();
        let current = &self.questions[step];

        let title = match self.questions.len() {
            1 => current.text.clone(),
            count => format!("Question {} of {count}: {}", step + 1, current.text),
        };

        AgentEvent::UiRequest {
            request: UiRequest {
                id: id.to_owned(),
                kind: current.kind(title),
            },
        }
    }
}

impl Question {
    fn labels(&self) -> Vec<&str> {
        self.options.iter().map(|(label, _)| label.as_str()).collect()
    }

    /// One choice from a few options is a `select`, anything else an `input`.
    fn is_select(&self) -> bool {
        !self.multiple && !self.options.is_empty()
    }

    /// The labels as shown: numbered when they would read as an approval.
    fn shown_labels(&self) -> Vec<String> {
        let labels = self.labels();
        let looks_like_approval = labels.len() <= MAX_APPROVAL_OPTIONS
            && labels.iter().any(|label| starts_with_word(label, &APPROVE_WORDS))
            && labels.iter().any(|label| starts_with_word(label, &DENY_WORDS));

        labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                if looks_like_approval {
                    format!("{}. {label}", index + 1)
                } else {
                    (*label).to_owned()
                }
            })
            .collect()
    }

    fn kind(&self, title: String) -> UiRequestKind {
        if self.is_select() {
            return UiRequestKind::Select {
                title,
                options: self.shown_labels(),
                details: self.options.iter().map(|(_, detail)| detail.clone()).collect(),
                timeout_ms: None,
            };
        }

        if self.options.is_empty() {
            return UiRequestKind::Input {
                title,
                placeholder: None,
                timeout_ms: None,
            };
        }

        let labels = self.labels().join(", ");

        UiRequestKind::Input {
            title: format!("{title} (any of {labels}, separated by commas)"),
            placeholder: Some(labels),
            timeout_ms: None,
        }
    }

    /// The labels answered with `value`.
    fn answer(&self, value: &str) -> Result<Vec<String>> {
        if self.is_select() {
            let shown = self.shown_labels();
            let index = shown
                .iter()
                .position(|label| label == value)
                .ok_or_else(|| BusinessError::InvalidInput(format!("{value} is not one of the options")))?;

            return Ok(vec![self.options[index].0.clone()]);
        }

        if self.options.is_empty() {
            return Ok(vec![value.trim().to_owned()]
                .into_iter()
                .filter(|answer| !answer.is_empty())
                .collect());
        }

        let chosen = value.split(',').map(str::trim).filter(|part| !part.is_empty());
        let labels = self.labels();

        Ok(chosen
            .map(|part| {
                let known = labels.iter().find(|label| label.eq_ignore_ascii_case(part));
                known.map_or_else(|| part.to_owned(), |label| (*label).to_owned())
            })
            .collect())
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

fn question(value: &Value) -> Question {
    let options = value
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| Some((text(option, "label")?, text(option, "description"))))
        .collect();

    Question {
        text: redact_text(&text(value, "question").unwrap_or_default()),
        options,
        multiple: value.get("multiple").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// True when `label` starts with one of `words` as a whole word (any case).
fn starts_with_word(label: &str, words: &[&str]) -> bool {
    let lower = label.to_lowercase();

    words.iter().any(|word| {
        lower
            .strip_prefix(word)
            .is_some_and(|rest| !rest.starts_with(|next: char| next.is_alphanumeric() || next == '_'))
    })
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

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }

    let cut: String = text.chars().take(max).collect();
    format!("{cut}…")
}
