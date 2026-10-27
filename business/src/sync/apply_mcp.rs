//! The MCP servers of an apply ([`super::apply_plan`]): OpenCode's merged-names check and the
//! per-file batches. See the module docs of [`super::apply`].

use std::collections::BTreeSet;

use super::apply::{ApplyContext, outcome};
use super::endpoint::same_endpoint;
use super::plan_presence::{McpFiles, Presence, Replace, mcp_presence, with_merged};
use super::plan_rows::{Step, StoredItem, mcp_definition};
use super::schema::McpServer;
use crate::ecosystem::mcp_merge::{EntryChange, managed_text, merge_entries};
use crate::ecosystem::mcp_register::snippet;
use crate::ecosystem::{Blocked, MergeResult};
use crate::env::Env;
use crate::harnesses::opencode::merged_mcp_names;
use crate::model::{HarnessId, McpRefusal, SyncItemOutcome, SyncNote, SyncOutcome, SyncSkipReason};
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
    /// The probe (`opencode debug config`, or `opencode api` on OpenCode 2) failed.
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

/// The names OpenCode merges now, asked of the detected generation (logged without any of the
/// output when the check fails).
pub(super) async fn opencode_merged(context: ApplyContext<'_>) -> std::result::Result<BTreeSet<String>, Unchecked> {
    let Some(binary) = context.binaries.get(&HarnessId::OpenCode) else {
        return Err(Unchecked::NoBinary);
    };

    let generation = context
        .registry
        .get(HarnessId::OpenCode)
        .and_then(|opencode| opencode.opencode_generation())
        .unwrap_or_default();

    merged_mcp_names(context.env, context.runner, binary, generation)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "share setup could not check OpenCode's merged config");
            Unchecked::Failed
        })
}

/// Whether an existing entry is the same endpoint as a server (see [`EntryChange::Replace`]).
type Keeps<'a> = Box<dyn Fn(&serde_json::Value) -> bool + 'a>;

/// What the MCP servers of a batch are checked against besides the files the app reads.
#[derive(Clone, Copy)]
pub(super) enum Names<'a> {
    /// Nothing else (not OpenCode).
    NotNeeded,
    /// The names OpenCode merges, as its probe reported them in this apply.
    Checked(&'a BTreeSet<String>),
    /// OpenCode's merged names were not checked in this apply (no server was missing): a server
    /// found missing now is a conflict, never written unchecked.
    Unchecked,
    /// The check ran into `Unchecked` in this apply: a server found missing now ends as it says,
    /// never written unchecked.
    Unavailable(&'a Unchecked),
}

/// The outcome of an MCP item that `presence` already decides: the server or another of its name
/// is there, or the file cannot be read safely. `None` when the server is missing, or (a replace
/// item) its entry in the file the app writes still differs, which the item then replaces. Any
/// other item finding such an entry ends `conflict`: it appeared since the review.
pub(super) fn found_outcome(item: &StoredItem, presence: Result<Presence, McpRefusal>) -> Option<SyncItemOutcome> {
    match presence {
        Err(refusal) => Some(outcome(
            item,
            SyncOutcome::Skipped {
                reason: SyncSkipReason::McpFileRefused { refusal },
            },
        )),
        Ok(Presence::Absent) => None,
        Ok(Presence::Replaceable(_)) if item.replace => None,
        Ok(Presence::Conflict | Presence::Replaceable(_)) => Some(outcome(item, SyncOutcome::Conflict)),
        Ok(Presence::Unreadable) => Some(SyncItemOutcome {
            notes: vec![SyncNote::ConfigUnreadable],
            ..outcome(item, SyncOutcome::Conflict)
        }),
        Ok(Presence::Present | Presence::DifferentVersion { .. }) => Some(outcome(item, SyncOutcome::AlreadyPresent)),
    }
}

/// The MCP servers of one file: each re-checked (an OpenCode one against OpenCode's merged `names`
/// too), then all that are still missing added and all replace items whose entry still differs
/// replaced, in one write. A replace item whose entry is gone is added; one whose entry now equals
/// the shared one ends `alreadyPresent`. Without a file (the review found the
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
    // (index, server, definition, what replacing the existing entry keeps; `None`: it is added)
    let mut pending: Vec<(usize, &McpServer, serde_json::Value, Option<Replace>)> = Vec::new();

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

        let replacing = match presence {
            Ok(Presence::Replaceable(replace)) if item.replace => Some(replace),
            _ => None,
        };
        let decided = match (presence, names) {
            (Ok(Presence::Absent), Names::Unavailable(unchecked)) => Some(unchecked.outcome(item)),
            (presence, _) => found_outcome(item, presence),
        };

        if decided.is_none() {
            pending.push((index, server, mcp_definition(server, files.shape), replacing));
        }

        outcomes.push(decided);
    }

    if !pending.is_empty() {
        // Whether an existing entry is the same endpoint is asked of the entry the merge writes.
        let shape = files.shape;
        let keepers: Vec<Keeps<'_>> = pending
            .iter()
            .map(|(_, server, ..)| {
                let server: &McpServer = server;
                Box::new(move |entry: &serde_json::Value| same_endpoint(entry, server, shape)) as Box<_>
            })
            .collect();
        let entries: Vec<(String, EntryChange<'_>)> = pending
            .iter()
            .zip(&keepers)
            .map(|((_, server, definition, replacing), keeps)| {
                let change = if replacing.is_some() {
                    EntryChange::Replace {
                        definition: definition.clone(),
                        keeps: &**keeps,
                    }
                } else {
                    EntryChange::Add(definition.clone())
                };

                (server.name.clone(), change)
            })
            .collect();
        let merged = merge_entries(&files.target, files.shape, &entries);

        for (position, (index, server, definition, replacing)) in pending.iter().enumerate() {
            let result = match &merged {
                Err(Blocked::Refused(refusal)) => SyncOutcome::Skipped {
                    reason: SyncSkipReason::McpFileRefused { refusal: *refusal },
                },
                // Only the managed fields are pasted over the entry of the same endpoint; another
                // endpoint gets the whole entry, as a new server would.
                Err(Blocked::ManualEdit) if *replacing == Some(Replace::Keep) => SyncOutcome::Skipped {
                    reason: SyncSkipReason::ManualReplace {
                        file: tilde(env, &files.target),
                        snippet: managed_text(definition, files.shape),
                    },
                },
                Err(Blocked::ManualEdit) => SyncOutcome::Skipped {
                    reason: SyncSkipReason::ManualEdit {
                        file: tilde(env, &files.target),
                        snippet: snippet(&server.name, definition),
                    },
                },
                Ok(results) => match results.get(position).map(|server| server.result) {
                    Some(MergeResult::Added) => SyncOutcome::Done,
                    Some(MergeResult::Replaced) => SyncOutcome::Replaced,
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
