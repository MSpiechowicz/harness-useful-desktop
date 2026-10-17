//! A minimal Model Context Protocol client over stdio, for local read-only servers such as
//! `harness-dashboard mcp`.
//!
//! - Framing: newline-delimited JSON-RPC 2.0, one message per line, as the dashboard's
//!   `serveStream` (`src/mcp/server.ts`) reads and writes it; [`FrameReader`] keeps at most
//!   [`McpOptions::max_response_bytes`] of a line.
//! - Handshake: the legacy era (`initialize` with [`PROTOCOL_VERSION`], then
//!   `notifications/initialized`), which every server version speaks; results carry
//!   `structuredContent` from 2025-06-18 on.
//! - Requests run one at a time (`&mut self`) with integer ids; answers are matched by id, other
//!   messages (notifications, requests from the server, stale answers) are skipped.
//! - A timeout or the server's exit makes the client unusable ([`McpClient::is_usable`]); a
//!   JSON-RPC error or an unreadable line fails only the request.
//! - stderr is drained and discarded, and payloads are never logged.
//! - [`McpClient::shutdown`] closes stdin (the server exits at end of input), then kills.

use std::time::Duration;

use serde_json::{Map, Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{BusinessError, Result};
use crate::pi_family::rpc::{Frame, FrameReader, encode_frame};
use crate::process::{ChildHandle, ExecSpec, ProcessSpawner};
use crate::security::redact_text;

/// Protocol version sent in `initialize`.
pub const PROTOCOL_VERSION: &str = "2025-11-25";

/// Client name sent in `initialize`.
pub const CLIENT_NAME: &str = "harness-desktop";

/// Longest server error message kept, in characters.
const MAX_ERROR_CHARS: usize = 300;

/// How long [`McpClient::shutdown`] waits after closing stdin before killing.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Limits of one client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOptions {
    /// Longest wait for one answer (the handshake counts as one request per message).
    pub request_timeout: Duration,
    /// Largest accepted message, in bytes.
    pub max_response_bytes: usize,
    /// Version sent as `clientInfo.version`.
    pub client_version: String,
}

impl Default for McpOptions {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(15),
            max_response_bytes: 4 * 1024 * 1024,
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// One tool of a `tools/list` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTool {
    /// Its name.
    pub name: String,
    /// The property names of its `inputSchema`, as listed; `None` when it lists no `properties`
    /// object.
    pub arguments: Option<Vec<String>>,
}

impl McpTool {
    /// The tool in one `tools` entry; `None` without a name.
    fn from_listed(tool: &Value) -> Option<Self> {
        let name = tool.get("name").and_then(Value::as_str)?.to_owned();
        let arguments = tool
            .pointer("/inputSchema/properties")
            .and_then(Value::as_object)
            .map(|properties| properties.keys().cloned().collect());

        Some(Self { name, arguments })
    }
}

/// The text and structured parts of a `tools/call` result.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// `structuredContent`, when the server sent it.
    pub structured: Option<Value>,
    /// The `text` items of `content`, joined by newlines; `None` when there are none.
    pub text: Option<String>,
    /// The tool's `isError` flag.
    pub is_error: bool,
}

/// A connected server. See the module docs.
pub struct McpClient {
    stdin: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    reader: FrameReader<Box<dyn AsyncRead + Send + Unpin>>,
    child: Box<dyn ChildHandle>,
    options: McpOptions,
    next_id: u64,
    usable: bool,
    server_name: Option<String>,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("usable", &self.usable)
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

impl McpClient {
    /// Spawns `spec` and performs the handshake. The child is killed when this fails.
    pub async fn connect(spawner: &dyn ProcessSpawner, spec: &ExecSpec, options: McpOptions) -> Result<Self> {
        let process = spawner.spawn(spec)?;
        tokio::spawn(discard(process.stderr));

        let reader = FrameReader::with_limit(process.stdout, options.max_response_bytes);
        let mut client = Self {
            stdin: Some(process.stdin),
            reader,
            child: process.child,
            options,
            next_id: 0,
            usable: true,
            server_name: None,
        };

        match client.initialize().await {
            Ok(()) => Ok(client),
            Err(error) => {
                client.shutdown().await;
                Err(error)
            }
        }
    }

    /// False after a timeout or once the server stopped answering; the client should be dropped.
    pub fn is_usable(&self) -> bool {
        self.usable
    }

    /// `serverInfo.name` from the handshake.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    /// The names of the server's tools (`tools/list`, first page).
    pub async fn list_tools(&mut self) -> Result<Vec<String>> {
        Ok(self.tools().await?.into_iter().map(|tool| tool.name).collect())
    }

