//! Import review: what each item of a received bundle would do on this machine.
//!
//! ```text
//! codec::decode_input / decode_file -> DecodedBundle
//!     build_plan(env, registry, summaries, &decoded, &opencode_merged) -> BuiltPlan
//!         .plan   -> SyncImportPlan  (to the webview: statuses, exact steps, notes)
//!         .stored -> StoredPlan      (kept by Rust under the plan id, consumed by `apply_plan`)
//! ```
//!
//! Every step is built here from validated fields only: `MarketplaceAdd`, `PluginInstall` in the
//! user scope (an OpenCode plugin as `<name>[@<version>]`), `PackageInstall` (no project), or an MCP
//! entry holding only `type`, `command`, `args`, `url` and, when the server needs env or header
//! values, `"enabled": false` (OpenCode: `type`, the `command` array or `url`, and `enabled`). The
//! MCP file comes from [`mcp_target`], never from the bundle. Statuses compare the bundle with this machine's
//! files in process (see [`super::plan_presence`]); existing MCP definitions and local sources never
//! reach the plan.
//!
//! Status precedence: `invalid` (failed validation), `blocked` (the harness has no such kind or no
//! user MCP file, is not installed, or a plugin's marketplace is neither registered nor shared), then
//! what is here (`alreadyPresent`, `differentVersion`, `conflict`), else `new`. Only `new` items are
//! selectable; they start ticked, except stdio MCP servers (they run a program). A plugin whose
//! marketplace the bundle shares with the same name but another source is a `conflict` (installing
//! it would use the marketplace registered here). A repeated id keeps its first item. An MCP server
//! whose target file cannot be read safely is a `conflict` (nothing can be written there). An
//! OpenCode server is also a `conflict` when OpenCode merges a server of that name from a layer the
//! app cannot read, or when that could not be checked ([`OpenCodeMerged`]).

use std::collections::{BTreeSet, HashSet};

use super::codec::DecodedBundle;
use super::collect::item_id;
use super::plan_presence::{Machine, McpFiles, Presence, mcp_presence, with_merged};
use super::plan_rows::{Dependency, Row, SharedMarketplace, Step, StoredPlan, display_argv, mcp_preview};
use super::schema::{McpServer, Parsed, ParsedSection, Plugin};
use crate::ecosystem::mcp_target;
use crate::env::Env;
use crate::harness::{Harness, HarnessRegistry, ShareSupport};
use crate::harnesses::opencode::merged_mcp_names;
use crate::model::{
    HarnessId, HarnessSummary, SyncBlockReason, SyncImportPlan, SyncImportSection, SyncItemKind, SyncItemStatus,
};
use crate::pi_family::tilde;
use crate::process::{ProcessRunner, ResolvedBinary};

/// A reviewed import: what the webview shows, and what Rust keeps to apply it.
#[derive(Debug, Clone)]
pub struct BuiltPlan {
    /// The review sent to the webview.
    pub plan: SyncImportPlan,
    /// The steps behind it, for `apply_plan`; never sent to the webview.
    pub stored: StoredPlan,
}

/// The MCP server names OpenCode merges from all its config layers, including those the app cannot
/// read (a remote organization config, managed settings, `OPENCODE_CONFIG_CONTENT`, ...), as
/// `opencode debug config` reports them ([`merged_mcp_names`]). Checked before the review
/// ([`OpenCodeMerged::probe`]) and again right before the apply writes an OpenCode server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OpenCodeMerged {
    /// Not checked, or the check failed: no new OpenCode server is offered.
    #[default]
    Unknown,
    /// The names OpenCode merges.
    Names(BTreeSet<String>),
}

impl OpenCodeMerged {
    /// True when `decoded` shares a valid OpenCode MCP server, which needs the check.
    pub fn needed(decoded: &DecodedBundle) -> bool {
        decoded
            .sections
            .iter()
            .filter(|section| section.harness == HarnessId::OpenCode)
            .any(|section| section.mcp_servers.iter().any(|server| server.item.is_ok()))
    }

    /// Runs the check with OpenCode's detected `binary`; `Unknown` without one or when it fails
    /// (logged without any of the output).
    pub async fn probe(env: &Env, runner: &dyn ProcessRunner, binary: Option<&ResolvedBinary>) -> Self {
        let Some(binary) = binary else {
            return Self::Unknown;
        };

        match merged_mcp_names(env, runner, binary).await {
            Ok(names) => Self::Names(names),
            Err(error) => {
                tracing::warn!(%error, "share setup could not check OpenCode's merged config");
                Self::Unknown
            }
        }
    }

    fn names(&self) -> Option<&BTreeSet<String>> {
        match self {
            Self::Unknown => None,
            Self::Names(names) => Some(names),
        }
    }
}

