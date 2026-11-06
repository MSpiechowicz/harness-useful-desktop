//! Helpers of the question tests: the answers to the one dialog holding all of an agent's
//! questions, and a way to read the questions it shows.
#![allow(dead_code)]

use harness_business::model::{AgentEvent, AskAnswer, AskQuestion, UiOutcome, UiRequest, UiRequestKind, UiResponse};

/// The answer to question `index` that picks `labels`.
pub fn pick(index: usize, labels: &[&str]) -> AskAnswer {
    AskAnswer {
        id: index.to_string(),
        selected_options: labels.iter().map(|label| (*label).to_owned()).collect(),
        custom_input: None,
    }
}

/// The answer to question `index` that types `text`.
pub fn typed(index: usize, text: &str) -> AskAnswer {
    AskAnswer {
        custom_input: Some(text.to_owned()),
        ..pick(index, &[])
    }
}

/// The answer to question `index` that picks `labels` and types `text` besides.
pub fn pick_and_type(index: usize, labels: &[&str], text: &str) -> AskAnswer {
    AskAnswer {
        custom_input: Some(text.to_owned()),
        ..pick(index, labels)
    }
}

/// The outcome of answering every question with `answers`.
pub fn outcome(answers: Vec<AskAnswer>) -> UiOutcome {
    UiOutcome::Answers { answers }
}

/// The answers to the questions of dialog `id`.
pub fn answers(id: &str, answers: Vec<AskAnswer>) -> UiResponse {
    UiResponse {
        id: id.to_owned(),
        outcome: outcome(answers),
    }
}

/// The questions of a dialog; panics for any other kind.
pub fn questions_of(request: &UiRequest) -> &[AskQuestion] {
    match &request.kind {
        UiRequestKind::Questions { questions, .. } => questions,
        other => panic!("not a questions dialog: {other:?}"),
    }
}

/// The questions of the dialog a `uiRequest` event shows.
pub fn event_questions(event: &AgentEvent) -> &[AskQuestion] {
    match event {
        AgentEvent::UiRequest { request } => questions_of(request),
        other => panic!("not a dialog: {other:?}"),
    }
}

/// The labels of `question`'s options.
pub fn labels(question: &AskQuestion) -> Vec<&str> {
    question.options.iter().map(|option| option.label.as_str()).collect()
}
