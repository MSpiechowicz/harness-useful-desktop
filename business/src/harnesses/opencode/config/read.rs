//! How an OpenCode settings file is read: JSON with comments in both `*.json` and `*.jsonc`, with
//! `{env:NAME}` values shown by name and never resolved.

use std::path::Path;

use serde_json::Value;

use crate::model::ConfigFormat;
use crate::pi_family::{Document, read_document};

/// Reads an OpenCode settings file: JSON with comments, `{env:NAME}` values shown by name.
pub(in crate::harnesses::opencode) fn read_config(path: &Path) -> Document {
    match read_document(path, ConfigFormat::Jsonc) {
        Document::Parsed(value) => Document::Parsed(env_names(value)),
        other => other,
    }
}

/// `jsonc` for `*.jsonc` files, else `json` (OpenCode accepts comments in both).
pub(in crate::harnesses::opencode) fn format_of(path: &Path) -> ConfigFormat {
    match path.extension() {
        Some(extension) if extension == "jsonc" => ConfigFormat::Jsonc,
        _ => ConfigFormat::Json,
    }
}

/// Rewrites every string that is exactly `{env:NAME}` to `${NAME}`, which the redactor shows as
/// a reference to `NAME` without reading it. Other text, `{file:...}` included, is left as written.
fn env_names(value: Value) -> Value {
    match value {
        Value::String(text) => match env_placeholder(&text) {
            Some(name) => Value::String(format!("${{{name}}}")),
            None => Value::String(text),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(env_names).collect()),
        Value::Object(entries) => {
            Value::Object(entries.into_iter().map(|(key, item)| (key, env_names(item))).collect())
        }
        other => other,
    }
}

/// `NAME` when `text` is exactly `{env:NAME}` and `NAME` is a plain variable name.
fn env_placeholder(text: &str) -> Option<&str> {
    let name = text.strip_prefix("{env:")?.strip_suffix('}')?;
    let mut chars = name.chars();

    let starts_well = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    let continues_well = chars.all(|next| next.is_ascii_alphanumeric() || next == '_');

    (starts_well && continues_well).then_some(name)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_whole_env_placeholders_become_references() {
        let value = json!({
            "apiKey": "{env:ANTHROPIC_API_KEY}",
            "list": ["{env:_X1}", "{env:1BAD}", "pre {env:Y}", "{file:~/.secrets/key}"],
        });

        assert_eq!(
            env_names(value),
            json!({
                "apiKey": "${ANTHROPIC_API_KEY}",
                "list": ["${_X1}", "{env:1BAD}", "pre {env:Y}", "{file:~/.secrets/key}"],
            })
        );
    }
}