/// Builds the review of `decoded` for this machine, under a fresh uuid plan id. A harness counts as
/// installed when its summary in `summaries` says so; `opencode` is what OpenCode merges (see
/// [`OpenCodeMerged`]). Reads the harnesses' registries and MCP files (never runs anything); call
/// it from a blocking-capable context.
pub fn build_plan(
    env: &Env,
    registry: &HarnessRegistry,
    summaries: &[HarnessSummary],
    decoded: &DecodedBundle,
    opencode: &OpenCodeMerged,
) -> BuiltPlan {
    let mut seen = HashSet::new();
    let mut sections = Vec::new();
    let mut stored = Vec::new();

    for parsed in &decoded.sections {
        let mut context = Context::new(env, registry.get(parsed.harness), parsed.harness, summaries);
        if parsed.harness == HarnessId::OpenCode {
            context.merged = opencode.names();
        }

        let rows: Vec<Row> = context
            .rows(parsed)
            .into_iter()
            .filter(|row| seen.insert(row.item.id.clone()))
            .collect();

        sections.push(SyncImportSection {
            harness: parsed.harness,
            installed: context.installed,
            items: rows.iter().map(|row| row.item.clone()).collect(),
        });
        stored.extend(rows.into_iter().map(|row| row.stored));
    }

    let id = uuid::Uuid::new_v4().to_string();
    tracing::info!(
        sections = sections.len(),
        items = stored.len(),
        selectable = stored.iter().filter(|item| item.step.is_some()).count(),
        "share setup import reviewed"
    );

    BuiltPlan {
        plan: SyncImportPlan {
            plan_id: id.clone(),
            app: decoded.app.clone(),
            created_at: decoded.created_at.clone(),
            sections,
            unsupported: decoded.unsupported.clone(),
            invalid_count: u32::try_from(decoded.invalid_count()).unwrap_or(u32::MAX),
        },
        stored: StoredPlan::new(id, stored),
    }
}

/// The user MCP file of `harness`, when it has one.
fn mcp_files(env: &Env, harness: &dyn Harness) -> Option<McpFiles> {
    mcp_target(env, harness).map(|file| McpFiles {
        target: file.path,
        also_read: file.also_read,
        shape: file.shape,
    })
}

/// The marketplace a plugin comes from: its `marketplace`, else the `@marketplace` of its id. An
/// OpenCode plugin is an npm package and has none.
fn plugin_marketplace(harness: HarnessId, plugin: &Plugin) -> Option<String> {
    if harness == HarnessId::OpenCode {
        return None;
    }

    if let Some(marketplace) = &plugin.marketplace {
        return Some(marketplace.clone());
    }

    if plugin.id.starts_with('@') {
        return None;
    }

    plugin
        .id
        .rsplit_once('@')
        .map(|(_, marketplace)| marketplace.to_owned())
}

/// One harness section: what the harness supports and what is installed here.
struct Context<'a> {
    env: &'a Env,
    id: HarnessId,
    harness: Option<&'a dyn Harness>,
    installed: bool,
    support: ShareSupport,
    machine: Machine,
    mcp_files: Option<McpFiles>,
    /// For OpenCode, the names it merges (`None`: unknown).
    merged: Option<&'a BTreeSet<String>>,
}

impl<'a> Context<'a> {
    fn new(env: &'a Env, harness: Option<&'a dyn Harness>, id: HarnessId, summaries: &[HarnessSummary]) -> Self {
        let detected = summaries
            .iter()
            .any(|summary| summary.info.id == id && summary.detection.installed);
        let installed = harness.is_some() && detected;
        let support = harness.map_or(ShareSupport::NONE, |harness| harness.share_support());

        // Nothing is read for a harness that is not installed: all its items are blocked.
        let (machine, mcp_files) = match harness {
            Some(harness) if installed => (Machine::read(env, harness), mcp_files(env, harness)),
            _ => (Machine::default(), None),
        };

        Self {
            env,
            id,
            harness,
            installed,
            support,
            machine,
            mcp_files,
            merged: None,
        }
    }

    /// Marketplaces, plugins, packages, then MCP servers, each in bundle order.
    fn rows(&self, parsed: &ParsedSection) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut marketplaces = Vec::new();

        for item in &parsed.marketplaces {
            let row = self.row(
                SyncItemKind::Marketplace,
                item,
                |marketplace| Step::Marketplace(marketplace.clone()),
                &[],
            );

            if let Ok(marketplace) = &item.item {
                marketplaces.push(SharedMarketplace {
                    name: marketplace.name.clone(),
                    id: row.item.id.clone(),
                    status: row.item.status.clone(),
                });
            }

            rows.push(row);
        }

        for item in &parsed.plugins {
            rows.push(self.row(
                SyncItemKind::Plugin,
                item,
                |plugin| Step::Plugin(plugin.clone()),
                &marketplaces,
            ));
        }

        for item in &parsed.packages {
            rows.push(self.row(
                SyncItemKind::Package,
                item,
                |package| Step::Package(package.clone()),
                &[],
            ));
        }

        for item in &parsed.mcp_servers {
            rows.push(self.row(SyncItemKind::McpServer, item, |server| Step::Mcp(server.clone()), &[]));
        }