    /// The server's tools with the arguments they take (`tools/list`, first page).
    pub async fn tools(&mut self) -> Result<Vec<McpTool>> {
        let result = self.request("tools/list", json!({})).await?;

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .map(|tools| tools.iter().filter_map(McpTool::from_listed).collect())
            .unwrap_or_default();

        Ok(tools)
    }

    /// Calls tool `name` with `arguments` (`tools/call`).
    pub async fn call_tool(&mut self, name: &str, arguments: Map<String, Value>) -> Result<ToolOutput> {
        let result = self
            .request("tools/call", json!({ "name": name, "arguments": arguments }))
            .await?;

        let texts: Vec<&str> = result
            .get("content")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|item| item.get("text").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();

        Ok(ToolOutput {
            structured: result
                .get("structuredContent")
                .filter(|value| !value.is_null())
                .cloned(),
            text: (!texts.is_empty()).then(|| texts.join("\n")),
            is_error: result.get("isError").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// Sends request `method` and returns its `result`.
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        if !self.usable {
            return Err(BusinessError::Process("the MCP server is no longer running".to_owned()));
        }

        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await?;

        let waited = tokio::time::timeout(self.options.request_timeout, self.answer(id, method)).await;

        match waited {
            Ok(answer) => answer,
            Err(_elapsed) => {
                self.usable = false;
                Err(BusinessError::Timeout(format!(
                    "the MCP server did not answer {method} within {} s",
                    self.options.request_timeout.as_secs()
                )))
            }
        }
    }

    /// Sends notification `method` (no answer is expected).
    pub async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
            .await
    }

    /// Closes stdin, waits up to 2 s for the server to exit, then kills its process group.
    pub async fn shutdown(mut self) {
        self.usable = false;
        drop(self.stdin.take());

        if tokio::time::timeout(SHUTDOWN_GRACE, self.child.wait()).await.is_err() {
            self.child.start_kill().ok();
            self.child.wait().await.ok();
        }
    }

    async fn initialize(&mut self) -> Result<()> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": CLIENT_NAME, "version": self.options.client_version },
        });
        let result = self.request("initialize", params).await?;

        if !result.get("protocolVersion").is_some_and(Value::is_string) {
            return Err(BusinessError::Protocol(
                "the MCP server's initialize result has no protocolVersion".to_owned(),
            ));
        }

        self.server_name = result
            .pointer("/serverInfo/name")
            .and_then(Value::as_str)
            .map(str::to_owned);

        self.notify("notifications/initialized", json!({})).await
    }

    async fn send(&mut self, message: &Value) -> Result<()> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(BusinessError::Process("the MCP server's input is closed".to_owned()));
        };

        let written = async {
            stdin.write_all(&encode_frame(message)).await?;
            stdin.flush().await
        };

        if written.await.is_err() {
            self.usable = false;
            return Err(BusinessError::Process("the MCP server is no longer running".to_owned()));
        }

        Ok(())
    }

    /// Reads until the answer to request `id` arrives.
    async fn answer(&mut self, id: u64, method: &str) -> Result<Value> {
        loop {
            let Some(frame) = self.reader.next_frame().await else {
                self.usable = false;
                return Err(BusinessError::Process(format!(
                    "the MCP server exited before answering {method}"
                )));
            };

            let message = match frame {
                Frame::Json(message) => message,
                // Answers come in order, so the unreadable line is most likely this one's.
                Frame::Fault(fault) => {
                    return Err(BusinessError::Protocol(format!(
                        "the MCP server's answer to {method} could not be read: {}",
                        fault.error
                    )));
                }
            };

            if message.get("id").and_then(Value::as_u64) != Some(id) || message.get("method").is_some() {
                continue;
            }

            if let Some(error) = message.get("error") {
                return Err(rpc_error(method, error));
            }

            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}

/// A JSON-RPC error as a [`BusinessError::Protocol`] with the server's (redacted, clipped) message.
fn rpc_error(method: &str, error: &Value) -> BusinessError {
    let message = error.get("message").and_then(Value::as_str).unwrap_or("unknown error");
    let message = clean_error_text(message);
    let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);

    BusinessError::Protocol(format!("{method} failed ({code}): {message}"))
}

/// Error text from the server, redacted and cut to [`MAX_ERROR_CHARS`], fit to show in the UI.
pub(super) fn clean_error_text(text: &str) -> String {
    redact_text(text).chars().take(MAX_ERROR_CHARS).collect()
}

/// Reads `stream` to the end and drops everything.
async fn discard(mut stream: Box<dyn AsyncRead + Send + Unpin>) {
    let mut chunk = vec![0_u8; 8 * 1024];

    while matches!(stream.read(&mut chunk).await, Ok(read) if read > 0) {}
}
