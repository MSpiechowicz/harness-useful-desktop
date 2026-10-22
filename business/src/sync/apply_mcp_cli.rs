//! The MCP servers of an apply ([`super::apply_plan`]) whose file only the harness's CLI changes:
//! Claude Code's user scope, which every running Claude Code rewrites. Each server is re-checked
//! against the file, then added with the command line the review showed (`claude mcp add-json
//! --scope user <name> <json>`, the arguments of [`add_json_args`] with the time limit of
//! [`MCP_CLI_TIMEOUT`]). `add-json` refuses a name the user scope already has, so nothing is ever
//! overwritten; after a failed run the file is read once more, and a server that is there by then
//! ends `alreadyPresent` or `conflict`. Nothing is retried.

use super::apply::{ApplyContext, outcome};
use super::apply_mcp::found_outcome;
use super::plan_presence::{McpFiles, mcp_presence};
use super::plan_rows::{Step, StoredItem, mcp_definition};
use crate::ecosystem::MCP_CLI_TIMEOUT;
use crate::ecosystem::mcp_cli::add_json_args;
use crate::harness::HarnessCli;
use crate::model::{SyncItemOutcome, SyncOutcome, SyncSkipReason};
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

    if let Some(decided) = found_outcome(item, mcp_presence(files, server)) {
        return decided;
    }

    let cli = HarnessCli {
        harness,
        env: context.env,
        runner: context.runner,
        binary,
    };
    let args = add_json_args(&server.name, &mcp_definition(server, files.shape));

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
