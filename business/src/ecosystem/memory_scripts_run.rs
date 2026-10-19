//! Routine script runs (see "Script runs" in `docs/routines-format.md`): a saved Harness Memory
//! script run by name, with no arguments and no stdin.
//!
//! - Each run has its own `harness-memory mcp` connection ([`McpClient`], started like the pooled
//!   one of [`MemoryScripts`] and stopped after the run), so a long script never blocks the Memory
//!   view, another routine or the pooled client's 15 s requests, and stopping a run stops its
//!   server (and the script with it).
//! - `script_get` first: a version this machine must approve (`approval: "needed"`) is not run
//!   ([`ScriptRunResult::NeedsTrust`]). Otherwise `script_run`, waited for at most the script's own
//!   timeout ([`SCRIPT_TIMEOUT_MAX`] at most) plus [`RUN_TIMEOUT_MARGIN`]. When the script changed
//!   between the two calls the server asks the user the normal way, and a refusal is `NeedsTrust`
//!   too.
//! - A stop (see [`MemoryScripts::run_for_routine`]) cancels the request and closes the server's
//!   input: Harness Memory then kills the script with its process group and removes its file.
//!   A server that has not exited 2 s later is killed ([`McpClient::cancel`]).
//! - `project_path` is the routine's project root when it has one, so a repository script is found
//!   in that repository and a script runs in the project.
//!
//! Neither the script's body nor its output is logged; the caller redacts the output it keeps.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};

use super::mcp_client::{McpClient, McpOptions, ToolOutput};
use super::memory_scripts::{MemoryScripts, SCRIPT_GET_TOOL, SCRIPT_RUN_TOOL, fill_approval, tool_error_text};
use crate::env::Env;
use crate::model::ScriptScope;

/// The longest timeout Harness Memory gives a script (its `SCRIPT_TIMEOUT_MAX`).
pub const SCRIPT_TIMEOUT_MAX: Duration = Duration::from_secs(600);

/// How much longer than the script's own timeout a run is waited for.
pub const RUN_TIMEOUT_MARGIN: Duration = Duration::from_secs(30);

/// Longest wait for `script_get`.
const GET_TIMEOUT: Duration = Duration::from_secs(15);

/// What the server answers for a name it does not have (`no script named <name> here or in the
/// global scope`).
const MISSING_PREFIX: &str = "no script named";

/// The script a routine runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptCall<'a> {
    /// The script name.
    pub name: &'a str,
    /// Harness Memory's scope (`repo` or `global`).
    pub scope: ScriptScope,
    /// The routine's project root, `None` for a global routine.
    pub project: Option<&'a Path>,
}

impl ScriptCall<'_> {
    fn arguments(&self) -> Map<String, Value> {
        let mut arguments = Map::new();
        arguments.insert("name".to_owned(), json!(self.name));
        arguments.insert("scope".to_owned(), json!(self.scope));

        if let Some(project) = self.project {
            arguments.insert("project_path".to_owned(), json!(project.to_string_lossy()));
        }

        arguments
    }
}

/// How a routine script run went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptRunResult {
    /// Harness Memory is not installed, predates scripts, or its server did not start.
    Unavailable,
    /// No script of that name in that scope.
    NotFound,
    /// This machine must approve the script first (or the user declined it).
    NeedsTrust,
    /// Harness Memory failed the call: a short, redacted reason.
    Failed(String),
    /// No answer within the script's timeout plus [`RUN_TIMEOUT_MARGIN`].
    TimedOut,
    /// The run was stopped before it ended (see [`MemoryScripts::run_for_routine`]).
    Stopped,
    /// The script ran.
    Finished {
        /// Its exit code; `None` after a timeout or a signal.
        exit_code: Option<i32>,
        /// Harness Memory killed it at its timeout.
        timed_out: bool,
        /// Its stdout as the server returns it (capped, secrets masked).
        stdout: String,
        /// Its stderr as the server returns it (capped, secrets masked).
        stderr: String,
    },
}

