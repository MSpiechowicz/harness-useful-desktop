//! Harness Dashboard data through `harness-dashboard mcp` (`dashboard_query`,
//! `dashboard_availability`).
//!
//! [`Dashboard`] keeps one pooled client ([`McpPool`]) and reuses it between queries; it shuts the
//! server down after [`DashboardOptions::idle_timeout`] without use, and starts it again on the
//! next query. The server runs in the home directory with `mcp` as its only argument: `--db` is
//! never passed (the dashboard finds its own database), the app never reads the database or its
//! `auth-token`, and `CLAUDE_CODE_SESSION_ID` is not passed on (it would pick a session for
//! `session_cost` when the app names none).

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::catalog::DASHBOARD_BINARY;
use super::detect::resolve_companion;
use super::mcp_client::{ToolOutput, clean_error_text};
use super::mcp_pool::{McpPool, PoolOptions};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{DashboardAvailability, DashboardQuery, DashboardResult, DashboardTool};
use crate::process::{ExecSpec, ProcessSpawner, ResolvedBinary, launcher_search_dirs};

/// How the pooled client behaves (the server stops after 30 s without a query).
pub type DashboardOptions = PoolOptions;

/// The `harness-dashboard` executable: `PATH` and the usual install directories (`~/.local/bin`
/// from `install.sh`), and on Windows `%LOCALAPPDATA%\Programs\harness-dashboard` (`install.ps1`).
pub fn resolve_dashboard(env: &Env) -> Option<ResolvedBinary> {
    resolve_companion(env, DASHBOARD_BINARY)
}

/// The pooled dashboard connection. See the module docs.
pub struct Dashboard {
    pool: McpPool,
}

impl Dashboard {
    /// A pool that starts the server through `spawner`.
    pub fn new(spawner: Arc<dyn ProcessSpawner>, options: DashboardOptions) -> Self {
        Self {
            pool: McpPool::new(spawner, options),
        }
    }

    /// Whether queries can be answered: the binary exists, the server starts, and it offers every
    /// [`DashboardTool`].
    pub async fn availability(&self, env: &Env) -> DashboardAvailability {
        let Some(binary) = resolve_dashboard(env) else {
            return DashboardAvailability::NotInstalled;
        };

        let tools = match self.pool.list_tools(&server_spec(env, &binary.path)).await {
            Ok(tools) => tools,
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
        let arguments = args.to_arguments()?;

        let binary =
            resolve_dashboard(env).ok_or_else(|| BusinessError::NotFound(format!("executable {DASHBOARD_BINARY}")))?;

        let output = self
            .pool
            .call_tool(&server_spec(env, &binary.path), tool.wire_name(), arguments)
            .await?;

        Ok(dashboard_result(tool, output))
    }

    /// Stops the server now (app shutdown, tests).
    pub async fn shutdown(&self) {
        self.pool.shutdown().await;
    }
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
