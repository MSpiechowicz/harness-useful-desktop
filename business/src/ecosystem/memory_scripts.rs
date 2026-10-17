//! The deterministic scripts agents keep in Harness Memory (`memory_scripts_list`,
//! `memory_script_get`, `memory_script_delete`, `memory_scripts_delete_all`).
//!
//! [`MemoryScripts`] lists one page of them with `harness-memory scripts list --json` (see
//! [`memory_cli`](super::memory_cli)), and reads and deletes one through the installed server
//! ([`resolve_memory`]) run in the home directory with `mcp` as its only argument, on one pooled
//! client ([`McpPool`], stopped after 30 s without use): `script_get` and `script_delete`, passing
//! the project root as `project_path` when there is one. The app never opens the memory database:
//! the server reads the local file or the team database as its own config says. Deleting every
//! script runs `harness-memory scripts delete --all` (see
//! [`memory_cli::delete_all`](super::memory_cli::delete_all)).
//!
//! Whether the server keeps scripts at all is probed once per binary (`tools/list` must offer
//! `script_list`) and cached until the binary at that path changes; a failed probe is not cached.
//! Each probe starts a fresh server, so a binary upgraded in place is not answered for by the
//! process of the old one. The same probe tells whether the server asks the user before running a
//! script it has not approved: its `script_run` takes no `sha` argument (the agent-side approval
//! of earlier builds), which is what makes `trust_team_scripts` a key it knows. A server whose
//! `script_run` is missing or lists no arguments cannot be told apart, so the config keys are left
//! as saved ([`ScriptsSupport::Unknown`]).
//!
//! A summary without `approval` (an older server) gets one from `trusted`: `approved` or
//! `needed`; so does one with a value this app does not know.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::catalog::MEMORY_BINARY;
use super::mcp_client::{ToolOutput, clean_error_text};
use super::mcp_pool::{McpPool, PoolOptions};
use super::memory_binary::resolve_memory;
use super::memory_cli::{
    CliCall, LIST_OUTPUT_BYTES, LIST_TIMEOUT, ListWindow, count_of, delete_all, list_availability, run_json,
};
use super::memory_remote::ScriptsSupport;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    MemoryDeleteAll, MemoryScriptsAvailability, MemoryScriptsPage, ScriptDetail, ScriptGroup, ScriptGroupCounts,
    ScriptSort, ScriptSummary,
};
use crate::process::{ExecSpec, ProcessRunner, ProcessSpawner};

/// The tool whose presence marks a server that keeps scripts.
pub const SCRIPT_LIST_TOOL: &str = "script_list";

/// The command that lists scripts, in messages.
const SCRIPTS_LIST_COMMAND: &str = "scripts list";

const SCRIPT_GET_TOOL: &str = "script_get";
const SCRIPT_RUN_TOOL: &str = "script_run";

/// The `script_run` argument of servers that let the agent approve a script.
const AGENT_APPROVAL_ARGUMENT: &str = "sha";

/// The `approval` values this app knows.
const APPROVALS: [&str; 4] = ["local", "approved", "team", "needed"];
const SCRIPT_DELETE_TOOL: &str = "script_delete";

/// The pooled `harness-memory mcp` connection for scripts. See the module docs.
pub struct MemoryScripts {
    pool: McpPool,
    probe: Mutex<Option<Probe>>,
}

/// Which scripts [`MemoryScripts::list`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptsQuery {
    /// The page and the search.
    pub window: ListWindow,
    /// One group only, or all.
    pub group: Option<ScriptGroup>,
    /// `updated` unless given.
    pub sort: Option<ScriptSort>,
}

/// A successful `tools/list` of one binary, as it was when probed.
#[derive(Clone, PartialEq, Eq)]
struct Probe {
    path: PathBuf,
    modified: Option<SystemTime>,
    has_scripts: bool,
    /// Whether `script_run` takes no `sha`; `None` without `script_run` or its argument list.
    asks_first: Option<bool>,
}

impl MemoryScripts {
    /// A pool that starts the server through `spawner`.
    pub fn new(spawner: Arc<dyn ProcessSpawner>, options: PoolOptions) -> Self {
        Self {
            pool: McpPool::new(spawner, options),
            probe: Mutex::new(None),
        }
    }

