//! The companion MCP servers (the dashboard's, the memory server's and the app's own) in a file
//! only the harness's CLI changes: Claude Code's user scope, the top-level `mcpServers` of its
//! global file ([`Shape::ClaudeCli`], [`crate::harness::McpTarget::ClaudeCli`]), rewritten by every
//! running Claude Code, and Codex's `config.toml` ([`Shape::CodexCli`],
//! [`crate::harness::McpTarget::CodexCli`]), TOML with the user's own formatting and comments. The
//! app never writes either.
//!
//! - Decisions: the same as for the other files ([`super::mcp_register`],
//!   [`super::memory_register`], [`super::desktop_register`]): found by name or by the binary it
//!   runs, an entry the user owns never changed, a removed entry not added back, an owned entry
//!   following the program. They are made on a read of the file.
//! - Changes: what a decision would have written, as CLI runs ([`add_args`], [`remove_args`]). On
//!   Claude Code a new server is `claude mcp add-json --scope user <name> <json>` with `{"type":
//!   "stdio", "command": "<absolute path>", "args": ["mcp"]}` and a removed one `claude mcp remove
//!   --scope user <name>`; `add-json` refuses a name the user scope already has, so nothing is ever
//!   overwritten. On Codex a new server is `codex mcp add <name> -- <absolute path> mcp` (or
//!   `--url <url>`) and a removed one `codex mcp remove <name>`. The decisions only add names the
//!   file does not have. A changed server is removed and added again (and added back as it was
//!   when that fails).
//! - `codex mcp add` takes only a command and its arguments, or a URL and a bearer token variable:
//!   a change whose old or new entry holds anything else (`env` values, `http_headers`, `enabled`,
//!   timeouts) is refused as `CliFailed` before anything runs, so an entry with literal secrets is
//!   never rewritten.
//! - Each run has a time limit of [`MCP_CLI_TIMEOUT`], runs without a shell and with the harness's
//!   spawn environment ([`HarnessCli`]). When one fails, the file is read once more: a decision
//!   that no longer needs a change (the server appeared meanwhile) is reported, anything else is
//!   refused as `CliFailed`. Nothing is retried, so a registration at launch never loops.
//! - A change that removed an entry and could add neither the new nor the old one back is refused
//!   as `CliRemovedEntry` without that second read (it would take the missing entry for one the
//!   user removed).
//! - A server whose name starts with `-` is never changed (the CLI would read it as an option, and
//!   `-h` prints help with exit 0): the change is refused as `CliFailed` before anything runs.
//! - `codex mcp add` silently replaces a server of the same name, so the file is read again right
//!   before each add: a server of that name that appeared meanwhile (the user, or a running
//!   harness, wrote it) is never overwritten, the change stops there as a failed run.
//! - A batch-shim CLI (`claude.cmd`, an npm install on Windows) cannot take the JSON argument of
//!   `add-json`: such a change is refused as `CliBatchShim` before anything runs, so no entry is
//!   removed and left out.
//!
//! Logs name the harness, the command and the server, with the CLI's redacted last line; never a
//! file's contents.

use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value};

use super::catalog::DASHBOARD_BINARY;
use super::dashboard::resolve_dashboard;
use super::desktop_register::{DesktopRegistration, desktop_change, desktop_outcome};
use super::mcp_merge::{Blocked, Change, Shape, Target, load, servers};
use super::mcp_register::{dashboard_change, dashboard_outcome};
use super::memory_register::{MemoryRegistration, memory_change, memory_outcome, memory_removal, target_only};
use super::setup::McpFile;
use crate::harness::HarnessCli;
use crate::model::{McpRefusal, McpState};
use crate::security::{is_safe_arg, redact_text};

/// Time limit of one `claude mcp` run (it only edits the file, so this is generous).
pub const MCP_CLI_TIMEOUT: Duration = Duration::from_secs(60);

/// [`super::register_dashboard_mcp`] through the CLI: the entry runs the resolved
/// `harness-dashboard` by its absolute path (its name when it does not resolve).
pub async fn register_dashboard_mcp_cli(cli: HarnessCli<'_>, file: &McpFile) -> McpState {
    let program = resolve_dashboard(cli.env).map_or_else(
        || DASHBOARD_BINARY.to_owned(),
        |binary| binary.path.to_string_lossy().into_owned(),
    );
    let entry = file.shape.entry(&program);

    let changed = change_via_cli(cli, file, |target| dashboard_change(cli.env, file, target, &entry)).await;

    dashboard_outcome(cli.env, file, &entry, changed)
}