        rows
    }

    /// The row of one parsed item.
    fn row<T>(
        &self,
        kind: SyncItemKind,
        parsed: &Parsed<T>,
        to_step: impl FnOnce(&T) -> Step,
        marketplaces: &[SharedMarketplace],
    ) -> Row {
        let step = match &parsed.item {
            Ok(item) => to_step(item),
            Err(reason) => {
                let position = format!("#{}", parsed.index + 1);
                let label = parsed.label.clone().unwrap_or_else(|| position.clone());

                return Row::invalid(item_id(self.id, kind, &position), self.id, kind, label, *reason);
            }
        };

        let (status, dependency) = match self.blocked(kind) {
            Some(reason) => (SyncItemStatus::Blocked { reason }, None),
            None => self.compare(&step, marketplaces),
        };

        let mut row = Row::valid(item_id(self.id, kind, &step.key()), self.id, &step, status);
        if row.item.status == SyncItemStatus::New {
            self.select(&mut row, step, dependency);
        }

        row
    }

    /// Why no item of `kind` can be imported here, if so.
    fn blocked(&self, kind: SyncItemKind) -> Option<SyncBlockReason> {
        let no_mcp_file = kind == SyncItemKind::McpServer && self.installed && self.mcp_files.is_none();

        if !self.support.supports(kind) || no_mcp_file {
            return Some(SyncBlockReason::NotSupported);
        }

        if !self.installed {
            return Some(SyncBlockReason::HarnessNotInstalled);
        }

        None
    }

    /// The status of an item against this machine, and the bundle marketplace a new plugin needs.
    fn compare(&self, step: &Step, marketplaces: &[SharedMarketplace]) -> (SyncItemStatus, Option<Dependency>) {
        let presence = match step {
            Step::Marketplace(marketplace) => self.machine.marketplace(marketplace),
            Step::Plugin(plugin) => self.machine.plugin(plugin),
            Step::Package(package) => self.machine.package(package),
            Step::Mcp(server) => self.mcp(server),
        };

        match (step, presence) {
            (Step::Plugin(plugin), Presence::Absent) => self.plugin_status(plugin, marketplaces),
            (_, presence) => (status_of(presence), None),
        }
    }

    /// What this machine has of an MCP server. A target file that cannot be read safely is a
    /// conflict (nothing could be written there); so is an OpenCode server OpenCode merges from
    /// elsewhere, or that could not be checked.
    fn mcp(&self, server: &McpServer) -> Presence {
        let Some(files) = &self.mcp_files else {
            return Presence::Absent;
        };

        let presence = mcp_presence(files, server).unwrap_or(Presence::Conflict);

        if self.id != HarnessId::OpenCode {
            return presence;
        }

        with_merged(presence, self.merged, &server.name)
    }

    /// A plugin that is not installed: `new` when its marketplace is registered here or shared as
    /// `new` (then it depends on that item), `conflict` when the bundle's marketplace of that name
    /// conflicts, else blocked.
    fn plugin_status(
        &self,
        plugin: &Plugin,
        marketplaces: &[SharedMarketplace],
    ) -> (SyncItemStatus, Option<Dependency>) {
        let Some(name) = plugin_marketplace(self.id, plugin) else {
            return (SyncItemStatus::New, None);
        };

        let shared = marketplaces.iter().find(|marketplace| marketplace.name == name);

        match shared {
            Some(shared) if shared.status == SyncItemStatus::New => {
                let dependency = Dependency {
                    id: shared.id.clone(),
                    marketplace: name,
                };
                (SyncItemStatus::New, Some(dependency))
            }
            Some(shared) if shared.status == SyncItemStatus::Conflict => (SyncItemStatus::Conflict, None),
            _ if self.machine.has_marketplace(&name) => (SyncItemStatus::New, None),
            _ => (
                SyncItemStatus::Blocked {
                    reason: SyncBlockReason::MarketplaceMissing,
                },
                None,
            ),
        }
    }

    /// Makes a `new` row selectable with its displayed step.
    fn select(&self, row: &mut Row, step: Step, dependency: Option<Dependency>) {
        let display = match &step {
            Step::Mcp(server) => self
                .mcp_files
                .as_ref()
                .map(|files| (mcp_preview(tilde(self.env, &files.target), server), Some(files.clone()))),
            _ => self
                .harness
                .zip(step.action(self.id))
                .and_then(|(harness, action)| display_argv(harness, &action))
                .map(|display| (display, None)),
        };

        match display {
            Some((display, files)) => row.select(step, display, dependency, files),
            // Unreachable for validated items: a step the adapter refuses is never offered.
            None => {
                row.item.status = SyncItemStatus::Blocked {
                    reason: SyncBlockReason::NotSupported,
                }
            }
        }
    }
}

fn status_of(presence: Presence) -> SyncItemStatus {
    match presence {
        Presence::Absent => SyncItemStatus::New,
        Presence::Present => SyncItemStatus::AlreadyPresent,
        Presence::DifferentVersion { installed, shared } => SyncItemStatus::DifferentVersion { installed, shared },
        Presence::Conflict => SyncItemStatus::Conflict,
    }
}