    /// Whether scripts can be read: the binary exists, the server starts, and it offers
    /// `script_list`.
    pub async fn availability(&self, env: &Env) -> MemoryScriptsAvailability {
        match self.server(env).await {
            Ok(_) => MemoryScriptsAvailability::Available,
            Err(availability) => availability,
        }
    }

    /// Whether the installed server keeps scripts, so `scripts = "local"` may be saved in its
    /// config file, and asks before running one, so `trust_team_scripts` may be saved too (an
    /// older server refuses those keys). `Unknown` when it is not found, the probe failed, or its
    /// `script_run` does not tell (see the module docs): the saved keys are then left as they are.
    pub async fn support(&self, env: &Env) -> ScriptsSupport {
        match self.probed(env).await {
            Ok((_, probe)) => match probe.asks_first {
                Some(true) => ScriptsSupport::Approvals,
                Some(false) => ScriptsSupport::Supported,
                None => ScriptsSupport::Unknown,
            },
            Err(MemoryScriptsAvailability::Outdated) => ScriptsSupport::Unsupported,
            Err(_) => ScriptsSupport::Unknown,
        }
    }

    /// One page of the scripts of `project` (a project root; `None` for the global ones only) and
    /// the global ones, from `harness-memory scripts list --json` (see [`memory_cli`](super::memory_cli)).
    /// Sorted with `--sort name|language|scope|place|runs|lastRun|updated` (`updated` unless
    /// asked). Never fails: what went wrong is the page's availability, with no scripts. A server
    /// that keeps scripts but does not ask before running one, has no paged list, or does not know
    /// the sort, is `outdated`.
    pub async fn list(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        project: Option<&Path>,
        query: &ScriptsQuery,
    ) -> MemoryScriptsPage {
        let unavailable = |availability| MemoryScriptsPage {
            availability,
            total: 0,
            groups: ScriptGroupCounts::default(),
            scripts: Vec::new(),
        };

        let server = match self.cli_server(env).await {
            Ok(server) => server,
            Err(availability) => return unavailable(availability),
        };

        let group = query.group.map(|group| match group {
            ScriptGroup::NeedsReview => "needsReview",
            ScriptGroup::Repo => "repo",
            ScriptGroup::Global => "global",
        });
        let sort = match query.sort {
            Some(ScriptSort::Name) => "name",
            Some(ScriptSort::Language) => "language",
            Some(ScriptSort::Scope) => "scope",
            Some(ScriptSort::Place) => "place",
            Some(ScriptSort::Runs) => "runs",
            Some(ScriptSort::LastRun) => "lastRun",
            Some(ScriptSort::Updated) | None => "updated",
        };

        let call = CliCall {
            name: SCRIPTS_LIST_COMMAND,
            args: [OsString::from("scripts")]
                .into_iter()
                .chain(
                    query
                        .window
                        .list_args(group.map(|group| ("--group", group)), sort, project),
                )
                .collect(),
            timeout: LIST_TIMEOUT,
            output_bytes: LIST_OUTPUT_BYTES,
        };

        match run_json(env, runner, &server, call).await {
            Ok(data) => scripts_page(&data),
            Err(error) => unavailable(list_availability(error)),
        }
    }

    /// The installed server's path when it asks before running a script, the generation whose
    /// `notes` and `scripts list` commands take pages: otherwise `outdated`, or why it cannot be
    /// used.
    pub(crate) async fn cli_server(&self, env: &Env) -> std::result::Result<PathBuf, MemoryScriptsAvailability> {
        let (_, probe) = self.probed(env).await?;

        match probe.asks_first {
            Some(true) => Ok(probe.path),
            _ => Err(MemoryScriptsAvailability::Outdated),
        }
    }

    /// Script `id` with its body. `notFound` when it is gone or the server is not installed,
    /// `unsupported` when the server predates scripts, `process` when the server fails it.
    pub async fn get(&self, env: &Env, id: i64, project: Option<&Path>) -> Result<ScriptDetail> {
        let data = self.call_by_id(env, SCRIPT_GET_TOOL, id, project).await?;

        let script = data
            .get("script")
            .cloned()
            .ok_or_else(|| missing_field(SCRIPT_GET_TOOL, "script"))?;

        parse(script, SCRIPT_GET_TOOL)
    }