/// [`super::register_memory_mcp`] through the CLI.
pub async fn register_memory_mcp_cli(
    cli: HarnessCli<'_>,
    file: &McpFile,
    command: &Path,
    recorded: bool,
) -> MemoryRegistration {
    let command = command.to_string_lossy();
    let changed = change_via_cli(cli, file, |target| memory_change(file, target, &command, recorded)).await;

    memory_outcome(changed)
}

/// [`super::unregister_memory_mcp`] through the CLI.
pub async fn unregister_memory_mcp_cli(cli: HarnessCli<'_>, file: &McpFile) -> Result<bool, Blocked> {
    let target_only = target_only(file);

    change_via_cli(cli, file, |target| memory_removal(&target_only, target)).await
}

/// [`super::register_desktop_mcp`] through the CLI.
pub async fn register_desktop_mcp_cli(
    cli: HarnessCli<'_>,
    file: &McpFile,
    command: &Path,
    recorded: bool,
    previous: Option<&str>,
) -> DesktopRegistration {
    let command = command.to_string_lossy();
    let changed = change_via_cli(cli, file, |target| {
        desktop_change(file, target, &command, recorded, previous)
    })
    .await;

    desktop_outcome(changed)
}

/// Reads `file`, lets `decide` keep it or change it, and runs the change as CLI commands (see the
/// module docs). Reads the file; call it where a short blocking read is fine.
async fn change_via_cli<R>(
    cli: HarnessCli<'_>,
    file: &McpFile,
    mut decide: impl FnMut(&Target) -> Change<R>,
) -> Result<R, Blocked> {
    let target = load(&file.path, file.shape).map_err(Blocked::Refused)?;
    let change = decide(&target);

    let (document, value) = match change {
        Change::Keep(value) => return Ok(value),
        Change::Write(document, value) => (document, value),
    };

    let edits = edits(&target.document(), &document, file.shape);

    let Some(runs) = edits
        .iter()
        .map(|edit| edit_args(edit, file.shape))
        .collect::<Option<Vec<_>>>()
    else {
        tracing::warn!(
            harness = ?cli.harness.id(),
            "MCP entry not changed: the harness's CLI cannot write it as it is"
        );
        return Err(Blocked::Refused(McpRefusal::CliFailed));
    };

    if cli.binary.is_batch_script && !runs.iter().all(|args| batch_safe(args)) {
        return Err(Blocked::Refused(McpRefusal::CliBatchShim));
    }

    match apply(cli, &edits, file).await {
        Applied::All => return Ok(value),
        Applied::RemovedEntry => return Err(Blocked::Refused(McpRefusal::CliRemovedEntry)),
        Applied::Failed => {}
    }

    let again = load(&file.path, file.shape).map_err(Blocked::Refused)?;

    match decide(&again) {
        Change::Keep(value) => Ok(value),
        Change::Write(..) => Err(Blocked::Refused(McpRefusal::CliFailed)),
    }
}

/// One server change.
#[derive(Debug, Clone, PartialEq)]
enum Edit {
    Add { name: String, definition: Value },
    Remove { name: String },
    Replace { name: String, old: Value, new: Value },
}

/// The server changes that turn the servers of `before` into those of `after`.
fn edits(before: &Map<String, Value>, after: &Map<String, Value>, shape: Shape) -> Vec<Edit> {
    let before = servers(before, shape).unwrap_or_default();
    let after = servers(after, shape).unwrap_or_default();

    let removed = before
        .keys()
        .filter(|name| !after.contains_key(*name))
        .map(|name| Edit::Remove { name: name.clone() });

    let added_or_changed = after.iter().filter_map(|(name, new)| match before.get(name) {
        None => Some(Edit::Add {
            name: name.clone(),
            definition: new.clone(),
        }),
        Some(old) if old != new => Some(Edit::Replace {
            name: name.clone(),
            old: old.clone(),
            new: new.clone(),
        }),
        Some(_) => None,
    });

    removed.chain(added_or_changed).collect()
}

