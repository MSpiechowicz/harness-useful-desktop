//! Redaction of Claude Code settings. The shared rules ([`Redactor`]) already hide `env`, hook
//! `headers` and every `mcpServers.<name>.env` and `.headers`, and keys such as `apiKeyHelper`.
//! Claude adds settings whose names the rules do not know: the commands that print credentials or
//! headers are shown as hidden commands, `pluginConfigs` (plugin options, which may hold keys) as a
//! hidden container, and the secret-looking words of MCP `args` are masked.

use serde_json::Value;

use crate::model::{ConfigField, ConfigValue, SecretKind};
use crate::pi_family::mcp_config::mask_server_args;
use crate::security::{RedactionContext, Redactor};

/// Settings holding a command whose output is a credential or a set of headers.
const SECRET_COMMANDS: &[&str] = &[
    "apiKeyHelper",
    "awsCredentialExport",
    "awsAuthRefresh",
    "otelHeadersHelper",
];

/// Settings whose whole value is hidden, keys aside.
const HIDDEN_SETTINGS: &[&str] = &["pluginConfigs"];

/// The redacted top-level fields of a settings object; `null` has none.
pub(super) fn settings_fields(value: &Value) -> Result<Vec<ConfigField>, String> {
    let entries = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Object(entries) => entries,
        _ => return Err("Expected a key/value object at the top level".to_owned()),
    };

    let mut fields: Vec<ConfigField> = entries
        .iter()
        .map(|(key, value)| ConfigField {
            key: key.clone(),
            value: redact_setting(key, value),
        })
        .collect();

    mask_server_args(&mut fields, "mcpServers");
    Ok(fields)
}

fn redact_setting(key: &str, value: &Value) -> ConfigValue {
    if SECRET_COMMANDS.contains(&key) {
        return match value {
            Value::String(_) => ConfigValue::Secret(SecretKind::Command),
            other => Redactor.redact_hidden(other),
        };
    }

    if HIDDEN_SETTINGS.contains(&key) {
        return Redactor.redact_hidden(value);
    }

    Redactor.redact_at(value, &mut RedactionContext::at([key]))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn field<'a>(fields: &'a [ConfigField], key: &str) -> &'a ConfigValue {
        &fields.iter().find(|field| field.key == key).unwrap().value
    }

    #[test]
    fn credential_commands_and_plugin_options_are_hidden() {
        let fields = settings_fields(&json!({
            "awsAuthRefresh": "aws sso login --profile work",
            "otelHeadersHelper": "/opt/bin/headers.sh",
            "pluginConfigs": { "demo@market": { "options": { "region": "eu", "limit": 3 } } },
            "model": "opus",
        }))
        .unwrap();

        assert_eq!(
            field(&fields, "awsAuthRefresh"),
            &ConfigValue::Secret(SecretKind::Command)
        );
        assert_eq!(
            field(&fields, "otelHeadersHelper"),
            &ConfigValue::Secret(SecretKind::Command)
        );
        assert!(!format!("{:?}", field(&fields, "pluginConfigs")).contains("eu"));
        assert_eq!(field(&fields, "model"), &ConfigValue::Text("opus".to_owned()));
    }

    #[test]
    fn a_settings_file_must_hold_an_object() {
        assert!(settings_fields(&json!([1, 2])).is_err());
        assert_eq!(settings_fields(&Value::Null), Ok(Vec::new()));
    }
}
