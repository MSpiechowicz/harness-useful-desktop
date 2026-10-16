//! Harness Dashboard data through `harness-dashboard mcp` (`dashboard_query`,
//! `dashboard_availability`).
//!
//! [`Dashboard`] keeps one [`McpClient`] behind a mutex and reuses it between queries; it shuts
//! the server down after [`DashboardOptions::idle_timeout`] without use, and starts it again on the
//! next query. The server runs in the home directory with `mcp` as its only argument: `--db` is
//! never passed (the dashboard finds its own database), the app never reads the database or its
//! `auth-token`, and `CLAUDE_CODE_SESSION_ID` is not passed on (it would pick a session for
//! `session_cost` when the app names none).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};
use tokio::sync::Mutex;

use super::catalog::DASHBOARD_BINARY;
use super::detect::resolve_companion;
use super::mcp_client::{McpClient, McpOptions, ToolOutput, clean_error_text};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{DashboardAvailability, DashboardQuery, DashboardResult, DashboardTool};
use crate::process::{ExecSpec, ProcessSpawner, ResolvedBinary, launcher_search_dirs};

/// How the pooled client behaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardOptions {
    /// The server stops after this long without a query (30 s).
    pub idle_timeout: Duration,
    /// Request timeout and response cap of the client.
    pub client: McpOptions,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(30),
            client: McpOptions::default(),
        }
    }
}

/// The `harness-dashboard` executable: `PATH` and the usual install directories (`~/.local/bin`
/// from `install.sh`), and on Windows `%LOCALAPPDATA%\Programs\harness-dashboard` (`install.ps1`).
pub fn resolve_dashboard(env: &Env) -> Option<ResolvedBinary> {
    resolve_companion(env, DASHBOARD_BINARY)
}

/// The pooled dashboard connection. See the module docs.
pub struct Dashboard {
    spawner: Arc<dyn ProcessSpawner>,
    options: DashboardOptions,
    slot: Arc<Mutex<Slot>>,
}

/// The running client, the binary it runs and a counter of uses (for the idle timer).
#[derive(Default)]
struct Slot {
    client: Option<(PathBuf, McpClient)>,
    uses: u64,
}

impl Dashboard {
    /// A pool that starts the server through `spawner`.
    pub fn new(spawner: Arc<dyn ProcessSpawner>, options: DashboardOptions) -> Self {
        Self {
            spawner,
            options,
            slot: Arc::new(Mutex::new(Slot::default())),
        }
    }

    /// Whether queries can be answered: the binary exists, the server starts, and it offers every
    /// [`DashboardTool`].
    pub async fn availability(&self, env: &Env) -> DashboardAvailability {
        let Some(binary) = resolve_dashboard(env) else {
            return DashboardAvailability::NotInstalled;
        };

        let tools = match self.exec(env, &binary.path, &Request::ListTools).await {
            Ok(Reply::Tools(tools)) => tools,
            Ok(Reply::Tool(_)) => Vec::new(),
            Err(error) => {
                return DashboardAvailability::Failed {
                    message: error.to_string(),
                };
            }
        };

        let missing: Vec<&str> = DashboardTool::ALL
            .into_iter()
            .map(DashboardTool::wire_name)
            .filter(|wire_name| !tools.iter().any(|name| name == wire_name))
            .collect();

        if missing.is_empty() {
            return DashboardAvailability::Available;
        }

        DashboardAvailability::Failed {
            message: format!(
                "This harness-dashboard lacks the tools {}; update it with `harness-dashboard update`.",
                missing.join(", ")
            ),
        }
    }

    /// Runs `query` after validating its arguments ([`DashboardQuery::parse`]). `notFound` when the
    /// dashboard is not installed; a tool that reports an error returns `isError` with its text.
    pub async fn query(&self, env: &Env, query: &DashboardQuery) -> Result<DashboardResult> {
        let args = query.parse()?;
        let tool = args.tool();
        let request = Request::CallTool {
            name: tool.wire_name(),
            arguments: args.to_arguments()?,
        };

        let binary =
            resolve_dashboard(env).ok_or_else(|| BusinessError::NotFound(format!("executable {DASHBOARD_BINARY}")))?;

        match self.exec(env, &binary.path, &request).await? {
            Reply::Tool(output) => Ok(dashboard_result(tool, output)),
            Reply::Tools(_) => Err(BusinessError::Internal("unexpected dashboard reply".to_owned())),
        }
    }

