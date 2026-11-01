//! The notes guard of a Codex chat ([plan](../../../../docs/plans/codex.md), decision 11) and the
//! MCP servers its helper chat switches off.
//!
//! Codex starts its MCP servers with a cleared environment, so the `HARNESS_MEMORY_SESSION` of a
//! chat with a memory identity reaches a harness-memory server only when the server's `env_vars`
//! names it. The same goes for `HARNESS_KNOWLEDGE_SESSION` and a harness-knowledge server, which is
//! told the same way but never counts for the chat's own memory settings (no notice, no state). So the chat reads the project's effective settings from Codex itself (`config/read`
//! with the project as `cwd`: every layer Codex loads there, with its trust, profiles and managed
//! settings) after the handshake and before every later new, resumed or forked thread, and sends
//! `"mcp_servers.<name>.env_vars": [<its own…>, "HARNESS_MEMORY_SESSION"]` in the `config` of each
//! `thread/start`, `thread/resume` and `thread/fork` for every enabled stdio server named
//! `harness-memory` or running the `harness-memory` program. That holds for the thread only:
//! nothing is written to `config.toml` and nothing goes on the command line (so a `codex.cmd`
//! shim passes it too).
//!
//! - A server the read did not return is never named: Codex refuses a thread whose `config` names
//!   a server it does not have (`invalid transport`). A server whose name a key path cannot hold (a
//!   dot) or whose `env_vars` is not a list gets nothing.
//! - The chat passes its identity on ([`CodexSession::memory_passed_on`]) only when the newest read
//!   gave at least one server the variable, or one lists it already. Otherwise the app does not
//!   offer the chat's own guards. A later read that changes it sends a `state` with it
//!   (`memoryGuards`), so the memory button follows. Every harness-memory server that could not be
//!   told gets a notice naming it, also when another one was, once for each new list of them.
//! - A later read that fails keeps the servers of the one before.
//! - A helper chat (AI naming) switches off every server of the read (`naming.rs`). Servers the
//!   read does not list (a plugin's own) stay on for it.

use std::time::Duration;

use serde_json::{Map, Value};

use super::{Chat, CodexSession, Live};
use crate::codex_rpc::wire::outbound::Request;
use crate::ecosystem::catalog::{KNOWLEDGE_BINARY, KNOWLEDGE_MCP_SERVER, MEMORY_BINARY, MEMORY_MCP_SERVER};
use crate::ecosystem::knowledge_binary::KNOWLEDGE_SESSION_VAR;
use crate::ecosystem::memory_guards::SESSION_VAR;
use crate::error::{BusinessError, Result};
use crate::harnesses::codex::is_plain_name;
use crate::model::{AgentEvent, NoticeLevel};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;

/// The MCP servers of Codex's effective settings, by name.
pub(super) type McpServers = Map<String, Value>;

/// What a chat with a memory identity sends with its threads for its servers.
#[derive(Debug, Default)]
pub(super) struct MemoryConfig {
    /// The thread `config` entries: the `env_vars` of each harness-memory server.
    pub config: Map<String, Value>,
    /// At least one server gets the variable or lists it already.
    pub passed_on: bool,
    /// The harness-memory servers that get nothing (their name or variables cannot be written).
    pub missed: Vec<String>,
}

/// The `env_vars` entries of every harness-memory server of `servers` (see the module docs).
pub(super) fn memory_config(servers: &McpServers) -> MemoryConfig {
    let mut memory = MemoryConfig::default();

    for (name, server) in servers {
        if server.get("enabled") == Some(&Value::Bool(false)) {
            continue;
        }

        if runs(name, server, KNOWLEDGE_MCP_SERVER, KNOWLEDGE_BINARY) {
            // Harness Knowledge learns the chat the same way, but its guards are not the chat's own
            // memory settings, so it never changes what the chat offers or says.
            if let EnvVars::Add(entries) = env_vars_with(name, server, KNOWLEDGE_SESSION_VAR) {
                memory.config.insert(env_vars_key(name), Value::Array(entries));
            }

            continue;
        }

        if !runs(name, server, MEMORY_MCP_SERVER, MEMORY_BINARY) {
            continue;
        }

        match env_vars_with(name, server, SESSION_VAR) {
            EnvVars::Add(entries) => {
                memory.config.insert(env_vars_key(name), Value::Array(entries));
                memory.passed_on = true;
            }
            EnvVars::Present => memory.passed_on = true,
            EnvVars::Cannot => memory.missed.push(name.clone()),
        }
    }

    memory
}

/// What it takes for a server's `env_vars` to name a session variable.
enum EnvVars {
    /// The entries to send: its own and the variable.
    Add(Vec<Value>),
    /// It names the variable already.
    Present,
    /// Codex cannot be told: its `env_vars` is no list, or its name cannot be a key path.
    Cannot,
}

/// The `env_vars` of `server` with `variable` added (see [`EnvVars`]).
fn env_vars_with(name: &str, server: &Value, variable: &str) -> EnvVars {
    let own = match server.get("env_vars") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(entries)) => entries.clone(),
        Some(_) => return EnvVars::Cannot,
    };

    if own.iter().any(|entry| names_session_var(entry, variable)) {
        return EnvVars::Present;
    }

    if !is_plain_name(name) {
        return EnvVars::Cannot;
    }

    let mut entries = own;
    entries.push(Value::String(variable.to_owned()));

    EnvVars::Add(entries)
}

/// The `config` key of the `env_vars` of server `name`.
fn env_vars_key(name: &str) -> String {
    format!("mcp_servers.{name}.env_vars")
}