/// How far [`apply`] got.
enum Applied {
    /// Every edit succeeded.
    All,
    /// An edit failed and the file is as it was, or holds the edits before it.
    Failed,
    /// A replace removed its entry and could add neither definition back.
    RemovedEntry,
}

/// Runs `edits` of `file` in order, stopping at the first that fails.
async fn apply(cli: HarnessCli<'_>, edits: &[Edit], file: &McpFile) -> Applied {
    for edit in edits {
        let applied = match edit {
            Edit::Add { name, definition } => done(add(cli, file, name, definition).await),
            Edit::Remove { name } => done(remove(cli, file.shape, name).await),
            Edit::Replace { name, old, new } => replace(cli, file, name, old, new).await,
        };

        if !matches!(applied, Applied::All) {
            return applied;
        }
    }

    Applied::All
}

fn done(ok: bool) -> Applied {
    if ok { Applied::All } else { Applied::Failed }
}

/// Removes `name` and adds it as `new`; when the add fails, adds it back as `old`. A server of that
/// name that appeared meanwhile is left as it is.
async fn replace(cli: HarnessCli<'_>, file: &McpFile, name: &str, old: &Value, new: &Value) -> Applied {
    if !remove(cli, file.shape, name).await {
        return Applied::Failed;
    }

    if add(cli, file, name, new).await {
        return Applied::All;
    }

    if present(file, name) || add(cli, file, name, old).await {
        return Applied::Failed;
    }

    tracing::warn!(
        harness = ?cli.harness.id(),
        server = name,
        "MCP entry removed to change it, and neither the new nor the old one could be added"
    );
    Applied::RemovedEntry
}

/// The CLI arguments of `edit`'s runs in a `shape` file (a replace: the remove and both adds);
/// `None` when the CLI cannot write one of its entries (see [`add_args`]).
fn edit_args(edit: &Edit, shape: Shape) -> Option<Vec<String>> {
    match edit {
        Edit::Add { name, definition } => add_args(shape, name, definition),
        Edit::Remove { name } => remove_args(shape, name),
        Edit::Replace { name, old, new } => Some(
            [
                remove_args(shape, name)?,
                add_args(shape, name, new)?,
                add_args(shape, name, old)?,
            ]
            .concat(),
        ),
    }
}

/// Whether a batch shim takes `args` (the rule of [`HarnessCli::run_args`]).
fn batch_safe(args: &[String]) -> bool {
    args.iter().all(|arg| is_safe_arg(arg.trim_start_matches('-')))
}

/// The arguments (after the program name) that add the server `name` with `definition` to the
/// user scope: `mcp add-json --scope user <name> <json>`. Share setup imports servers with them
/// too.
pub fn add_json_args(name: &str, definition: &Value) -> Vec<String> {
    vec![
        "mcp".to_owned(),
        "add-json".to_owned(),
        "--scope".to_owned(),
        "user".to_owned(),
        name.to_owned(),
        Value::to_string(definition),
    ]
}

/// The arguments (after the program name) that add the server `name` with `definition` through
/// the CLI of a `shape` file: [`add_json_args`] for Claude Code, [`codex_add_args`] for Codex.
/// `None` when that CLI cannot write the entry as it is, the name would read as an option, or the
/// file is not changed by a CLI.
pub fn add_args(shape: Shape, name: &str, definition: &Value) -> Option<Vec<String>> {
    if !is_cli_name(name) {
        return None;
    }

    match shape {
        Shape::ClaudeCli => Some(add_json_args(name, definition)),
        Shape::CodexCli => codex_add_args(name, definition),
        Shape::McpServers | Shape::OpenCode | Shape::OpenCodeV2 => None,
    }
}

/// The arguments that remove the server `name` through the CLI of a `shape` file: `mcp remove
/// --scope user <name>` for Claude Code, `mcp remove <name>` for Codex. `None` when the name would
/// read as an option.
pub fn remove_args(shape: Shape, name: &str) -> Option<Vec<String>> {
    if !is_cli_name(name) {
        return None;
    }

    let mut args = vec!["mcp".to_owned(), "remove".to_owned()];

    if shape == Shape::ClaudeCli {
        args.extend(["--scope".to_owned(), "user".to_owned()]);
    }

    args.push(name.to_owned());
    Some(args)
}

