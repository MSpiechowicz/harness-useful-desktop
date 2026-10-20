//! Applying a reviewed import ([`apply_plan`]).
//!
//! Refused as a whole (`invalidInput`, nothing runs, nothing is written) unless the user confirmed,
//! the selection is not empty, every selected id is a selectable item of this plan, and every
//! selected plugin that depends on a marketplace of the bundle has that marketplace selected too or
//! already registered here.
//!
//! Harnesses run one after another, in plan order; per harness: marketplaces, plugins, packages,
//! then MCP servers. Each item is checked against the machine again right before its step, so
//! applying twice runs nothing the second time: an item found present ends `alreadyPresent`, a
//! same-name item with another definition ends `conflict`, and neither runs anything. CLI steps go
//! through [`run_cli_action`] (fixed argv, no shell, ANSI stripped, output redacted and capped), in
//! the user scope and without a project. The MCP servers of one file are written by one
//! `merge_entries` call; a refused file skips all of them, and an OpenCode config file with
//! comments skips them as `manualEdit`, each with the entry to paste. OpenCode servers are first
//! checked against the names OpenCode merges from all its layers (`merged_mcp_names`, run once
//! per apply after OpenCode's CLI items and before any of its servers is written, and only when
//! one of them is missing from every file the app reads): a merged name ends `conflict`, and when
//! the check cannot run (no binary: `skipped: harnessNotInstalled`) or fails (`failed`), none is
//! written: each server still missing ends that way, the others end as they would after a check
//! (`alreadyPresent`, `conflict`, `skipped`). Without the check (every server already present,
//! conflicting or refused) no OpenCode server is written either: one found missing by then ends
//! `conflict`. A server left alone because a file that could define it cannot be read or parsed
//! ends `conflict` with the note `configUnreadable`. A failed marketplace skips the plugins that
//! depend on it; every other item still runs.
//!
//! There is no rollback: what succeeded stays when a later item fails. Logs carry counts only.

use std::collections::{BTreeMap, HashSet};

use super::StoredPlan;
use super::apply_mcp::{Names, any_missing, mcp_batch, opencode_merged};
use super::plan_presence::{Machine, McpFiles, Presence};
use super::plan_rows::{Step, StoredItem};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{CliRun, Harness, HarnessRegistry, run_cli_action};
use crate::model::{
    HarnessId, SyncImportResult, SyncItemKind, SyncItemOutcome, SyncOutcome, SyncProgress, SyncSkipReason,
};
use crate::process::{ProcessRunner, ResolvedBinary};
use crate::security::redact_text;

/// What [`apply_plan`] runs with.
#[derive(Clone, Copy)]
pub struct ApplyContext<'a> {
    /// The environment (CLIs run in the home directory).
    pub env: &'a Env,
    /// The harnesses.
    pub registry: &'a HarnessRegistry,
    /// Runs the harness CLIs.
    pub runner: &'a dyn ProcessRunner,
    /// The detected binary of each installed harness (the Tauri layer resolves one per
    /// [`StoredPlan::harnesses`]). CLI items of a harness missing here end `skipped:
    /// harnessNotInstalled`; its MCP servers are still merged (no binary needed), except
    /// OpenCode's, which need it to check what OpenCode merges.
    pub binaries: &'a BTreeMap<HarnessId, ResolvedBinary>,
}

