//! `codex app-server`: the command line of a Codex chat (Codex CLI 0.154, the plan's "Protocol"
//! and decision 8).
//!
//! The chat talks JSON-RPC over stdin and stdout (`--listen stdio://`, the default), one process
//! per chat. Every launch switches Codex's own update check and update prompts off
//! ([`APP_SERVER_ARGS`]); a launch that trusts the project once trusts it for this process only
//! ([`trust_once_args`]). Codex starts its MCP servers with a cleared environment, so a chat with
//! a memory identity names `HARNESS_MEMORY_SESSION` among the variables of each harness-memory
//! server it has ([`memory_session_args`], plan decision 11). Nothing else of the user's
//! `config.toml` is overridden here, and nothing is written to it. The chat
//! engine names the thread after the handshake (`thread/start`, or `thread/resume` for a resumed
//! chat) with the project as its working directory and the chat's approval mode, so no session,
//! model or approval flag is ever on the command line. A resumed thread id is a lowercase UUID.

use std::path::Path;

use serde_json::Value;

use super::paths::codex_home;
use super::settings::{is_plain_name, mcp_servers};
use super::trust::{toml_string, trust_once_args};
use crate::ecosystem::catalog::{MEMORY_BINARY, MEMORY_MCP_SERVER};
use crate::ecosystem::memory_guards::SESSION_VAR;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{LaunchRequest, rpc_exec_spec};
use crate::process::ExecSpec;

/// The arguments of every Codex chat launch: the app server, without the update check at startup
/// and with the `in_app_updates` feature off (`-c features.<name>=false`, which is what
/// `--disable <name>` sets).
pub const APP_SERVER_ARGS: [&str; 5] = [
    "app-server",
    "-c",
    "check_for_update_on_startup=false",
    "-c",
    "features.in_app_updates=false",
];

/// `codex app-server <update overrides> [<trust once>]` in the project. A resumed thread is
/// checked here and named by the engine.
pub fn codex_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    if let Some(id) = request.resume
        && !is_thread_id(id)
    {
        return Err(BusinessError::InvalidInput(
            "a Codex thread id is a lowercase UUID".to_owned(),
        ));
    }

    let mut args = APP_SERVER_ARGS.map(str::to_owned).to_vec();

    if request.trust_once {
        args.extend(trust_once_args(request.project)?);
    }

    rpc_exec_spec(request.binary, request.project, &args)
}

/// True for a Codex thread id: a lowercase, hyphenated UUID.
pub fn is_thread_id(id: &str) -> bool {
    uuid::Uuid::try_parse(id).is_ok_and(|parsed| parsed.hyphenated().to_string() == id)
}

/// `-c mcp_servers.<name>.env_vars=[<its own…>, "HARNESS_MEMORY_SESSION"]` for every
/// harness-memory stdio server of the settings that apply to a chat in `project` (the user file,
/// and the project file in a trusted folder or one trusted once): one named `harness-memory` or
/// running the `harness-memory` program. A server missing from them gets none (an override for a
/// server Codex does not have stops `codex app-server`), and so does one whose name or variables
/// cannot be written as a `-c` key path and TOML value.
pub(super) fn memory_session_args(env: &Env, project: &Path, trust_once: bool) -> Vec<String> {
    mcp_servers(&codex_home(env), project, trust_once)
        .iter()
        .filter(|(name, server)| is_memory_server(name, server))
        .filter_map(|(name, server)| env_vars_override(name, server))
        .flat_map(|value| ["-c".to_owned(), value])
        .collect()
}

/// A stdio server named `harness-memory` or running the `harness-memory` program.
fn is_memory_server(name: &str, server: &Value) -> bool {
    let Some(program) = server.get("command").and_then(Value::as_str) else {
        return false;
    };

    let file_name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let stem = file_name.strip_suffix(".exe").unwrap_or(&file_name);

    name == MEMORY_MCP_SERVER || stem == MEMORY_BINARY
}

/// `mcp_servers.<name>.env_vars=[…]` adding the session variable to the server's own list; `None`
/// when it is listed already, or the name or a listed entry cannot be written.
fn env_vars_override(name: &str, server: &Value) -> Option<String> {
    let own = match server.get("env_vars") {
        None => Vec::new(),
        Some(Value::Array(entries)) => entries.clone(),
        Some(_) => return None,
    };

    if !is_plain_name(name) || own.iter().any(|entry| entry == SESSION_VAR) {
        return None;
    }

    let mut entries = own.iter().map(toml_value).collect::<Option<Vec<_>>>()?;
    entries.push(format!("\"{SESSION_VAR}\""));

    Some(format!("mcp_servers.{name}.env_vars=[{}]", entries.join(",")))
}

/// `value` (a string, or a table of strings) as a TOML value.
fn toml_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => toml_string(text),
        Value::Object(fields) => {
            let pairs = fields
                .iter()
                .map(|(key, value)| Some(format!("{}={}", toml_string(key)?, toml_string(value.as_str()?)?)))
                .collect::<Option<Vec<_>>>()?;

            Some(format!("{{{}}}", pairs.join(",")))
        }
        _ => None,
    }
}