/// Whether a server name can stand as a CLI argument: one starting with `-` would be read as an
/// option (`codex mcp remove -h` prints help and exits 0, a removal that never happened).
fn is_cli_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('-')
}

/// `mcp add <name> -- <command> <args…>` for a stdio entry, `mcp add <name> --url <url>
/// [--bearer-token-env-var <name>]` for a streamable HTTP one. `None` for an entry holding any other
/// field (`env`, `http_headers`, `enabled`, timeouts, ...), which `codex mcp add` cannot write: the
/// entry would lose it, and its values may be secrets.
pub fn codex_add_args(name: &str, definition: &Value) -> Option<Vec<String>> {
    let fields = definition.as_object()?;
    let text = |key: &str| fields.get(key).and_then(Value::as_str);
    let mut args = vec!["mcp".to_owned(), "add".to_owned(), name.to_owned()];

    match (text("command"), text("url")) {
        (Some(command), None) => {
            let known = fields
                .iter()
                .all(|(key, value)| matches!(key.as_str(), "command" | "args") || (key == "type" && value == "stdio"));
            let words: Vec<String> = match fields.get("args") {
                None => Vec::new(),
                Some(Value::Array(words)) => words
                    .iter()
                    .map(|word| word.as_str().map(str::to_owned))
                    .collect::<Option<_>>()?,
                Some(_) => return None,
            };

            if !known {
                return None;
            }

            args.push("--".to_owned());
            args.push(command.to_owned());
            args.extend(words);
        }
        (None, Some(url)) => {
            let known = fields.iter().all(|(key, value)| {
                matches!(key.as_str(), "url" | "bearer_token_env_var")
                    || (key == "type" && (value == "http" || value == "streamable-http"))
            });

            if !known {
                return None;
            }

            args.extend(["--url".to_owned(), url.to_owned()]);

            if let Some(variable) = text("bearer_token_env_var") {
                args.extend(["--bearer-token-env-var".to_owned(), variable.to_owned()]);
            }
        }
        _ => return None,
    }

    Some(args)
}

/// Adds `name` unless the file has such a server by now (see the module docs).
async fn add(cli: HarnessCli<'_>, file: &McpFile, name: &str, definition: &Value) -> bool {
    let Some(args) = add_args(file.shape, name, definition) else {
        return false;
    };

    if present(file, name) {
        tracing::warn!(
            harness = ?cli.harness.id(),
            server = name,
            "MCP entry not added: a server of that name appeared meanwhile"
        );
        return false;
    }

    run(cli, args, name).await
}

async fn remove(cli: HarnessCli<'_>, shape: Shape, name: &str) -> bool {
    match remove_args(shape, name) {
        Some(args) => run(cli, args, name).await,
        None => false,
    }
}

/// Whether `file`, read now, has a server `name` (or cannot be read, so nothing is added blindly).
fn present(file: &McpFile, name: &str) -> bool {
    load(&file.path, file.shape).map_or(true, |target| {
        servers(&target.document(), file.shape).is_some_and(|servers| servers.contains_key(name))
    })
}

/// Runs `args` for the server `server`; true when the CLI reports success. Logs the outcome (see
/// the module docs).
async fn run(cli: HarnessCli<'_>, args: Vec<String>, server: &str) -> bool {
    let harness = cli.harness.id();
    let verb = args.get(1).cloned().unwrap_or_default();

    match cli.run_args(args, MCP_CLI_TIMEOUT).await {
        Ok(result) if result.ok => {
            tracing::info!(?harness, verb = %verb, server = %server, "MCP entry changed through the CLI");
            true
        }
        Ok(result) => {
            tracing::warn!(
                ?harness,
                verb = %verb,
                server = %server,
                exit_code = ?result.exit_code,
                timed_out = result.timed_out,
                summary = result.summary.unwrap_or_default(),
                "MCP change through the CLI failed"
            );
            false
        }
        Err(error) => {
            tracing::warn!(
                ?harness,
                verb = %verb,
                server = %server,
                error = redact_text(&error.to_string()),
                "MCP change through the CLI could not run"
            );
            false
        }
    }
}