impl MemoryScripts {
    /// Runs the script `call` names for a routine, on a connection of its own (see the module
    /// docs). When `stop` completes first the run is [`ScriptRunResult::Stopped`]: the request is
    /// cancelled and the server shut down gracefully, so it stops the script.
    pub async fn run_for_routine(
        &self,
        env: &Env,
        call: &ScriptCall<'_>,
        stop: impl Future<Output = ()>,
    ) -> ScriptRunResult {
        tokio::pin!(stop);

        let Ok(spec) = self.server(env).await else {
            return ScriptRunResult::Unavailable;
        };

        let options = McpOptions {
            request_timeout: SCRIPT_TIMEOUT_MAX + RUN_TIMEOUT_MARGIN,
            ..McpOptions::default()
        };

        // A server still starting is killed when its connection is dropped.
        let connected = tokio::select! {
            connected = McpClient::connect(self.spawner.as_ref(), &spec, options) => connected,
            () = &mut stop => return ScriptRunResult::Stopped,
        };

        let Ok(mut client) = connected else {
            return ScriptRunResult::Unavailable;
        };

        let result = tokio::select! {
            result = run_on(&mut client, call) => Some(result),
            () = &mut stop => None,
        };

        match result {
            Some(result) => {
                client.shutdown().await;
                result
            }
            None => {
                client.cancel().await;
                ScriptRunResult::Stopped
            }
        }
    }
}

/// `script_get`, then `script_run` unless the script needs approval.
async fn run_on(client: &mut McpClient, call: &ScriptCall<'_>) -> ScriptRunResult {
    let got = tokio::time::timeout(GET_TIMEOUT, client.call_tool(SCRIPT_GET_TOOL, call.arguments())).await;

    let output = match got {
        Ok(Ok(output)) => output,
        Ok(Err(_)) | Err(_) => return ScriptRunResult::Failed("Harness Memory did not answer".to_owned()),
    };

    if output.is_error {
        return refused(&output);
    }

    let Some(mut script) = output.structured.and_then(|data| data.get("script").cloned()) else {
        return ScriptRunResult::Failed("Harness Memory answered without the script".to_owned());
    };

    fill_approval(&mut script);

    if script["approval"] == "needed" {
        return ScriptRunResult::NeedsTrust;
    }

    let limit = script["timeoutSecs"]
        .as_u64()
        .map_or(SCRIPT_TIMEOUT_MAX, Duration::from_secs)
        .min(SCRIPT_TIMEOUT_MAX);

    let ran = tokio::time::timeout(
        limit + RUN_TIMEOUT_MARGIN,
        client.call_tool(SCRIPT_RUN_TOOL, call.arguments()),
    )
    .await;

    match ran {
        Ok(Ok(output)) if output.is_error => refused(&output),
        Ok(Ok(output)) => finished(output.structured.unwrap_or(Value::Null)),
        Ok(Err(_)) => ScriptRunResult::Failed("Harness Memory stopped during the run".to_owned()),
        Err(_) => ScriptRunResult::TimedOut,
    }
}

/// A tool error: a missing script, a script the user must approve (or declined), or another
/// failure with the server's (redacted, clipped) text.
fn refused(output: &ToolOutput) -> ScriptRunResult {
    let untrusted = output.structured.as_ref().is_some_and(|data| data["untrusted"] == true);

    if untrusted {
        return ScriptRunResult::NeedsTrust;
    }

    let text = tool_error_text(output);

    if text.trim_start().starts_with(MISSING_PREFIX) {
        return ScriptRunResult::NotFound;
    }

    ScriptRunResult::Failed(text)
}

/// The `structuredContent` of a `script_run` that ran.
fn finished(data: Value) -> ScriptRunResult {
    let text = |field: &str| data[field].as_str().unwrap_or_default().to_owned();

    ScriptRunResult::Finished {
        exit_code: data["exitCode"].as_i64().and_then(|code| i32::try_from(code).ok()),
        timed_out: data["timedOut"] == true,
        stdout: text("stdout"),
        stderr: text("stderr"),
    }
}