    /// Stops the server now (app shutdown, tests).
    pub async fn shutdown(&self) {
        let taken = self.slot.lock().await.client.take();

        if let Some((_, client)) = taken {
            client.shutdown().await;
        }
    }

    /// Sends `request` on the pooled client for `binary`, starting the server when there is none
    /// (or it runs another binary, or it stopped). When a reused client turns out to have exited,
    /// the server is started again and the request sent once more.
    async fn exec(&self, env: &Env, binary: &Path, request: &Request) -> Result<Reply> {
        let mut slot = self.slot.lock().await;

        let reusable = matches!(&slot.client, Some((path, client)) if path == binary && client.is_usable());
        if !reusable && let Some((_, stale)) = slot.client.take() {
            tokio::spawn(stale.shutdown());
        }

        let mut outcome = self.exec_once(&mut slot, env, binary, request).await;

        if reusable && matches!(outcome, Err(BusinessError::Process(_))) {
            outcome = self.exec_once(&mut slot, env, binary, request).await;
        }

        slot.uses += 1;
        self.schedule_idle_shutdown(slot.uses);

        outcome
    }

    async fn exec_once(&self, slot: &mut Slot, env: &Env, binary: &Path, request: &Request) -> Result<Reply> {
        if slot.client.as_ref().is_none_or(|(_, client)| !client.is_usable()) {
            let spec = server_spec(env, binary);
            let started = McpClient::connect(self.spawner.as_ref(), &spec, self.options.client.clone()).await?;
            slot.client = Some((binary.to_path_buf(), started));
        }

        let Some((_, client)) = slot.client.as_mut() else {
            return Err(BusinessError::Internal("the dashboard client is missing".to_owned()));
        };

        let outcome = match request {
            Request::ListTools => client.list_tools().await.map(Reply::Tools),
            Request::CallTool { name, arguments } => client.call_tool(name, arguments.clone()).await.map(Reply::Tool),
        };

        if !client.is_usable()
            && let Some((_, broken)) = slot.client.take()
        {
            tokio::spawn(broken.shutdown());
        }

        outcome
    }

    /// Shuts the server down after the idle timeout unless it was used again meanwhile.
    fn schedule_idle_shutdown(&self, uses: u64) {
        let slot = Arc::downgrade(&self.slot);
        let idle = self.options.idle_timeout;

        tokio::spawn(async move {
            tokio::time::sleep(idle).await;

            let Some(slot) = slot.upgrade() else {
                return;
            };

            let mut slot = slot.lock().await;
            if slot.uses != uses {
                return;
            }

            if let Some((_, client)) = slot.client.take() {
                drop(slot);
                client.shutdown().await;
            }
        });
    }
}

/// What [`Dashboard::exec`] sends.
enum Request {
    ListTools,
    CallTool {
        name: &'static str,
        arguments: Map<String, Value>,
    },
}

/// What [`Dashboard::exec`] gets back.
enum Reply {
    Tools(Vec<String>),
    Tool(ToolOutput),
}

/// `harness-dashboard mcp`, run in the home directory.
fn server_spec(env: &Env, binary: &Path) -> ExecSpec {
    ExecSpec::new(binary)
        .args(["mcp"])
        .cwd(&env.home)
        .env_remove_prefix("CLAUDE_CODE_SESSION_ID")
        .env("NO_COLOR", "1")
        .search_dirs(launcher_search_dirs(env, binary))
}

/// Maps a `tools/call` result: `structuredContent` (or the text parsed as JSON) as `data`, the
/// text only when there is no structured content; errors carry their text (redacted and clipped
/// like the server's JSON-RPC errors) and no data.
fn dashboard_result(tool: DashboardTool, output: ToolOutput) -> DashboardResult {
    if output.is_error {
        let text = output.text.map_or_else(
            || "The dashboard reported an error.".to_owned(),
            |text| clean_error_text(&text),
        );

        return DashboardResult {
            tool,
            data: Value::Null,
            text: Some(text),
            is_error: true,
        };
    }

    match output.structured {
        Some(data) => DashboardResult {
            tool,
            data,
            text: None,
            is_error: false,
        },
        None => {
            let data = output
                .text
                .as_deref()
                .and_then(|text| serde_json::from_str(text).ok())
                .unwrap_or(Value::Null);

            DashboardResult {
                tool,
                data,
                text: output.text,
                is_error: false,
            }
        }
    }
}
