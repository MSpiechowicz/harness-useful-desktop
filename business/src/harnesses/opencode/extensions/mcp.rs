//! OpenCode MCP servers: the `mcp` key of each settings file, deep-merged by server name with
//! later files winning (`config/config.ts`). OpenCode 2 (`core/src/config/plugin/mcp.ts`, 2.1.2)
//! reads `mcp.servers.<name>` and the legacy `mcp.<name>` (see [`mcp_servers`](super::super::config::mcp_servers)),
//! a later file replaces a whole server, and `disabled: true` switches one off.
//!
//! A local server is `{ "type": "local", "command": [executable, ...args], "environment": {...} }`,
//! a remote one `{ "type": "remote", "url": ..., "headers": {...}, "oauth": {...} }`; both may set
//! `enabled` and `timeout`. Every value is redacted at its key path (`mcp.<name>.environment`,
//! `headers` and the `oauth` secrets are hidden). Command-line words also pass the free-text
//! masking, and the word after a secret-looking flag (`--api-key <value>`) is hidden.

use serde_json::Value;

use super::super::config::{Layers, Step, command_words, format_of, mcp_servers as file_servers};
use crate::harness::OpenCodeGeneration;
use crate::model::{ConfigField, ConfigSource, ConfigValue, McpServerInfo, McpTransport};
use crate::pi_family::source;
use crate::security::{RedactionContext, Redactor};

/// See the module docs. Servers keep the order in which their names first appear; the source is
/// the last file that set the server.
pub(super) fn mcp_servers(layers: &Layers) -> Vec<McpServerInfo> {
    let generation = layers.generation;
    let mut merged: Vec<(String, Value, ConfigSource)> = Vec::new();

    for step in &layers.steps {
        let Step::File { path, place, document } = step else {
            continue;
        };
        let Some(servers) = document.value().and_then(|value| file_servers(value, generation)) else {
            continue;
        };
        let file = source(path, place.layer(), None, format_of(path));

        for (name, server) in servers.into_iter().filter(|(_, server)| server.is_object()) {
            match merged.iter_mut().find(|(existing, _, _)| *existing == name) {
                Some((_, value, source)) => {
                    match generation {
                        OpenCodeGeneration::V1 => merge_deep(value, &server),
                        OpenCodeGeneration::V2 => *value = server,
                    }
                    *source = file.clone();
                }
                None => merged.push((name, server, file.clone())),
            }
        }
    }

    merged
        .into_iter()
        .map(|(name, server, source)| server_info(name, &server, source, generation))
        .collect()
}

/// Objects merge key by key; anything else replaces the target.
fn merge_deep(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(key) {
                    Some(existing) => merge_deep(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

fn server_info(name: String, server: &Value, source: ConfigSource, generation: OpenCodeGeneration) -> McpServerInfo {
    let switched_off = |key: &str, off: bool| server.get(key).and_then(Value::as_bool) == Some(off);
    let disabled =
        switched_off("enabled", false) || (generation == OpenCodeGeneration::V2 && switched_off("disabled", true));

    let mut info = McpServerInfo {
        transport: transport(server),
        command: None,
        args: Vec::new(),
        url: None,
        enabled: !disabled,
        source,
        details: Vec::new(),
        name,
    };

    let mut context = RedactionContext::at(["mcp", info.name.as_str()]);
    let ConfigValue::Map(fields) = Redactor.redact_at(server, &mut context) else {
        return info;
    };

    for ConfigField { key, value } in fields {
        match (key.as_str(), value) {
            ("type" | "enabled", _) => {}
            ("disabled", _) if generation == OpenCodeGeneration::V2 => {}
            ("command", ConfigValue::List(words)) => {
                let mut words = command_words(words).into_iter();

                info.command = match words.next() {
                    Some(ConfigValue::Text(executable)) => Some(executable),
                    _ => None,
                };
                info.args = words.collect();
            }
            ("url", url) => info.url = Some(url),
            (_, value) => info.details.push(ConfigField { key, value }),
        }
    }

    info
}

/// `local` runs a child process over stdio; `remote` speaks streamable HTTP (falling back to SSE).
fn transport(server: &Value) -> McpTransport {
    match server.get("type").and_then(Value::as_str) {
        Some("local") => McpTransport::Stdio,
        Some("remote") => McpTransport::Http,
        Some(_) => McpTransport::Unknown,
        None if server.get("command").is_some() => McpTransport::Stdio,
        None if server.get("url").is_some() => McpTransport::Http,
        None => McpTransport::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn later_files_merge_into_earlier_servers() {
        let mut server = json!({ "type": "local", "command": ["a"], "environment": { "A": "1" } });
        merge_deep(&mut server, &json!({ "enabled": false, "environment": { "B": "2" } }));

        assert_eq!(
            server,
            json!({ "type": "local", "command": ["a"], "environment": { "A": "1", "B": "2" }, "enabled": false })
        );
    }
}
