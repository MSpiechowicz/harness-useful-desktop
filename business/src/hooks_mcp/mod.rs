//! The MCP server mode of the app (`harness-desktop mcp`): agents list, read, add, change and
//! remove hooks through it. Newline-delimited JSON-RPC 2.0 over stdio, read synchronously one
//! bounded line at a time; no window and no Tauri runtime.
//!
//! - Tools: `hook_list`, `hook_get`, `hook_add`, `hook_update`, `hook_remove` ([`tools`]). Every
//!   write goes through [`HookStore`] as an agent, so it is pending until the user approves it in
//!   the Hooks view, and the server never writes the app's config folder. It reads the approvals
//!   there to report each hook as approved or pending, and never runs a hook.
//! - `project` defaults to the server's working folder (the harness starts it in the chat's
//!   folder), turned into a project root like the app does (`hooks::project_identity`).
//! - A line longer than [`MAX_FRAME_BYTES`] is answered with an error and skipped up to its
//!   newline. Unparsable JSON gets `-32700`, a batch (array) or a malformed request `-32600`, an
//!   unknown method `-32601`, an unknown tool `-32602`; serving continues after each.
//! - Notifications are never answered; answers from the client are ignored.
//! - Refusals and validation failures are tool results with `isError: true`.
//! - Nothing a client sends is written to stderr (the mode installs no log subscriber).

mod calls;
pub mod protocol;
pub mod tools;

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::app_dirs::config_dir;
use crate::ecosystem::{installed_memory, resolve_dashboard};
use crate::env::Env;
use crate::hooks::{HookCapabilities, HookStore};
use crate::pi_family::rpc::MAX_FRAME_BYTES;
use crate::prefs::PREFS_FILE;
use crate::process::{ProcessRunner, TokioRunner};
use crate::security::SafeReader;
use protocol::{INVALID_PARAMS, INVALID_REQUEST, Line, METHOD_NOT_FOUND, PARSE_ERROR};

/// The argument that starts the MCP server mode instead of the window.
pub const MCP_ARG: &str = "mcp";

/// Largest prefs file read for the memory setting.
const MAX_PREFS_BYTES: u64 = 256 * 1024;

/// The state one server process keeps.
#[derive(Debug)]
pub struct Server {
    /// The hooks folder; `None` when this computer has none (Windows without `LOCALAPPDATA`).
    store: Option<HookStore>,
    /// The folder the harness started the server in.
    cwd: Option<PathBuf>,
    /// What the hooks' `requires` are checked against, once at start.
    capabilities: HookCapabilities,
}

impl Server {
    /// A server over `store`, defaulting `project` to `cwd`, with no requirement met.
    pub fn new(store: Option<HookStore>, cwd: Option<PathBuf>) -> Self {
        Self {
            store,
            cwd,
            capabilities: HookCapabilities::default(),
        }
    }

    /// The server with the hooks' requirements checked against `capabilities`.
    pub fn with_capabilities(mut self, capabilities: HookCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// The server of this machine: the hooks folder of `env` with the approvals in the app's
    /// config folder, its requirements checked against `capabilities` ([`machine_capabilities`]).
    pub fn from_env(env: &Env, cwd: Option<PathBuf>, capabilities: HookCapabilities) -> Self {
        Self::new(HookStore::from_env(env, config_dir(env)), cwd).with_capabilities(capabilities)
    }

    /// Answers one parsed message; `None` for notifications and client answers.
    pub fn handle(&mut self, message: Value) -> Option<Value> {
        let null = Value::Null;

        let Some(object) = message.as_object() else {
            return Some(protocol::error(&null, INVALID_REQUEST, "Invalid Request"));
        };

        let id = object.get("id");
        let method = object.get("method").and_then(Value::as_str);
        let valid_id = id.is_none_or(|id| id.is_string() || id.is_number() || id.is_null());
        let reply_id = id.filter(|_| valid_id).cloned().unwrap_or(Value::Null);

        let Some(method) = method else {
            let is_answer = id.is_some() && (object.contains_key("result") || object.contains_key("error"));
            return (!is_answer).then(|| protocol::error(&reply_id, INVALID_REQUEST, "Invalid Request"));
        };

        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || !valid_id {
            return Some(protocol::error(&reply_id, INVALID_REQUEST, "Invalid Request"));
        }

        // A notification (no id) is never answered, whatever its method.
        let id = id?;
        let params = object.get("params").cloned().unwrap_or_else(|| json!({}));

        let answer = match method {
            "initialize" => protocol::result(id, protocol::initialize_result(&params)),
            "ping" => protocol::result(id, json!({})),
            "tools/list" => protocol::result(id, tools::definitions()),
            "tools/call" => self.tools_call(id, &params),
            _ => protocol::error(id, METHOD_NOT_FOUND, "Method not found"),
        };

        Some(answer)
    }

    fn tools_call(&self, id: &Value, params: &Value) -> Value {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return protocol::error(id, INVALID_PARAMS, "tools/call needs a tool name");
        };

        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => json!({}),
            Some(arguments) => arguments.clone(),
        };