/// Applies the `selected` items of `stored` (see the module docs), reporting each item to `sink` as
/// it starts and finishes. `invalidInput` when the request is refused; otherwise the outcome of
/// every selected item, in the order they were applied.
pub async fn apply_plan(
    context: ApplyContext<'_>,
    stored: &StoredPlan,
    selected: &[String],
    confirmed: bool,
    mut sink: impl FnMut(SyncProgress),
) -> Result<SyncImportResult> {
    let items = chosen(context, stored, selected, confirmed)?;
    let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
    let mut run = Run {
        context,
        total,
        next: 0,
        failed_marketplaces: HashSet::new(),
        outcomes: Vec::new(),
    };

    for harness in harness_order(&items) {
        let of_harness: Vec<&StoredItem> = items.iter().copied().filter(|item| item.harness == harness).collect();
        let (servers, cli): (Vec<&StoredItem>, Vec<&StoredItem>) = of_harness
            .into_iter()
            .partition(|item| item.kind == SyncItemKind::McpServer);

        for item in cli {
            run.started(item, &mut sink);
            let outcome = run.cli_item(item).await;
            run.finished(item, outcome, &mut sink);
        }

        // The merge needs no binary: an MCP item has its file only when the review found the
        // harness installed. OpenCode's merged names are checked once, after its CLI items ran, and
        // only when one of its servers would be written.
        let merged = match harness {
            HarnessId::OpenCode if any_missing(&servers) => Some(opencode_merged(context).await),
            _ => None,
        };

        for (files, servers) in by_file(&servers) {
            for item in &servers {
                run.started(item, &mut sink);
            }

            let names = match (&merged, harness) {
                (Some(Ok(names)), _) => Names::Checked(names),
                (Some(Err(unchecked)), _) => Names::Unavailable(unchecked),
                (None, HarnessId::OpenCode) => Names::Unchecked,
                (None, _) => Names::NotNeeded,
            };

            let outcomes = mcp_batch(context.env, files.as_ref(), &servers, names);
            for (item, outcome) in servers.into_iter().zip(outcomes) {
                run.finished(item, outcome, &mut sink);
            }
        }
    }

    let result = SyncImportResult {
        ok: run.outcomes.iter().all(|item| succeeded(&item.outcome)),
        items: run.outcomes,
    };

    let count = |matches: fn(&SyncOutcome) -> bool| result.items.iter().filter(|item| matches(&item.outcome)).count();
    tracing::info!(
        selected = result.items.len(),
        done = count(|outcome| matches!(outcome, SyncOutcome::Done)),
        already_present = count(|outcome| matches!(outcome, SyncOutcome::AlreadyPresent)),
        conflicts = count(|outcome| matches!(outcome, SyncOutcome::Conflict)),
        skipped = count(|outcome| matches!(outcome, SyncOutcome::Skipped { .. })),
        failed = count(|outcome| matches!(outcome, SyncOutcome::Failed)),
        "share setup import applied"
    );

    Ok(result)
}

/// The selected items in plan order, or why the request is refused (never naming an id).
fn chosen<'p>(
    context: ApplyContext<'_>,
    stored: &'p StoredPlan,
    selected: &[String],
    confirmed: bool,
) -> Result<Vec<&'p StoredItem>> {
    let refuse = |message: &str| Err(BusinessError::InvalidInput(message.to_owned()));

    if !confirmed {
        return refuse("the user has not confirmed the import");
    }

    if selected.is_empty() {
        return refuse("no item is selected");
    }

    let wanted: HashSet<&str> = selected.iter().map(String::as_str).collect();
    for id in &wanted {
        match stored.items().iter().find(|item| item.id == *id) {
            None => return refuse("a selected item is not part of this import; review it again"),
            Some(item) if item.step.is_none() => return refuse("a selected item cannot be imported"),
            Some(_) => {}
        }
    }

    let items: Vec<&StoredItem> = stored
        .items()
        .iter()
        .filter(|item| wanted.contains(item.id.as_str()))
        .collect();

    for item in &items {
        let Some(dependency) = &item.depends_on else {
            continue;
        };

        if !wanted.contains(dependency.id.as_str()) && !registered(context, item.harness, &dependency.marketplace) {
            return refuse("a selected plugin needs its marketplace; select the marketplace too");
        }
    }

    Ok(items)
}

/// True when `harness` has the marketplace `name` registered right now.
fn registered(context: ApplyContext<'_>, harness: HarnessId, name: &str) -> bool {
    context
        .registry
        .get(harness)
        .is_some_and(|harness| Machine::read(context.env, harness).has_marketplace(name))
}

/// The harnesses of `items`, in order of first appearance.
fn harness_order(items: &[&StoredItem]) -> Vec<HarnessId> {
    let mut order = Vec::new();

    for item in items {
        if !order.contains(&item.harness) {
            order.push(item.harness);
        }
    }

    order
}

/// MCP items grouped by target file, in order of first appearance.
fn by_file<'p>(servers: &[&'p StoredItem]) -> Vec<(Option<McpFiles>, Vec<&'p StoredItem>)> {
    let mut groups: Vec<(Option<McpFiles>, Vec<&StoredItem>)> = Vec::new();

    for item in servers {
        match groups.iter_mut().find(|(files, _)| *files == item.mcp_files) {
            Some((_, group)) => group.push(item),
            None => groups.push((item.mcp_files.clone(), vec![item])),
        }
    }

    groups
}

