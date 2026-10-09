use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A configuration value after redaction (see `security::redact`).
///
/// Secrets are replaced by [`ConfigValue::Secret`] before a value ever leaves the business layer,
/// so the webview can render configs without seeing credential material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[ts(export)]
pub enum ConfigValue {
    /// JSON `null` / YAML `~`.
    Null,
    /// A boolean.
    Bool(bool),
    /// A number in its textual form (avoids float rounding in JavaScript).
    Number(String),
    /// A visible string.
    Text(String),
    /// An ordered list.
    List(Vec<ConfigValue>),
    /// An object, in source order where available.
    Map(Vec<ConfigField>),
    /// A hidden value; only its kind is disclosed.
    Secret(SecretKind),
}

/// One key/value entry of a [`ConfigValue::Map`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigField {
    /// The key, always visible.
    pub key: String,
    /// The (redacted) value.
    pub value: ConfigValue,
}

/// What kind of secret was hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum SecretKind {
    /// A literal secret value (token, password, key, ...).
    Value,
    /// A `!command` that produces a secret; its text is hidden and it is never executed.
    Command,
    /// A `$NAME` / `${NAME}` environment reference; the variable name is safe to show, its value
    /// is never read.
    EnvRef {
        /// The referenced environment variable name.
        name: String,
    },
}
