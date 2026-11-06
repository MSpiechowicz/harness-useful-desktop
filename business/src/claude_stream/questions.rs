//! `AskUserQuestion` questions as one chat dialog holding all of them, so an answer can still change
//! until the last is submitted. Codex's questions and forms use it too.
//!
//! - Marked as a question, so the UI never shows it as an approval card.
//! - Options keep their descriptions. A typed answer is always taken: Claude Code takes any text as
//!   an answer. A question without options is typed only.
//! - The answers go back together, keyed by the question text, the labels joined with `", "`
//!   ([`question_answers`](super::wire::outbound::question_answers)). A blank typed answer leaves its
//!   question unanswered.

use super::mapper::{Question, QuestionAsk};
use crate::error::Result;
use crate::model::{
    AgentEvent, AskAnswer, AskOption, AskQuestion, UiPurpose, UiRequest, UiRequestKind, answers_by_position,
};
use crate::security::redact_text;

/// The questions of one `AskUserQuestion` call being answered.
#[derive(Debug, Clone)]
pub struct QuestionDialog {
    request_id: String,
    questions: Vec<Question>,
    /// The answers, in question order, once given (empty for an unanswered question).
    answers: Vec<Vec<String>>,
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
        })
    }

    /// The dialog of every question, under the CLI's request id.
    pub fn first(&self) -> AgentEvent {
        AgentEvent::UiRequest {
            request: UiRequest {
                purpose: Some(UiPurpose::Question),
                ..UiRequest::new(
                    self.request_id.clone(),
                    UiRequestKind::Questions {
                        questions: self.questions.iter().enumerate().map(shown).collect(),
                        timeout_ms: None,
                    },
                )
            },
        }
    }

    /// Takes `answers`, one per question in order. A wrong one fails and takes none of them.
    pub fn accept(&mut self, answers: &[AskAnswer]) -> Result<()> {
        let mut accepted = Vec::with_capacity(self.questions.len());

        for (index, answer) in answers_by_position(answers, self.questions.len())? {
            let question = &self.questions[index];

            answer.check(&labels(question), question.multi_select, true)?;

            accepted.push(
                answer
                    .selected_options
                    .iter()
                    .cloned()
                    .chain(answer.typed().map(str::to_owned))
                    .collect(),
            );
        }

        self.answers = accepted;

        Ok(())
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

/// `question` as shown: its text and descriptions redacted, its labels exact (they are the answers).
fn shown((index, question): (usize, &Question)) -> AskQuestion {
    let options = question
        .options
        .iter()
        .filter(|option| !option.label.is_empty())
        .map(|option| AskOption {
            label: option.label.clone(),
            description: option
                .description
                .as_deref()
                .filter(|text| !text.is_empty())
                .map(redact_text),
        })
        .collect();

    AskQuestion::new(
        index.to_string(),
        redact_text(&question.question),
        options,
        question.multi_select,
    )
}
