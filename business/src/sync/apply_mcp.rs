//! The MCP servers of an apply ([`super::apply_plan`]): OpenCode's merged-names check and the
//! per-file batches. See the module docs of [`super::apply`].

use std::collections::BTreeSet;

use super::apply::{ApplyContext, outcome};
use super::plan_presence::{McpFiles, Presence, mcp_presence, with_merged};
use super::plan_rows::{Step, StoredItem, mcp_definition};
use crate::ecosystem::mcp_merge::merge_entries;
use crate::ecosystem::mcp_register::snippet;
use crate::ecosystem::{Blocked, MergeResult};
use crate::env::Env;
use crate::harnesses::opencode::merged_mcp_names;
use crate::model::{HarnessId, SyncItemOutcome, SyncNote, SyncOutcome, SyncSkipReason};
use crate::pi_family::tilde;

/// True when one of the MCP `servers` is missing from every file the app reads, so it would be
/// written.
pub(super) fn any_missing(servers: &[&StoredItem]) -> bool {
    servers.iter().any(|item| match (&item.mcp_files, &item.step) {
        (Some(files), Some(Step::Mcp(server))) => matches!(mcp_presence(files, server), Ok(Presence::Absent)),
        _ => false,
    })
}

/// Why OpenCode's merged names could not be checked: none of its servers is written then.
pub(super) enum Unchecked {
    /// OpenCode has no detected binary now.
    NoBinary,
    /// `opencode debug config` failed.
    Failed,
}

impl Unchecked {
    fn outcome(&self, item: &StoredItem) -> SyncItemOutcome {
        match self {
            Self::NoBinary => outcome(
                item,
                SyncOutcome::Skipped {
                    reason: SyncSkipReason::HarnessNotInstalled,
                },
            ),
            Self::Failed => SyncItemOutcome {
                summary: Some(UNCHECKED_SUMMARY.to_owned()),
                ..outcome(item, SyncOutcome::Failed)
            },
        }
    }
}

/// The summary of an OpenCode server not written because the check failed.
const UNCHECKED_SUMMARY: &str =
    "Could not check the MCP servers OpenCode merges from all its config layers. Nothing was written.";

/// The names OpenCode merges now (logged without any of the output when the check fails).
pub(super) async fn opencode_merged(context: ApplyContext<'_>) -> std::result::Result<BTreeSet<String>, Unchecked> {
    let Some(binary) = context.binaries.get(&HarnessId::OpenCode) else {
        return Err(Unchecked::NoBinary);
    };

    merged_mcp_names(context.env, context.runner, binary)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "share setup could not check OpenCode's merged config");
            Unchecked::Failed
        })
}

/// What the MCP servers of a batch are checked against besides the files the app reads.
#[derive(Clone, Copy)]
pub(super) enum Names<'a> {
    /// Nothing else (not OpenCode).
    NotNeeded,
    /// The names OpenCode merges, as `opencode debug config` reported them in this apply.
    Checked(&'a BTreeSet<String>),
    /// OpenCode's merged names were not checked in this apply (no server was missing): a server
    /// found missing now is a conflict, never written unchecked.
    Unchecked,
    /// The check ran into `Unchecked` in this apply: a server found missing now ends as it says,
    /// never written unchecked.
    Unavailable(&'a Unchecked),
}

/// The MCP servers of one file: each re-checked (an OpenCode one against OpenCode's merged `names`
/// too), then all that are still missing merged at once. Without a file (the review found the
/// harness not installed) every server is skipped.
pub(super) fn mcp_batch(
    env: &Env,
    files: Option<&McpFiles>,
    items: &[&StoredItem],
    names: Names<'_>,
) -> Vec<SyncItemOutcome> {
    let Some(files) = files else {
        return items
            .iter()
            .map(|item| {
                outcome(
                    item,
                    SyncOutcome::Skipped {
                        reason: SyncSkipReason::HarnessNotInstalled,
                    },
                )
            })
            .collect();
    };

    let mut outcomes: Vec<Option<SyncItemOutcome>> = Vec::new();
    let mut pending: Vec<(usize, (String, serde_json::Value))> = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let Some(Step::Mcp(server)) = &item.step else {
            outcomes.push(Some(outcome(item, SyncOutcome::Failed)));
            continue;
        };

        let presence = match names {
            Names::NotNeeded | Names::Unavailable(_) => mcp_presence(files, server),
            Names::Checked(names) => {
                mcp_presence(files, server).map(|presence| with_merged(presence, Some(names), &server.name))
            }
            Names::Unchecked => mcp_presence(files, server).map(|presence| with_merged(presence, None, &server.name)),
        };

        let decided = match presence {
            Err(refusal) => Some(outcome(
                item,
                SyncOutcome::Skipped {
                    reason: SyncSkipReason::McpFileRefused { refusal },
                },
            )),
            Ok(Presence::Absent) => match names {
                Names::Unavailable(unchecked) => Some(unchecked.outcome(item)),
                _ => None,
            },
            Ok(Presence::Conflict) => Some(outcome(item, SyncOutcome::Conflict)),
            Ok(Presence::Unreadable) => Some(SyncItemOutcome {
                notes: vec![SyncNote::ConfigUnreadable],
                ..outcome(item, SyncOutcome::Conflict)
            }),
            Ok(Presence::Present | Presence::DifferentVersion { .. }) => {
                Some(outcome(item, SyncOutcome::AlreadyPresent))
            }
        };

        if decided.is_none() {
            pending.push((index, (server.name.clone(), mcp_definition(server, files.shape))));
        }

        outcomes.push(decided);
    }

    if !pending.is_empty() {
        let entries: Vec<(String, serde_json::Value)> = pending.iter().map(|(_, entry)| entry.clone()).collect();
        let merged = merge_entries(&files.target, files.shape, &entries);

        for (position, (index, (name, definition))) in pending.iter().enumerate() {
            let result = match &merged {
                Err(Blocked::Refused(refusal)) => SyncOutcome::Skipped {
                    reason: SyncSkipReason::McpFileRefused { refusal: *refusal },
                },
                Err(Blocked::ManualEdit) => SyncOutcome::Skipped {
                    reason: SyncSkipReason::ManualEdit {
                        file: tilde(env, &files.target),
                        snippet: snippet(name, definition),
                    },
                },
                Ok(results) => match results.get(position).map(|server| server.result) {
                    Some(MergeResult::Added) => SyncOutcome::Done,
                    Some(MergeResult::AlreadyPresent) => SyncOutcome::AlreadyPresent,
                    Some(MergeResult::Conflict) => SyncOutcome::Conflict,
                    None => SyncOutcome::Failed,
                },
            };

            outcomes[*index] = Some(outcome(items[*index], result));
        }
    }

    items
        .iter()
        .zip(outcomes)
        .map(|(item, decided)| decided.unwrap_or_else(|| outcome(item, SyncOutcome::Failed)))
        .collect()
}
