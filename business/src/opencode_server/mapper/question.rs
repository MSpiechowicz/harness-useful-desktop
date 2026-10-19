//! One question of an OpenCode `question.asked` request as a chat dialog (see the `asks` module):
//! a `select` for a single choice (with a typed-answer option when the question takes one), an
//! `input` for several choices (comma-separated) or a free answer.

use serde_json::Value;

use super::message::text;
use crate::error::{BusinessError, Result};
use crate::model::UiRequestKind;
use crate::security::redact_text;

/// The option of a single-choice question that asks for a typed answer instead.
pub(super) const OWN_ANSWER: &str = "Type your own answer…";

/// One question.
pub(super) struct Question {
    pub(super) text: String,
    options: Vec<(String, Option<String>)>,
    multiple: bool,
    /// A typed answer is allowed (OpenCode's default).
    custom: bool,
}

impl Question {
    fn labels(&self) -> Vec<&str> {
        self.options.iter().map(|(label, _)| label.as_str()).collect()
    }

    /// One choice from a few options is a `select`, anything else an `input`.
    fn is_select(&self) -> bool {
        !self.multiple && !self.options.is_empty()
    }

    /// A single choice that also takes a typed answer, offered as [`OWN_ANSWER`].
    fn offers_own_answer(&self) -> bool {
        self.is_select() && self.custom
    }

    /// True when `value` is the [`OWN_ANSWER`] option rather than an option of the question.
    pub(super) fn is_own_answer(&self, value: &str) -> bool {
        self.offers_own_answer() && value == OWN_ANSWER && !self.labels().contains(&value)
    }

    pub(super) fn kind(&self, title: String) -> UiRequestKind {
        if self.is_select() {
            let mut options: Vec<String> = self.labels().into_iter().map(str::to_owned).collect();
            let mut details: Vec<Option<String>> = self.options.iter().map(|(_, detail)| detail.clone()).collect();

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
    pub(super) fn answer(&self, value: &str) -> Result<Vec<String>> {
        if self.is_select() {
            let label = self
                .labels()
                .into_iter()
                .find(|label| *label == value)
                .ok_or_else(|| BusinessError::InvalidInput(format!("{value} is not one of the options")))?;

            return Ok(vec![label.to_owned()]);
        }

        if self.options.is_empty() {
            return Ok(typed(value));
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

/// Reads a question of a `question.asked` request, its text redacted.
pub(super) fn question(value: &Value) -> Question {
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
        custom: value.get("custom").and_then(Value::as_bool).unwrap_or(true),
    }
}

/// A typed answer: its trimmed text, or no answer when it is blank.
pub(super) fn typed(value: &str) -> Vec<String> {
    let answer = value.trim();

    (!answer.is_empty()).then(|| answer.to_owned()).into_iter().collect()
}