    /// Deletes script `id`. `notFound` when it is gone or the server is not installed,
    /// `unsupported` when the server predates scripts, `process` when the server fails it.
    pub async fn delete(&self, env: &Env, id: i64, project: Option<&Path>) -> Result<()> {
        let data = self.call_by_id(env, SCRIPT_DELETE_TOOL, id, project).await?;

        match data.get("deleted").and_then(Value::as_bool) {
            Some(true) => Ok(()),
            Some(false) => Err(BusinessError::NotFound(format!("script {id}"))),
            None => Err(missing_field(SCRIPT_DELETE_TOOL, "deleted")),
        }
    }

    /// Deletes every script [`Self::list`] shows for `project` without filters (all groups and
    /// pages): how many, and how many of them were in the team database. `notFound` when the
    /// server is not installed, `unsupported` when it predates the paged list or deleting them
    /// all, `process` with its (redacted) line when it fails.
    pub async fn delete_all(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        project: Option<&Path>,
    ) -> Result<MemoryDeleteAll> {
        let server = self
            .cli_server(env)
            .await
            .map_err(|availability| unavailable_error(availability, "scripts"))?;

        delete_all(env, runner, &server, "scripts", project).await
    }

    /// Stops the server now (app shutdown, tests).
    pub async fn shutdown(&self) {
        self.pool.shutdown().await;
    }

    /// Calls `tool` with `{id, project_path}` and returns its structured content. A tool error is
    /// `notFound` when the server says the script is gone, otherwise `process` with its (redacted,
    /// clipped) text.
    async fn call_by_id(&self, env: &Env, tool: &str, id: i64, project: Option<&Path>) -> Result<Value> {
        let spec = self
            .server(env)
            .await
            .map_err(|availability| unavailable_error(availability, "scripts"))?;

        let mut arguments = Map::new();
        arguments.insert("id".to_owned(), json!(id));
        let arguments = with_project(arguments, project);

        let output = self.pool.call_tool(&spec, tool, arguments).await?;

        if output.is_error {
            let text = tool_error_text(&output);

            if text.trim() == not_found_text(id) {
                return Err(BusinessError::NotFound(text));
            }

            return Err(BusinessError::Process(format!("{tool}: {text}")));
        }

        structured(output, tool)
    }

    /// The server's spec when it keeps scripts, or why it cannot be used.
    async fn server(&self, env: &Env) -> std::result::Result<ExecSpec, MemoryScriptsAvailability> {
        self.probed(env).await.map(|(spec, _)| spec)
    }

    /// The server's spec and what its tools say, when it keeps scripts.
    async fn probed(&self, env: &Env) -> std::result::Result<(ExecSpec, Probe), MemoryScriptsAvailability> {
        let Some(binary) = resolve_memory(env) else {
            return Err(MemoryScriptsAvailability::NotInstalled);
        };

        let spec = server_spec(env, &binary.path);
        let modified = std::fs::metadata(&binary.path)
            .and_then(|metadata| metadata.modified())
            .ok();

        let probe = match self.cached(&binary.path, modified) {
            Some(probe) => probe,
            None => {
                // A pooled server may run the binary as it was before an in-place upgrade.
                self.pool.shutdown().await;

                let tools = self
                    .pool
                    .tools(&spec)
                    .await
                    .map_err(|error| MemoryScriptsAvailability::Failed {
                        message: error.to_string(),
                    })?;

                let probe = Probe {
                    path: binary.path.clone(),
                    modified,
                    has_scripts: tools.iter().any(|tool| tool.name == SCRIPT_LIST_TOOL),
                    asks_first: tools
                        .iter()
                        .find(|tool| tool.name == SCRIPT_RUN_TOOL)
                        .and_then(|tool| tool.arguments.as_ref())
                        .map(|arguments| !arguments.iter().any(|name| name == AGENT_APPROVAL_ARGUMENT)),
                };
                self.remember(probe.clone());

                probe
            }
        };

        if !probe.has_scripts {
            return Err(MemoryScriptsAvailability::Outdated);
        }

        Ok((spec, probe))
    }

    fn cached(&self, path: &Path, modified: Option<SystemTime>) -> Option<Probe> {
        let probe = self.probe.lock().unwrap_or_else(PoisonError::into_inner);

        probe
            .as_ref()
            .filter(|probe| probe.path == path && probe.modified == modified && modified.is_some())
            .cloned()
    }

    fn remember(&self, probe: Probe) {
        *self.probe.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe);
    }
}

