//! Codex's questions and an MCP server's form as the chat's question dialog
//! ([`QuestionDialog`]: one dialog per question, in order, the answers together after the last).
//!
//! - **The model's questions** (`item/tool/requestUserInput`, Plan mode only): each question with
//!   its options; the answers go back keyed by the question's id. A secret question is asked like
//!   the others (the dialog does not hide what is typed yet).
//! - **A form** (`mcpServer/elicitation/request` in `form` mode): one question per property of
//!   its schema, a choice for an `enum` (`Yes` / `No` for a boolean), else a typed answer read as
//!   the property's type (a number or integer that does not parse stays text, for the server to
//!   refuse). A form without properties asks `Accept` or `Decline`.

use serde_json::{Map, Value};

use crate::claude_stream::mapper::{Question, QuestionAsk, QuestionOption};
use crate::claude_stream::questions::QuestionDialog;
use crate::codex_rpc::wire::requests::{Elicitation, UserInputRequest};
use crate::security::redact_text;

/// The choices of a boolean property.
const YES: &str = "Yes";
const NO: &str = "No";

/// The choices of a form without properties.
pub(super) const ACCEPT: &str = "Accept";
const DECLINE: &str = "Decline";

/// The questions of an ask as asked: the dialog and the id (or property) behind each question.
pub(super) struct Asked {
    pub(super) dialog: QuestionDialog,
    /// Per question in order: its id (a property's name) and the text it was asked with.
    keys: Vec<(String, String)>,
}

impl Asked {
    /// The labels answering each question, keyed by its id; unanswered ones left out.
    pub(super) fn answers(&self) -> Vec<(String, Vec<String>)> {
        let mut answered = self.dialog.answers();
        let mut keyed = Vec::new();

        for (key, text) in &self.keys {
            if let Some(position) = answered.iter().position(|(asked, _)| asked == text) {
                let (_, labels) = answered.remove(position);
                keyed.push((key.clone(), labels));
            }
        }

        keyed
    }
}

/// The model's questions of request `dialog`; `None` when it asks none.
pub(super) fn questions(dialog: &str, request: &UserInputRequest) -> Option<Asked> {
    let asked: Vec<(String, Question)> = request
        .questions
        .iter()
        .map(|question| {
            let text = [&question.question, &question.header, &question.id]
                .into_iter()
                .find(|text| !text.trim().is_empty())
                .cloned()
                .unwrap_or_default();

            let options = question
                .options
                .iter()
                .map(|option| QuestionOption {
                    label: option.label.clone(),
                    description: Some(option.description.clone()).filter(|text| !text.is_empty()),
                })
                .collect();

            (question.id.clone(), question_of(text, options))
        })
        .collect();

    ask(dialog, asked)
}

/// What a form asks for, kept to read its answers.
pub(super) struct Form {
    fields: Vec<(String, Kind)>,
}

/// How a property's answer is read.
#[derive(Clone, Copy)]
enum Kind {
    Text,
    Number,
    Integer,
    Boolean,
}

/// The form of `elicitation` as the questions of request `dialog`.
pub(super) fn form(dialog: &str, elicitation: &Elicitation) -> Option<(Asked, Form)> {
    let server = redact_text(&elicitation.server_name);
    let message = elicitation.message.trim();
    let properties = elicitation
        .requested_schema
        .as_ref()
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object);

    let Some(properties) = properties.filter(|properties| !properties.is_empty()) else {
        let options = [ACCEPT, DECLINE].map(|label| QuestionOption {
            label: label.to_owned(),
            description: None,
        });
        let question = question_of(format!("{server}: {message}"), options.to_vec());

        return Some((
            ask(dialog, vec![(String::new(), question)])?,
            Form { fields: Vec::new() },
        ));
    };

    let mut fields = Vec::new();
    let mut asked = Vec::new();

    for (name, property) in properties {
        let (kind, options) = field(property);
        let label = ["title", "description"]
            .into_iter()
            .find_map(|key| {
                property
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
            })
            .unwrap_or(name);

        fields.push((name.clone(), kind));
        asked.push((
            name.clone(),
            question_of(format!("{server} asks for {label}. {message}"), options),
        ));
    }

    Some((ask(dialog, asked)?, Form { fields }))
}

impl Form {
    /// True for a form without properties (`Accept` / `Decline`).
    pub(super) fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// The form's values from `answers` (per property name), read as each property's type.
    pub(super) fn content(&self, answers: &[(String, Vec<String>)]) -> Value {
        let mut content = Map::new();

        for (name, kind) in &self.fields {
            let Some((_, labels)) = answers.iter().find(|(key, _)| key == name) else {
                continue;
            };

            let text = labels.join(", ");
            content.insert(name.clone(), value(*kind, text));
        }

        Value::Object(content)
    }
}

/// How `property` is answered: its kind and its choices.
fn field(property: &Value) -> (Kind, Vec<QuestionOption>) {
    let choice = |label: &str| QuestionOption {
        label: label.to_owned(),
        description: None,
    };

    if let Some(values) = property.get("enum").and_then(Value::as_array) {
        let labels = values.iter().filter_map(Value::as_str).map(choice).collect();
        return (Kind::Text, labels);
    }

    match property.get("type").and_then(Value::as_str) {
        Some("boolean") => (Kind::Boolean, vec![choice(YES), choice(NO)]),
        Some("number") => (Kind::Number, Vec::new()),
        Some("integer") => (Kind::Integer, Vec::new()),
        _ => (Kind::Text, Vec::new()),
    }
}

/// `text` read as `kind`; text when it does not parse.
fn value(kind: Kind, text: String) -> Value {
    let parsed = match kind {
        Kind::Text => None,
        Kind::Number => text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number),
        Kind::Integer => text.parse::<i64>().ok().map(Value::from),
        Kind::Boolean => match text.as_str() {
            YES | "true" => Some(Value::Bool(true)),
            NO | "false" => Some(Value::Bool(false)),
            _ => None,
        },
    };

    parsed.unwrap_or(Value::String(text))
}

/// A question asking `text` (redacted, as Codex and an MCP server may echo a secret) with
/// `options`, whose descriptions are redacted too (their labels are the answers).
fn question_of(text: String, options: Vec<QuestionOption>) -> Question {
    let options = options
        .into_iter()
        .map(|option| QuestionOption {
            description: option.description.map(|text| redact_text(&text)),
            ..option
        })
        .collect();

    Question {
        question: redact_text(&text),
        options,
        ..Question::default()
    }
}

/// The dialog of `asked` (key and question) under `dialog`; `None` when nothing is asked.
fn ask(dialog: &str, asked: Vec<(String, Question)>) -> Option<Asked> {
    let asked: Vec<(String, Question)> = asked
        .into_iter()
        .filter(|(_, question)| !question.question.trim().is_empty())
        .collect();

    let keys = asked
        .iter()
        .map(|(key, question)| (key.clone(), question.question.clone()))
        .collect();

    let request = QuestionAsk {
        request_id: dialog.to_owned(),
        tool_call_id: String::new(),
        input: Value::Null,
        questions: asked.into_iter().map(|(_, question)| question).collect(),
    };

    Some(Asked {
        dialog: QuestionDialog::new(&request)?,
        keys,
    })
}
