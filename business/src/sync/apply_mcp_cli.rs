//! The MCP servers of an apply ([`super::apply_plan`]) whose file only the harness's CLI changes:
//! Claude Code's user scope, which every running Claude Code rewrites, and Codex's `config.toml`.
//! Each server is re-checked against the file, then added with the command line the review showed
//! (`claude mcp add-json --scope user <name> <json>` or `codex mcp add <name> …`, the arguments of
//! [`add_args`] with the time limit of [`MCP_CLI_TIMEOUT`]). A server already there is never
//! added: `add-json` refuses such a name too. After a failed run the file is read once more, and a
//! server that is there by then ends `alreadyPresent` or `conflict`. Nothing is retried.
//!
//! A server the review offered as `differentDefinition` (a replace item) whose entry still differs
//! is replaced ([`replace_entry`]): the entry is read now, and the CLI runs `remove` then `add` (the
//! old entry added back when the add fails). When the entry is an HTTP or SSE server at the same
//! endpoint as the shared server (see [`super::endpoint`]) its other settings (headers, OAuth) are
//! carried over, so the process list holds those values for the length of a run; for anything else
//! nothing of it is kept and the add is the fresh definition. A Codex entry that `codex mcp add`
//! cannot write back is never replaced. The argv and summary of the result never hold a value:
//! the argv is the review's add command, the summary a fixed text. One whose entry is gone is
//! added, any other change ends `conflict`.

use super::apply::{ApplyContext, outcome};
use super::apply_mcp::found_outcome;
use super::plan_presence::{McpFiles, Presence, Replace, mcp_presence, replace_of};
use super::plan_rows::{Step, StoredItem, mcp_definition};
use super::schema::McpServer;
use crate::ecosystem::MCP_CLI_TIMEOUT;
use crate::ecosystem::mcp_cli::{CliReplace, add_args, replace_entry};
use crate::ecosystem::mcp_merge::existing_entry;
use crate::ecosystem::setup::McpFile;
use crate::harness::HarnessCli;
use crate::model::{McpRefusal, SyncItemOutcome, SyncOutcome, SyncSkipReason};
use crate::security::redact_text;

/// Adds the server of `item` to `files` through the harness's CLI, unless it is there already
/// (see the module docs).
pub(super) async fn cli_server(context: ApplyContext<'_>, files: &McpFiles, item: &StoredItem) -> SyncItemOutcome {
    let Some(Step::Mcp(server)) = &item.step else {
        return outcome(item, SyncOutcome::Failed);
    };

    let harness = context.registry.get(item.harness);
    let binary = context.binaries.get(&item.harness);
    let (Some(harness), Some(binary)) = (harness, binary) else {
        return outcome(
            item,
            SyncOutcome::Skipped {
                reason: SyncSkipReason::HarnessNotInstalled,
            },
        );
    };

    let presence = mcp_presence(files, server);
    let replacing = item.replace && matches!(presence, Ok(Presence::Replaceable(_)));

    if !replacing && let Some(decided) = found_outcome(item, presence) {
        return decided;
    }

    let cli = HarnessCli {
        harness,
        env: context.env,
        runner: context.runner,
        binary,
    };

    if replacing {
        return replace_server(cli, files, item, server).await;
    }

    let Some(args) = add_args(files.shape, &server.name, &mcp_definition(server, files.shape)) else {
        return outcome(item, SyncOutcome::Failed);
    };

    match cli.run_args(args, MCP_CLI_TIMEOUT).await {
        Ok(result) if result.ok => SyncItemOutcome {
            summary: result.summary,
            argv: result.argv,
            ..outcome(item, SyncOutcome::Done)
        },
        Ok(result) => match found_outcome(item, mcp_presence(files, server)) {
            Some(found) if matches!(found.outcome, SyncOutcome::AlreadyPresent | SyncOutcome::Conflict) => found,
            _ => SyncItemOutcome {
                summary: result.summary,
                argv: result.argv,
                ..outcome(item, SyncOutcome::Failed)
            },
        },
        Err(error) => SyncItemOutcome {
            summary: Some(redact_text(&error.to_string())),
            argv: item.command.clone(),
            ..outcome(item, SyncOutcome::Failed)
        },
    }
}

/// The summary of a replace that ended well (a fixed text: the entry's values are never shown).
const KEPT_SUMMARY: &str = "Replaced. The server's other settings were kept.";

/// The summary of a replace of another endpoint.
const WHOLE_SUMMARY: &str = "Replaced. Nothing of the previous entry was kept.";

/// The summary of a replace whose runs failed.
const FAILED_SUMMARY: &str =
    "The harness's CLI could not replace this server. Its previous entry was left or put back.";

/// The summary of a replace that removed the entry and could add neither definition.
const REMOVED_SUMMARY: &str =
    "The server was removed to replace it, and neither the new nor the previous entry could be added. Add it again.";

/// Replaces the server of `item`, whose entry in `files` was found different (see the module
/// docs).
async fn replace_server(
    cli: HarnessCli<'_>,
    files: &McpFiles,
    item: &StoredItem,
    server: &McpServer,
) -> SyncItemOutcome {
    let existing = match existing_entry(&files.target, files.shape, &server.name) {
        Ok(Some(existing)) => existing,
        Ok(None) => return outcome(item, SyncOutcome::Conflict),
        Err(refusal) => {
            return outcome(
                item,
                SyncOutcome::Skipped {
                    reason: SyncSkipReason::McpFileRefused { refusal },
                },
            );
        }
    };

    let file = McpFile {
        path: files.target.clone(),
        also_read: Vec::new(),
        shape: files.shape,
    };
    // What is kept is decided on the entry read now.
    let Some(replace) = replace_of(files.shape, &server.name, &existing, server) else {
        return outcome(item, SyncOutcome::Conflict);
    };
    let fresh = mcp_definition(server, files.shape);
    let ended = |result: SyncOutcome, summary: &str| SyncItemOutcome {
        summary: Some(summary.to_owned()),
        argv: item.command.clone(),
        ..outcome(item, result)
    };

    match replace_entry(cli, &file, &server.name, &existing, &fresh, replace == Replace::Keep).await {
        CliReplace::Replaced if replace == Replace::Keep => ended(SyncOutcome::Replaced, KEPT_SUMMARY),
        CliReplace::Replaced => ended(SyncOutcome::Replaced, WHOLE_SUMMARY),
        CliReplace::Failed => ended(SyncOutcome::Failed, FAILED_SUMMARY),
        CliReplace::RemovedEntry => ended(SyncOutcome::Failed, REMOVED_SUMMARY),
        CliReplace::BatchShim => outcome(
            item,
            SyncOutcome::Skipped {
                reason: SyncSkipReason::McpFileRefused {
                    refusal: McpRefusal::CliBatchShim,
                },
            },
        ),
        CliReplace::CannotWrite => outcome(item, SyncOutcome::Conflict),
    }
}
