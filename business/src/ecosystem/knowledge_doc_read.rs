//! Documents as `harness-knowledge docs list|get` print them, read into [`KnowledgeDocument`]s.

use serde_json::Value;

use crate::error::{BusinessError, Result};
use crate::model::KnowledgeDocument;

/// The documents under `name` in `data`.
pub(super) fn read_documents(data: &Value, name: &str, command: &str) -> Result<Vec<KnowledgeDocument>> {
    let unreadable = || {
        BusinessError::Protocol(format!(
            "harness-knowledge {command} answered without a usable `{name}`"
        ))
    };

    data.get(name)
        .and_then(Value::as_array)
        .ok_or_else(unreadable)?
        .iter()
        .map(|document| serde_json::from_value(renamed(document.clone())).map_err(|_| unreadable()))
        .collect()
}

/// The document under `name` in `data`.
pub(super) fn read_document(data: &Value, name: &str, command: &str) -> Result<KnowledgeDocument> {
    data.get(name)
        .cloned()
        .map(renamed)
        .and_then(|document| serde_json::from_value(document).ok())
        .ok_or_else(|| {
            BusinessError::Protocol(format!(
                "harness-knowledge {command} answered without a usable `{name}`"
            ))
        })
}

/// `document` with the review's last request for changes under its name here, `lastChange`: a
/// server before the rename of `send-back` to `request-changes` prints `lastSentBack`.
pub(super) fn renamed(mut document: Value) -> Value {
    if let Value::Object(fields) = &mut document
        && !fields.contains_key("lastChange")
        && let Some(found) = fields
            .remove("lastSentBack")
            .or_else(|| fields.remove("lastChangesRequested"))
    {
        fields.insert("lastChange".to_owned(), found);
    }

    document
}