fn succeeded(outcome: &SyncOutcome) -> bool {
    matches!(outcome, SyncOutcome::Done | SyncOutcome::AlreadyPresent)
}

/// The state of one apply.
struct Run<'a> {
    context: ApplyContext<'a>,
    total: u32,
    next: u32,
    /// Marketplace items that did not end `done` or `alreadyPresent`.
    failed_marketplaces: HashSet<String>,
    outcomes: Vec<SyncItemOutcome>,
}

impl Run<'_> {
    fn started(&mut self, item: &StoredItem, sink: &mut impl FnMut(SyncProgress)) {
        sink(SyncProgress::Started {
            id: item.id.clone(),
            index: self.next,
            total: self.total,
        });
        self.next += 1;
    }

    fn finished(&mut self, item: &StoredItem, outcome: SyncItemOutcome, sink: &mut impl FnMut(SyncProgress)) {
        if item.kind == SyncItemKind::Marketplace && !succeeded(&outcome.outcome) {
            self.failed_marketplaces.insert(item.id.clone());
        }

        sink(SyncProgress::Finished {
            outcome: outcome.clone(),
        });
        self.outcomes.push(outcome);
    }

    /// A marketplace, plugin or package: re-checked, then run through the harness CLI.
    async fn cli_item(&self, item: &StoredItem) -> SyncItemOutcome {
        let harness = self.context.registry.get(item.harness);
        let binary = self.context.binaries.get(&item.harness);
        let (Some(harness), Some(binary), Some(step)) = (harness, binary, &item.step) else {
            return outcome(
                item,
                SyncOutcome::Skipped {
                    reason: SyncSkipReason::HarnessNotInstalled,
                },
            );
        };

        let dependency_failed = item
            .depends_on
            .as_ref()
            .is_some_and(|dependency| self.failed_marketplaces.contains(&dependency.id));
        if dependency_failed {
            return outcome(
                item,
                SyncOutcome::Skipped {
                    reason: SyncSkipReason::DependencyFailed,
                },
            );
        }

        match recheck(self.context.env, harness, step) {
            Presence::Absent => {}
            Presence::Conflict | Presence::Unreadable => return outcome(item, SyncOutcome::Conflict),
            Presence::Present | Presence::DifferentVersion { .. } => {
                return outcome(item, SyncOutcome::AlreadyPresent);
            }
        }

        let Some(action) = step.action(item.harness) else {
            return outcome(item, SyncOutcome::Failed);
        };

        let ran = run_cli_action(CliRun {
            harness,
            env: self.context.env,
            runner: self.context.runner,
            binary,
            action: &action,
            dry_run: false,
            project: None,
        })
        .await;

        match ran {
            Ok(result) => SyncItemOutcome {
                summary: result.summary,
                argv: result.argv,
                ..outcome(
                    item,
                    if result.ok {
                        SyncOutcome::Done
                    } else {
                        SyncOutcome::Failed
                    },
                )
            },
            Err(error) => SyncItemOutcome {
                summary: Some(redact_text(&error.to_string())),
                argv: item.command.clone(),
                ..outcome(item, SyncOutcome::Failed)
            },
        }
    }
}

/// What is installed now for a CLI step. A shared marketplace that now conflicts ends `conflict`,
/// which skips its dependent plugins.
fn recheck(env: &Env, harness: &dyn Harness, step: &Step) -> Presence {
    let machine = Machine::read(env, harness);

    match step {
        Step::Marketplace(marketplace) => machine.marketplace(marketplace),
        Step::Plugin(plugin) => machine.plugin(plugin),
        Step::Package(package) => machine.package(package),
        Step::Mcp(_) => Presence::Absent,
    }
}

/// `outcome` for `item`, with no command run.
pub(super) fn outcome(item: &StoredItem, outcome: SyncOutcome) -> SyncItemOutcome {
    SyncItemOutcome {
        id: item.id.clone(),
        harness: item.harness,
        kind: item.kind,
        label: item.label.clone(),
        outcome,
        summary: None,
        argv: Vec::new(),
        notes: Vec::new(),
    }
}
