//! One OpenCode 2 form (`form.created`) as chat dialogs, one per field, in order (see the `asks`
//! module), as the OpenCode 1 mapper asks a question request's questions.
//!
//! - The field's text is the question: for the `question` tool's forms its `description` (the
//!   `title` is the question's short header), for other forms its `title`. Several fields are
//!   titled `Question <n> of <count>: <text>`.
//! - A `string` field with options is a `select` of their labels, with a trailing [`OWN_ANSWER`]
//!   option when it takes `custom` text (choosing it asks for the text in an `input`); without
//!   options an `input`. A `multiselect` is an `input` of comma-separated choices (typed ones
//!   too), a `boolean` a `Yes` / `No` `select`, a `number` or `integer` an `input`.
//! - Answers carry the chosen options' values; a blank typed answer leaves the field unanswered.
//! - A form with an `external` field (a sign-in page to open) cannot be asked here.

use std::collections::BTreeMap;

use super::asks::text;
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, UiPurpose, UiRequest, UiRequestKind};
use crate::opencode_server_v2::wire::{Form, FormField, FormReplyBody, FormValue};
use crate::security::redact_text;

/// The option of a single-choice field that asks for a typed answer instead.
pub(super) const OWN_ANSWER: &str = "Type your own answer…";

/// The options of a `boolean` field.
const YES_NO: [&str; 2] = ["Yes", "No"];

/// A form being answered, one field at a time.
pub(super) struct FormAsk {
    /// `frm_…`.
    pub(super) id: String,
    fields: Vec<Field>,
    /// The answers so far, by field key.
    answers: BTreeMap<String, FormValue>,
    /// The field being asked.
    step: usize,
    /// The current field was answered with [`OWN_ANSWER`]: its typed answer comes next.
    typing: bool,
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
    /// The form's fields as dialogs; `None` without a field, or with one the chat cannot ask.
    pub(super) fn new(form: &Form) -> Option<Self> {
        let question = text(&form.metadata, "kind").as_deref() == Some("question");
        let fields: Option<Vec<Field>> = form.fields.iter().map(|field| Field::new(field, question)).collect();
        let fields = fields.filter(|fields| !fields.is_empty())?;

        Some(Self {
            id: form.id.clone(),
            fields,
            answers: BTreeMap::new(),
            step: 0,
            typing: false,
        })
    }

    /// Takes `value` as the answer to the current field. Returns the id of the dialog that comes
    /// next (the typed answer, or the next field), `None` once every field is answered.
    pub(super) fn answer(&mut self, value: &str) -> Result<Option<String>> {
        let current = self.current()?;
        let step = self.step + 1;

        if !self.typing && current.is_own_answer(value) {
            self.typing = true;

            return Ok(Some(format!("{}:{step}:own", self.id)));
        }

        let answer = if self.typing {
            typed(value).map(FormValue::Text)
        } else {
            current.answer(value)?
        };
        let key = current.key.clone();

        if let Some(answer) = answer {
            self.answers.insert(key, answer);
        }

        self.typing = false;
        self.step += 1;

        if self.step < self.fields.len() {
            return Ok(Some(format!("{}:{}", self.id, self.step + 1)));
        }

        Ok(None)
    }

    /// The answers, for the reply.
    pub(super) fn reply(&mut self) -> FormReplyBody {
        FormReplyBody {
            answer: std::mem::take(&mut self.answers),
        }
    }

    /// The dialog of the current field (or of its typed answer), as `id`.
    pub(super) fn dialog(&self, id: &str) -> AgentEvent {
        let current = &self.fields[self.step.min(self.fields.len() - 1)];

        let title = match self.fields.len() {
            1 => current.text.clone(),
            count => format!("Question {} of {count}: {}", self.step + 1, current.text),
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

    fn current(&self) -> Result<&Field> {
        self.fields
            .get(self.step)
            .ok_or_else(|| BusinessError::Internal("form without a current field".to_owned()))
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

        let options = field
            .options
            .iter()
            .map(|option| Choice {
                label: if option.label.is_empty() {
                    option.value.clone()
                } else {
                    option.label.clone()
                },
                value: option.value.clone(),
                detail: option.description.clone().filter(|detail| !detail.is_empty()),
            })
            .collect();

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

    /// One choice from a few options (or yes / no) is a `select`, anything else an `input`.
    fn is_select(&self) -> bool {
        self.kind == FieldKind::Bool || (self.kind == FieldKind::Text && !self.options.is_empty())
    }

    /// A single choice that also takes a typed answer, offered as [`OWN_ANSWER`].
    fn offers_own_answer(&self) -> bool {
        self.kind == FieldKind::Text && self.is_select() && self.custom
    }

    /// True when `value` is the [`OWN_ANSWER`] option rather than an option of the field.
    fn is_own_answer(&self, value: &str) -> bool {
        self.offers_own_answer() && value == OWN_ANSWER && !self.labels().contains(&value)
    }

    fn kind(&self, title: String) -> UiRequestKind {
        if self.is_select() {
            let mut options: Vec<String> = self.labels().into_iter().map(str::to_owned).collect();
            let mut details: Vec<Option<String>> = match self.kind {
                FieldKind::Bool => vec![None, None],
                _ => self.options.iter().map(|choice| choice.detail.clone()).collect(),
            };

            if self.offers_own_answer() {
                options.push(OWN_ANSWER.to_owned());
                details.push(None);
            }

            return UiRequestKind::Select {
                title,
                options,
                details,
                timeout_ms: None,
            };
        }

        if self.kind != FieldKind::Many || self.options.is_empty() {
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

    /// The answer `value` gives; `None` for a blank typed answer.
    fn answer(&self, value: &str) -> Result<Option<FormValue>> {
        let invalid = || BusinessError::InvalidInput(format!("{value} is not one of the options"));

        match self.kind {
            FieldKind::Bool => {
                let position = YES_NO.iter().position(|option| *option == value).ok_or_else(invalid)?;

                Ok(Some(FormValue::Bool(position == 0)))
            }
            FieldKind::Text if self.is_select() => {
                let choice = self
                    .options
                    .iter()
                    .find(|choice| choice.label == value)
                    .ok_or_else(invalid)?;

                Ok(Some(FormValue::Text(choice.value.clone())))
            }
            FieldKind::Text => Ok(typed(value).map(FormValue::Text)),
            FieldKind::Many => Ok(self.choices(value)),
            FieldKind::Number | FieldKind::Integer => self.number(value),
        }
    }

    /// The comma-separated choices of a `multiselect`, as option values (typed ones as written).
    fn choices(&self, value: &str) -> Option<FormValue> {
        let chosen: Vec<String> = value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                let known = self
                    .options
                    .iter()
                    .find(|choice| choice.label.eq_ignore_ascii_case(part) || choice.value.eq_ignore_ascii_case(part));

                known.map_or_else(|| part.to_owned(), |choice| choice.value.clone())
            })
            .collect();

        (!chosen.is_empty()).then_some(FormValue::Many(chosen))
    }

    fn number(&self, value: &str) -> Result<Option<FormValue>> {
        let Some(typed) = typed(value) else {
            return Ok(None);
        };

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

/// A typed answer: its trimmed text, or no answer when it is blank.
fn typed(value: &str) -> Option<String> {
    let answer = value.trim();

    (!answer.is_empty()).then(|| answer.to_owned())
}
