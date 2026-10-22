//! `AskUserQuestion` questions as chat dialogs, one per question, in order, shaped as the
//! OpenCode chats ask theirs (Verified fact 12).
//!
//! - Marked as questions, so the UI never shows one as an approval card. Several questions are
//!   titled `Question <n> of <count>: <text>`.
//! - A single choice is a `select` of the option labels (their descriptions as details) and a
//!   trailing [`OWN_ANSWER`], which asks for a typed answer in an `input` (Claude Code takes any
//!   text as an answer). Several choices are an `input` of comma-separated labels (typed ones
//!   too), a question without options an `input`.
//! - The answers go back together after the last one, keyed by the question text, the labels
//!   joined with `", "` ([`question_answers`](super::wire::outbound::question_answers)). A blank
//!   typed answer leaves its question unanswered.

use super::mapper::{Question, QuestionAsk};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, UiPurpose, UiRequest, UiRequestKind};
use crate::security::redact_text;

/// The option of a single choice that asks for a typed answer instead.
pub const OWN_ANSWER: &str = "Type your own answer…";

/// The questions of one `AskUserQuestion` call being answered.
#[derive(Debug, Clone)]
pub struct QuestionDialog {
    request_id: String,
    questions: Vec<Question>,
    /// The answers so far, in question order (empty for an unanswered question).
    answers: Vec<Vec<String>>,
    /// The current question was answered with [`OWN_ANSWER`]: its typed answer comes next.
    typing: bool,
}

impl QuestionDialog {
    /// The questions of `ask`; `None` when it asks none.
    pub fn new(ask: &QuestionAsk) -> Option<Self> {
        let questions: Vec<Question> = ask
            .questions
            .iter()
            .filter(|question| !question.question.is_empty())
            .cloned()
            .collect();

        (!questions.is_empty()).then(|| Self {
            request_id: ask.request_id.clone(),
            questions,
            answers: Vec::new(),
            typing: false,
        })
    }

    /// The dialog of the first question, under the CLI's request id.
    pub fn first(&self) -> AgentEvent {
        self.dialog(&self.request_id)
    }

    /// Takes `value` as the answer to the current question. Returns the next dialog (the typed
    /// answer, or the next question) with its id, `None` once every question is answered.
    pub fn answer(&mut self, value: &str) -> Result<Option<(String, AgentEvent)>> {
        let current = self.current()?;
        let step = self.answers.len() + 1;

        if !self.typing && is_single(current) && value == OWN_ANSWER && !labels(current).contains(&value) {
            self.typing = true;
            let id = format!("{}:{step}:own", self.request_id);

            return Ok(Some((id.clone(), self.dialog(&id))));
        }

        let answer = if self.typing {
            typed(value)
        } else {
            picked(current, value)?
        };
        self.typing = false;
        self.answers.push(answer);

        if self.answers.len() < self.questions.len() {
            let id = format!("{}:{}", self.request_id, self.answers.len() + 1);

            return Ok(Some((id.clone(), self.dialog(&id))));
        }

        Ok(None)
    }

    /// Each answered question's text with its labels.
    pub fn answers(&self) -> Vec<(String, Vec<String>)> {
        self.questions
            .iter()
            .zip(&self.answers)
            .filter(|(_, labels)| !labels.is_empty())
            .map(|(question, labels)| (question.question.clone(), labels.clone()))
            .collect()
    }

    fn current(&self) -> Result<&Question> {
        self.questions
            .get(self.answers.len())
            .ok_or_else(|| BusinessError::Internal("questions without a current one".to_owned()))
    }

    /// The dialog of the current question (or of its typed answer), as `id`.
    fn dialog(&self, id: &str) -> AgentEvent {
        let step = self.answers.len().min(self.questions.len() - 1);
        let current = &self.questions[step];
        let text = redact_text(&current.question);

        let title = match self.questions.len() {
            1 => text,
            count => format!("Question {} of {count}: {text}", step + 1),
        };

        let kind = if self.typing {
            input(title, None)
        } else {
            kind(current, title)
        };

        AgentEvent::UiRequest {
            request: UiRequest {
                purpose: Some(UiPurpose::Question),
                ..UiRequest::new(id, kind)
            },
        }
    }
}

/// The labels of `question`'s options.
fn labels(question: &Question) -> Vec<&str> {
    question
        .options
        .iter()
        .map(|option| option.label.as_str())
        .filter(|label| !label.is_empty())
        .collect()
}

/// One choice from options: a `select`.
fn is_single(question: &Question) -> bool {
    !question.multi_select && !labels(question).is_empty()
}

fn kind(question: &Question, title: String) -> UiRequestKind {
    let labels = labels(question);

    if is_single(question) {
        let mut options: Vec<String> = labels.iter().map(|label| (*label).to_owned()).collect();
        let mut details: Vec<Option<String>> = question
            .options
            .iter()
            .filter(|option| !option.label.is_empty())
            .map(|option| {
                option
                    .description
                    .as_deref()
                    .filter(|text| !text.is_empty())
                    .map(redact_text)
            })
            .collect();

        options.push(OWN_ANSWER.to_owned());
        details.push(None);

        return UiRequestKind::Select {
            title,
            options,
            details,
            timeout_ms: None,
        };
    }

    if labels.is_empty() {
        return input(title, None);
    }

    let listed = labels.join(", ");

    input(format!("{title} (any of {listed}, separated by commas)"), Some(listed))
}

fn input(title: String, placeholder: Option<String>) -> UiRequestKind {
    UiRequestKind::Input {
        title,
        placeholder,
        timeout_ms: None,
    }
}

/// The labels `value` answers `question` with.
fn picked(question: &Question, value: &str) -> Result<Vec<String>> {
    let labels = labels(question);

    if is_single(question) {
        let label = labels
            .into_iter()
            .find(|label| *label == value)
            .ok_or_else(|| BusinessError::InvalidInput(format!("{value} is not one of the options")))?;

        return Ok(vec![label.to_owned()]);
    }

    if labels.is_empty() {
        return Ok(typed(value));
    }

    let chosen = value.split(',').map(str::trim).filter(|part| !part.is_empty());

    Ok(chosen
        .map(|part| {
            let known = labels.iter().find(|label| label.eq_ignore_ascii_case(part));
            known.map_or_else(|| part.to_owned(), |label| (*label).to_owned())
        })
        .collect())
}

/// A typed answer: its trimmed text, or no answer when it is blank.
fn typed(value: &str) -> Vec<String> {
    let answer = value.trim();

    (!answer.is_empty()).then(|| answer.to_owned()).into_iter().collect()
}