/// What `harness-memory mcp` answers for a script id it does not have (`script_get`,
/// `script_delete`).
fn not_found_text(id: i64) -> String {
    format!("script {id} not found")
}

/// `harness-memory mcp`, run in the home directory (the project comes as `project_path`).
fn server_spec(env: &Env, binary: &Path) -> ExecSpec {
    ExecSpec::new(binary).args(["mcp"]).cwd(&env.home).env("NO_COLOR", "1")
}

/// `arguments` with `project_path` when there is a project.
fn with_project(mut arguments: Map<String, Value>, project: Option<&Path>) -> Map<String, Value> {
    if let Some(project) = project {
        arguments.insert("project_path".to_owned(), json!(project.to_string_lossy()));
    }

    arguments
}

/// The error for a call while `what` (`scripts`, `the notes view`) cannot be read.
pub(crate) fn unavailable_error(availability: MemoryScriptsAvailability, what: &str) -> BusinessError {
    match availability {
        MemoryScriptsAvailability::NotInstalled => BusinessError::NotFound(format!("executable {MEMORY_BINARY}")),
        MemoryScriptsAvailability::Outdated => {
            BusinessError::Unsupported(format!("this {MEMORY_BINARY} predates {what}; update Harness Memory"))
        }
        MemoryScriptsAvailability::Failed { message } => BusinessError::Process(message),
        MemoryScriptsAvailability::Available => BusinessError::Internal(format!("{what} can be read")),
    }
}

/// The `structuredContent` of a successful call; a tool error becomes a protocol error with its
/// (redacted, clipped) text.
fn structured(output: ToolOutput, tool: &str) -> Result<Value> {
    if output.is_error {
        return Err(BusinessError::Protocol(format!("{tool}: {}", tool_error_text(&output))));
    }

    output
        .structured
        .ok_or_else(|| BusinessError::Protocol(format!("{tool} answered without structured content")))
}

fn tool_error_text(output: &ToolOutput) -> String {
    output
        .text
        .as_deref()
        .map_or_else(|| "Harness Memory reported an error.".to_owned(), clean_error_text)
}

/// The page in a `scripts list --json` answer; an entry this app cannot read is skipped (logged).
/// A server that does not say `total` has the scripts it sent.
fn scripts_page(data: &Value) -> MemoryScriptsPage {
    let entries = data
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let scripts: Vec<ScriptSummary> = entries
        .into_iter()
        .filter_map(|entry| match parse::<ScriptSummary>(entry, SCRIPTS_LIST_COMMAND) {
            Ok(summary) => Some(summary),
            Err(_) => {
                tracing::warn!("memory scripts: an unreadable scripts list entry was skipped");
                None
            }
        })
        .collect();

    let groups = data.get("groups");
    let group = |name: &str| count_of(groups.and_then(|groups| groups.get(name))).unwrap_or(0);

    MemoryScriptsPage {
        availability: MemoryScriptsAvailability::Available,
        total: count_of(data.get("total")).unwrap_or(u32::try_from(scripts.len()).unwrap_or(u32::MAX)),
        groups: ScriptGroupCounts {
            needs_review: group("needsReview"),
            repo: group("repo"),
            global: group("global"),
        },
        scripts,
    }
}

/// `value` as a script DTO, with `approval` filled in (see the module docs). The parser's message
/// is dropped: it can quote the script's fields.
pub(crate) fn parse<T: DeserializeOwned>(mut value: Value, tool: &str) -> Result<T> {
    fill_approval(&mut value);

    serde_json::from_value(value).map_err(|_| BusinessError::Protocol(format!("{tool} answered an unreadable script")))
}

/// Sets a missing or unknown `approval` from `trusted` (`approved` or `needed`).
fn fill_approval(value: &mut Value) {
    let Some(fields) = value.as_object_mut() else {
        return;
    };

    let known = fields
        .get("approval")
        .and_then(Value::as_str)
        .is_some_and(|approval| APPROVALS.contains(&approval));

    if known {
        return;
    }

    let derived = match fields.get("trusted").and_then(Value::as_bool) {
        Some(true) => "approved",
        _ => "needed",
    };
    fields.insert("approval".to_owned(), json!(derived));
}

fn missing_field(tool: &str, field: &str) -> BusinessError {
    BusinessError::Protocol(format!("{tool} answered without `{field}`"))
}
