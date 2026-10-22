//! MCP server entries (`mcpServers` in `mcp.json`) as redacted [`McpServerInfo`]s.
//!
//! Every value is redacted at its real key path (`mcpServers.<name>.env.TOKEN`), so `env`,
//! `headers`, `auth` and `oauth` secrets never leave the business layer. How entries from several
//! files combine differs per harness and is left to the adapters. [`command_words`] masks the
//! secrets an MCP command line carries in its words.

use serde_json::Value;

use crate::model::{ConfigField, ConfigSource, ConfigValue, McpServerInfo, McpTransport};
use crate::security::{RedactionContext, Redactor, redact_text};

/// One parsed server plus the raw facts adapters need for merging.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedServer {
    /// The redacted server.
    pub info: McpServerInfo,
    /// True when the entry names an endpoint (`command`, `url` or `type`); PI treats entries
    /// without one as overrides of a same-named user server.
    pub has_endpoint: bool,
    /// The entry's own `enabled` flag, if it set one.
    pub enabled_flag: Option<bool>,
}

/// Parses the servers object found at `key_path` (e.g. `["mcpServers"]`) of a file.
/// Servers whose value is not an object are skipped.
pub fn parse_servers(servers: Option<&Value>, key_path: &[&str], source: &ConfigSource) -> Vec<ParsedServer> {
    let Some(servers) = servers.and_then(Value::as_object) else {
        return Vec::new();
    };

    servers
        .iter()
        .filter(|(_, server)| server.is_object())
        .map(|(name, server)| {
            let mut path: Vec<&str> = key_path.to_vec();
            path.push(name);

            parse_server(name, server, &path, source)
        })
        .collect()
}

fn parse_server(name: &str, server: &Value, path: &[&str], source: &ConfigSource) -> ParsedServer {
    let enabled_flag = enabled_flag(server.get("enabled"));
    let has_endpoint = ["command", "url", "type"].iter().any(|key| server.get(*key).is_some());

    let mut info = McpServerInfo {
        name: name.to_owned(),
        transport: transport(server),
        command: None,
        args: Vec::new(),
        url: None,
        enabled: enabled_flag.unwrap_or(true),
        source: source.clone(),
        details: Vec::new(),
    };

    // `server` is an object, so its redaction is a map.
    let redacted = Redactor.redact_at(server, &mut RedactionContext::at(path.iter().copied()));
    let fields = match redacted {
        ConfigValue::Map(fields) => fields,
        _ => Vec::new(),
    };

    for ConfigField { key, value } in fields {
        match (key.as_str(), value) {
            ("type" | "enabled", _) => {}
            ("command", ConfigValue::Text(command)) => info.command = Some(command),
            ("args", ConfigValue::List(args)) => info.args = args,
            ("url", url) => info.url = Some(url),
            (_, value) => info.details.push(ConfigField { key, value }),
        }
    }

    ParsedServer {
        info,
        has_endpoint,
        enabled_flag,
    }
}

/// `type` when present, else stdio for a `command` and HTTP for a `url`.
fn transport(server: &Value) -> McpTransport {
    match server.get("type").and_then(Value::as_str) {
        Some("stdio") => return McpTransport::Stdio,
        Some("http" | "streamable-http") => return McpTransport::Http,
        Some("sse") => return McpTransport::Sse,
        Some(_) => return McpTransport::Unknown,
        None => {}
    }

    if server.get("command").is_some() {
        McpTransport::Stdio
    } else if server.get("url").is_some() {
        McpTransport::Http
    } else {
        McpTransport::Unknown
    }
}

/// A boolean `enabled`, also accepting the strings `true`/`false`/`1`/`0` (OMP coerces them).
fn enabled_flag(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => match text.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Masks secrets in the already redacted words of an MCP command line: credentials and
/// `--token=...` assignments inside a word, and the whole word after a flag whose name looks
/// secret (`--api-key <value>`).
pub fn command_words(words: Vec<ConfigValue>) -> Vec<ConfigValue> {
    let mut masked: Vec<ConfigValue> = Vec::with_capacity(words.len());

    for word in words {
        let flag = match masked.last() {
            Some(ConfigValue::Text(previous)) => flag_name(previous),
            _ => None,
        };

        let word = match (word, flag) {
            (ConfigValue::Text(text), Some(flag)) => {
                Redactor.redact_at(&Value::String(text), &mut RedactionContext::at([flag]))
            }
            (ConfigValue::Text(text), None) => ConfigValue::Text(redact_text(&text)),
            (other, _) => other,
        };

        masked.push(word);
    }

    masked
}

/// The name of a `--flag` / `-f` word that carries no value of its own.
fn flag_name(word: &str) -> Option<String> {
    let name = word.trim_start_matches('-');

    if name.len() == word.len() || name.is_empty() || name.contains('=') {
        return None;
    }

    Some(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SecretKind;

    fn texts(words: &[&str]) -> Vec<ConfigValue> {
        words.iter().map(|word| ConfigValue::Text((*word).to_owned())).collect()
    }

    #[test]
    fn words_after_secret_flags_and_secret_assignments_are_hidden() {
        let masked = command_words(texts(&[
            "npx",
            "server",
            "--api-key",
            "abc",
            "--port",
            "8080",
            "--token=abc",
        ]));

        assert_eq!(masked[3], ConfigValue::Secret(SecretKind::Value));
        assert_eq!(masked[5], ConfigValue::Text("8080".to_owned()));
        assert_eq!(masked[6], ConfigValue::Text("--token=••••".to_owned()));
    }
}