        match calls::call(self, name, arguments) {
            Some(reply) => protocol::result(id, tool_result(reply)),
            None => protocol::error(id, INVALID_PARAMS, &format!("unknown tool: {}", short(name))),
        }
    }
}

/// The `tools/call` result of `reply`: the reply object as text and as `structuredContent`, or
/// the refusal text with `isError`.
fn tool_result(reply: calls::Reply) -> Value {
    match reply {
        Ok(value) => {
            let text = serde_json::to_string_pretty(&value).unwrap_or_default();

            json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": value,
                "isError": false,
            })
        }
        Err(refusal) => json!({
            "content": [{ "type": "text", "text": refusal }],
            "isError": true,
        }),
    }
}

/// Serves messages from `reader` until end of input; fails only when reading or writing fails.
pub fn serve<R: BufRead, W: Write>(mut reader: R, mut writer: W, server: &mut Server) -> io::Result<()> {
    let mut buffer = Vec::new();

    loop {
        let reply = match protocol::read_line(&mut reader, &mut buffer, MAX_FRAME_BYTES)? {
            Line::End => return Ok(()),
            Line::TooLong => Some(protocol::error(
                &Value::Null,
                INVALID_REQUEST,
                &format!("message exceeds {MAX_FRAME_BYTES} bytes"),
            )),
            Line::Text(line) if line.iter().all(u8::is_ascii_whitespace) => None,
            Line::Text(line) => match serde_json::from_slice::<Value>(&line) {
                Ok(message) => server.handle(message),
                Err(_) => Some(protocol::error(&Value::Null, PARSE_ERROR, "Parse error")),
            },
        };

        if let Some(reply) = reply {
            protocol::write_message(&mut writer, &reply)?;
        }
    }
}

/// `harness-desktop mcp`: serves this machine's hooks on stdin and stdout until stdin closes.
/// Fails when the environment cannot be read (no home folder) or stdio breaks.
pub fn run_stdio() -> ExitCode {
    let Ok(env) = Env::from_process() else {
        return ExitCode::FAILURE;
    };

    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
        return ExitCode::FAILURE;
    };
    let capabilities = runtime.block_on(machine_capabilities(&env, &TokioRunner));
    drop(runtime);

    let cwd = std::env::current_dir().ok();
    let mut server = Server::from_env(&env, cwd, capabilities);

    match serve(io::stdin().lock(), io::stdout().lock(), &mut server) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// The family members this machine has, checked like the app window does: Harness Memory counts
/// when it is installed and reports its version (`runner` asks it) and its memory setting is on
/// (the prefs file read without changing it), Harness Dashboard when it is installed.
pub async fn machine_capabilities(env: &Env, runner: &dyn ProcessRunner) -> HookCapabilities {
    let memory_on = memory_enabled(&config_dir(env));

    HookCapabilities {
        memory_available: memory_on && installed_memory(env, runner).await.is_some(),
        dashboard_available: resolve_dashboard(env).is_some(),
    }
}

/// The stored `memoryEnabled` pref, read only (on when unset or unreadable, like its default).
fn memory_enabled(config_dir: &Path) -> bool {
    let Ok(text) = SafeReader::read_to_string(&config_dir.join(PREFS_FILE), MAX_PREFS_BYTES) else {
        return true;
    };

    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|prefs| prefs.get("memoryEnabled").and_then(Value::as_bool))
        .unwrap_or(true)
}

/// At most 64 characters of agent text, for error messages.
fn short(text: &str) -> String {
    text.chars().take(64).collect()
}
