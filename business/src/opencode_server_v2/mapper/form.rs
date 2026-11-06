//! One OpenCode 2 form (`form.created`) as one chat dialog holding its fields (see the `asks`
//! module), as the OpenCode 1 mapper asks a question request's questions.
//!
//! - The field's text is the question: for the `question` tool's forms its `description` (the
//!   `title` is the question's short header), for other forms its `title`.
//! - A `string` field with options is a single choice, with a typed answer when it takes `custom`
//!   text; without options a typed answer. A `multiselect` is a multi-choice, a `boolean` a
//!   `Yes` / `No` choice, a `number` or `integer` a typed number.
//! - Answers carry the chosen options' values; a blank typed answer leaves the field unanswered.
//! - A form with an `external` field (a sign-in page to open) cannot be asked here.

use std::collections::BTreeMap;

use super::asks::text;
use crate::error::{BusinessError, Result};
use crate::model::{
    AgentEvent, AskAnswer, AskInput, AskOption, AskQuestion, UiPurpose, UiRequest, UiRequestKind, answers_by_position,
};
use crate::opencode_server_v2::wire::{Form, FormField, FormReplyBody, FormValue};
use crate::security::redact_text;

/// The options of a `boolean` field.
const YES_NO: [&str; 2] = ["Yes", "No"];

/// A form being answered.
pub(super) struct FormAsk {
    /// `frm_…`.
    pub(super) id: String,
    fields: Vec<Field>,
}

struct Field {
    key: String,
    text: String,
    kind: FieldKind,
    options: Vec<Choice>,
    /// Typed text is accepted besides the options.
    custom: bool,
}

struct Choice {
    label: String,
    value: String,
    detail: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Text,
    Many,
    Bool,
    Number,
    Integer,
}

impl FormAsk {
    /// The form's fields as a dialog; `None` without a field, or with one the chat cannot ask.
    pub(super) fn new(form: &Form) -> Option<Self> {
        let question = text(&form.metadata, "kind").as_deref() == Some("question");
        let fields: Option<Vec<Field>> = form.fields.iter().map(|field| Field::new(field, question)).collect();
        let fields = fields.filter(|fields| !fields.is_empty())?;

        Some(Self {
            id: form.id.clone(),
            fields,
        })
    }

    /// The reply holding `answers`, one per field in order; a wrong one fails with the dialog still open.
    pub(super) fn reply(&self, answers: &[AskAnswer]) -> Result<FormReplyBody> {
        let mut answer = BTreeMap::new();

        for (index, given) in answers_by_position(answers, self.fields.len())? {
            let field = &self.fields[index];

            if let Some(value) = field.answer(given)? {
                answer.insert(field.key.clone(), value);
            }
        }

        Ok(FormReplyBody { answer })
    }

    /// The dialog of every field, as `id`.
    pub(super) fn dialog(&self, id: &str) -> AgentEvent {
        let questions = self
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| field.shown(index))
            .collect();

        AgentEvent::UiRequest {
            request: UiRequest {
                purpose: Some(UiPurpose::Question),
                ..UiRequest::new(
                    id,
                    UiRequestKind::Questions {
                        questions,
                        timeout_ms: None,
                    },
                )
            },
        }
    }
}

impl Field {
    /// Reads a field, its text redacted; `None` for a kind the chat cannot ask.
    fn new(field: &FormField, question: bool) -> Option<Self> {
        let kind = match field.kind.as_str() {
            "string" | "" => FieldKind::Text,
            "multiselect" => FieldKind::Many,
            "boolean" => FieldKind::Bool,
            "number" => FieldKind::Number,
            "integer" => FieldKind::Integer,
            _ => return None,
        };

        let title = field.title.clone().filter(|title| !title.is_empty());
        let description = field.description.clone().filter(|description| !description.is_empty());
        let text = if question {
            description.or(title)
        } else {
            title.or(description)
        };

        let mut options: Vec<Choice> = Vec::with_capacity(field.options.len());

        for option in &field.options {
            let label = if option.label.is_empty() {
                &option.value
            } else {
                &option.label
            };
            let taken = options.iter().any(|choice| choice.label == *label);

            options.push(Choice {
                // Labels pick the option, so two alike name their values: each row answers with its own.
                label: if taken {
                    format!("{label} ({})", option.value)
                } else {
                    label.clone()
                },
                value: option.value.clone(),
                detail: option
                    .description
                    .as_deref()
                    .filter(|detail| !detail.is_empty())
                    .map(redact_text),
            });
        }

        Some(Self {
            key: field.key.clone(),
            text: redact_text(&text.unwrap_or_else(|| field.key.clone())),
            kind,
            options,
            custom: field.custom,
        })
    }

    fn labels(&self) -> Vec<&str> {
        match self.kind {
            FieldKind::Bool => YES_NO.to_vec(),
            _ => self.options.iter().map(|choice| choice.label.as_str()).collect(),
        }
    }

    /// The field as shown, as question `index` of its form.
    fn shown(&self, index: usize) -> AskQuestion {
        let options = match self.kind {
            FieldKind::Bool => YES_NO
                .iter()
                .map(|label| AskOption {
                    label: (*label).to_owned(),
                    description: None,
                })
                .collect(),
            FieldKind::Number | FieldKind::Integer => Vec::new(),
            _ => self
                .options
                .iter()
                .map(|choice| AskOption {
                    label: choice.label.clone(),
                    description: choice.detail.clone(),
                })
                .collect(),
        };

        AskQuestion {
            custom: self.custom && self.kind != FieldKind::Bool,
            input: match self.kind {
                FieldKind::Number => AskInput::Number,
                FieldKind::Integer => AskInput::Integer,
                _ => AskInput::Text,
            },
            ..AskQuestion::new(
                index.to_string(),
                self.text.clone(),
                options,
                self.kind == FieldKind::Many,
            )
        }
    }

    /// The value `given` answers with, `None` for no answer. Options answer with their values.
    fn answer(&self, given: &AskAnswer) -> Result<Option<FormValue>> {
        given.check(
            &self.labels(),
            self.kind == FieldKind::Many,
            self.custom && self.kind != FieldKind::Bool,
        )?;

        let value_of = |label: &String| {
            self.options
                .iter()
                .find(|choice| choice.label == *label)
                .map_or_else(|| label.clone(), |choice| choice.value.clone())
        };
        let picked = given.selected_options.first();
        let typed = given.typed();

        match self.kind {
            FieldKind::Bool => Ok(picked.map(|label| FormValue::Bool(label == YES_NO[0]))),
            FieldKind::Text => Ok(picked
                .map(value_of)
                .or_else(|| typed.map(str::to_owned))
                .map(FormValue::Text)),
            FieldKind::Many => {
                let chosen: Vec<String> = given
                    .selected_options
                    .iter()
                    .map(value_of)
                    .chain(typed.map(str::to_owned))
                    .collect();

                Ok((!chosen.is_empty()).then_some(FormValue::Many(chosen)))
            }
            FieldKind::Number | FieldKind::Integer => typed.map_or(Ok(None), |text| self.number(text)),
        }
    }

    fn number(&self, typed: &str) -> Result<Option<FormValue>> {
        let invalid = || BusinessError::InvalidInput(format!("{typed} is not a number"));

        let number = if self.kind == FieldKind::Integer {
            typed
                .parse::<i64>()
                .map(|number| number as f64)
                .map_err(|_| invalid())?
        } else {
            typed
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .ok_or_else(invalid)?
        };

        Ok(Some(FormValue::Number(number)))
    }
}