/// The notice of a chat whose harness-memory servers `missed` cannot be told to pass its
/// identity on, whether or not another server got it; none without such a server.
pub(super) fn memory_notice(memory: &MemoryConfig) -> Option<AgentEvent> {
    if memory.missed.is_empty() {
        return None;
    }

    let missed = memory.missed.join(", ");
    let message = match memory.passed_on {
        true => format!(
            "Codex cannot be told to pass this chat's key on to {missed}, so the chat's own memory settings do not apply there. The settings for every chat still apply"
        ),
        false => format!(
            "This chat's own memory settings do not apply: Codex cannot be told to pass the chat's key on to {missed}. The settings for every chat still apply"
        ),
    };

    Some(AgentEvent::Notice {
        level: NoticeLevel::Warning,
        message,
    })
}

/// A stdio server named `mcp_name` or running the `binary` program.
fn runs(name: &str, server: &Value, mcp_name: &str, binary: &str) -> bool {
    let Some(program) = server.get("command").and_then(Value::as_str) else {
        return false;
    };

    let file_name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let stem = file_name.strip_suffix(".exe").unwrap_or(&file_name);

    name == mcp_name || stem == binary
}

/// An `env_vars` entry naming `variable`: the name, or a table with it as `name`.
fn names_session_var(entry: &Value, variable: &str) -> bool {
    match entry {
        Value::String(name) => name == variable,
        Value::Object(fields) => fields.get("name").and_then(Value::as_str) == Some(variable),
        _ => false,
    }
}

impl Chat {
    /// The MCP servers Codex runs a thread of the project with (`config/read`), within `limit`.
    pub(super) async fn mcp_servers(&self, limit: Duration) -> Result<McpServers> {
        let request = Request::ConfigRead {
            cwd: self.project.to_string_lossy().into_owned(),
        };
        let read: Value = self.request(&request, limit).await?;

        match read.pointer("/config/mcp_servers") {
            None | Some(Value::Null) => Ok(Map::new()),
            Some(Value::Object(servers)) => Ok(servers.clone()),
            Some(_) => Err(BusinessError::Protocol(
                "Codex sent an unreadable answer to config/read".to_owned(),
            )),
        }
    }

    /// Before a new, resumed or forked thread of a chat with a memory identity: its servers read
    /// again, a failed read keeping the last ones. A change of whether the identity reaches a
    /// server sends a `state`, a new list of servers that cannot be told their notice.
    pub(super) async fn refresh_memory_config(&self) {
        if !self.memory_session {
            return;
        }

        let servers = match self.mcp_servers(DEFAULT_TIMEOUT).await {
            Ok(servers) => servers,
            Err(error) => {
                tracing::debug!(%error, "Codex did not list its MCP servers again, the last ones stay");
                return;
            }
        };

        let memory = memory_config(&servers);

        let mut live = self.lock();
        let notice = memory_notice(&memory).filter(|_| live.memory_missed != memory.missed);
        let changed = live.memory_passed_on != memory.passed_on;
        live.memory_config = memory.config;
        live.memory_passed_on = memory.passed_on;
        live.memory_missed = memory.missed;

        if let Some(notice) = notice {
            self.shared.emit(notice);
        }

        if changed {
            self.emit_state(&live);
        }
    }

    /// Whether the chat's own guards apply, for its `state`: `None` without a memory identity.
    pub(super) fn memory_guards(&self, live: &Live) -> Option<bool> {
        self.memory_session.then_some(live.memory_passed_on)
    }
}

impl CodexSession {
    /// Whether the chat's memory identity reaches a harness-memory server (see the module docs).
    pub fn memory_passed_on(&self) -> bool {
        self.chat.lock().memory_passed_on
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn servers(value: Value) -> McpServers {
        value.as_object().cloned().expect("an object")
    }

    #[test]
    fn a_knowledge_server_is_told_the_chat_without_counting_for_the_memory_guards() {
        let listed = servers(json!({
            "harness-knowledge": { "command": "/home/u/.local/bin/harness-knowledge", "args": ["mcp"] },
            "docs": { "command": "C:\\tools\\Harness-Knowledge.exe", "env_vars": ["TOKEN"] },
            "off": { "command": "harness-knowledge", "enabled": false },
            "told": { "command": "harness-knowledge", "env_vars": [{ "name": "HARNESS_KNOWLEDGE_SESSION" }] },
            "odd.name": { "command": "harness-knowledge" },
        }));

        let config = memory_config(&listed);

        assert_eq!(
            config.config.get("mcp_servers.harness-knowledge.env_vars"),
            Some(&json!(["HARNESS_KNOWLEDGE_SESSION"]))
        );
        assert_eq!(
            config.config.get("mcp_servers.docs.env_vars"),
            Some(&json!(["TOKEN", "HARNESS_KNOWLEDGE_SESSION"]))
        );
        assert_eq!(config.config.len(), 2, "off, told and unnameable servers get nothing");
        assert!(!config.passed_on, "the chat's own memory guards need a memory server");
        assert!(config.missed.is_empty(), "no notice about a knowledge server");
    }

    #[test]
    fn each_server_gets_its_own_variable() {
        let listed = servers(json!({
            "harness-memory": { "command": "harness-memory" },
            "harness-knowledge": { "command": "harness-knowledge" },
        }));

        let config = memory_config(&listed);

        assert_eq!(
            config.config.get("mcp_servers.harness-memory.env_vars"),
            Some(&json!(["HARNESS_MEMORY_SESSION"]))
        );
        assert_eq!(
            config.config.get("mcp_servers.harness-knowledge.env_vars"),
            Some(&json!(["HARNESS_KNOWLEDGE_SESSION"]))
        );
        assert!(config.passed_on);
    }
}
