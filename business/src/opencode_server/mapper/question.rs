//! One question of an OpenCode `question.asked` request (see the `asks` module).

use serde_json::Value;

use super::message::text;
use crate::error::Result;
use crate::model::{AskAnswer, AskOption, AskQuestion};
use crate::security::redact_text;

/// One question.
pub(super) struct Question {
    text: String,
    options: Vec<(String, Option<String>)>,
    multiple: bool,
    /// A typed answer is allowed (OpenCode's default).
    custom: bool,
}

impl Question {
    /// The question as shown, as question `index` of its request.
    pub(super) fn shown(&self, index: usize) -> AskQuestion {
        let options = self
            .options
            .iter()
            .map(|(label, description)| AskOption {
                label: label.clone(),
                description: description.clone(),
            })
            .collect();

        AskQuestion {
            custom: self.custom,
            ..AskQuestion::new(index.to_string(), self.text.clone(), options, self.multiple)
        }
    }

    /// The labels (and typed text) `answer` gives.
    pub(super) fn answer(&self, answer: &AskAnswer) -> Result<Vec<String>> {
        let labels: Vec<&str> = self.options.iter().map(|(label, _)| label.as_str()).collect();

        answer.check(&labels, self.multiple, self.custom)?;

        Ok(answer
            .selected_options
            .iter()
            .cloned()
            .chain(answer.typed().map(str::to_owned))
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
        .filter_map(|option| {
            let description = text(option, "description").map(|detail| redact_text(&detail));

            Some((text(option, "label")?, description))
        })
        .collect();

    Question {
        text: redact_text(&text(value, "question").unwrap_or_default()),
        options,
        multiple: value.get("multiple").and_then(Value::as_bool).unwrap_or(false),
        custom: value.get("custom").and_then(Value::as_bool).unwrap_or(true),
    }
}
