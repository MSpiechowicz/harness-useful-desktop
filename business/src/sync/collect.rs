//! Export: what this machine can share, rebuilt from the harnesses' own files for every preview and
//! every export (the webview only ever sends item ids back).
//!
//! ```text
//! collect_export(env, registry, summaries) -> ExportCollection
//!     export_preview(&collection)                         -> SyncExportPreview   (sync_export_preview)
//!     build_bundle(&collection, selected, app, createdAt) -> Bundle              (sync_export / _save)
//!         export_code(&bundle)                            -> SyncExported        (the `hud1:` string)
//!         codec::to_json_pretty(&bundle)                  -> String              (the `.json` file)
//! ```
//!
//! Sources are the raw user files, never the redacted extensions view: OMP's marketplace registry,
//! user plugin registry and `<plugins>/package.json`; PI's user `settings.json` `packages`; the user
//! MCP file of [`mcp_target_paths`]; OpenCode's global config (`plugin` and `mcp`, see
//! [`super::collect_opencode`]); Claude Code's marketplace and plugin registries and the user
//! scope of its global file (see [`super::collect_claude`]); Codex's `config.toml` (see
//! [`super::collect_codex`]). Only installed harnesses (per
//! `summaries`) are read, and only the kinds their [`Harness::share_support`] allows. Files are
//! read through `SafeReader`; auth files are never opened, and no value is logged.
//!
//! Every item that is shared passes the same whole-item validator the import runs
//! ([`super::validate`]), so an export always imports again with zero invalid items. Anything left
//! out is listed in `excluded` with its name and a typed reason, never a value: secrets in a
//! command, argument, URL or source; URL credentials; local paths; project-scope plugins; unknown
//! package kinds and transports; values the import would refuse; more than
//! [`MAX_ITEMS_PER_KIND`] items of a kind or [`MAX_ITEMS`] in total. The one exception is the
//! dashboard's own MCP entry, skipped without a row (Recommended setup installs it).
//!
//! Items are ordered by kind (marketplaces, plugins, packages, MCP servers) and, within a kind, by
//! id, so the result does not depend on key order in the files. When two entries share an id (a PI
//! source listed twice with different pins), the first in file order is kept and the other dropped
//! silently.

use std::collections::BTreeSet;

use super::codec;
use super::collect_claude::{claude_marketplaces, claude_plugins, claude_servers};
use super::collect_codex::{codex_marketplaces, codex_plugins, codex_servers};
use super::collect_items::{marketplaces, packages, plugins};
use super::collect_mcp::mcp_servers;
use super::collect_opencode::{opencode_plugins, opencode_servers};
use super::schema::{Bundle, MAX_ITEMS, MAX_ITEMS_PER_KIND, Marketplace, McpServer, Package, Plugin};
use super::secrets::exportable;
use super::validate;
use super::validate_source::split_npm_spec;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{Harness, HarnessRegistry};
use crate::model::{
    HarnessId, HarnessSummary, SyncExcluded, SyncExcludedReason, SyncExportItem, SyncExportPreview, SyncExportSection,
    SyncExported, SyncInvalidReason, SyncItemKind,
};

#[cfg(doc)]
use crate::ecosystem::mcp_target_paths;

/// Shown instead of a name that is not safe to display.
pub(super) const UNKNOWN_LABEL: &str = "unknown";

/// Everything shareable on this machine, per installed harness, plus what is left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportCollection {
    sections: Vec<CollectedSection>,
    excluded: Vec<SyncExcluded>,
}

impl ExportCollection {
    /// How many items can be shared.
    pub fn item_count(&self) -> usize {
        self.sections.iter().map(|section| section.items.len()).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CollectedSection {
    harness: HarnessId,
    items: Vec<CollectedItem>,
}

/// One shareable item: its preview row and the bundle entry it becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CollectedItem {
    pub preview: SyncExportItem,
    pub entry: Entry,
}

/// A bundle entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Entry {
    Marketplace(Marketplace),
    Plugin(Plugin),
    Package(Package),
    McpServer(McpServer),
}

/// What one kind of one harness yielded.
#[derive(Debug, Default)]
pub(super) struct Found {
    pub items: Vec<CollectedItem>,
    pub excluded: Vec<SyncExcluded>,
}

impl Found {
    pub fn exclude(&mut self, harness: HarnessId, kind: SyncItemKind, label: String, reason: SyncExcludedReason) {
        self.excluded.push(SyncExcluded {
            harness,
            kind,
            label,
            reason,
        });
    }
}

/// Reads the shareable items of every installed harness of `registry` (a harness without a summary,
/// or not detected, is skipped). Reads files; call it from a blocking-capable context.
pub fn collect_export(env: &Env, registry: &HarnessRegistry, summaries: &[HarnessSummary]) -> ExportCollection {
    let mut collection = ExportCollection::default();
    let mut total = 0;

    for harness in registry.all() {
        let installed = summaries
            .iter()
            .any(|summary| summary.info.id == harness.id() && summary.detection.installed);
        if !installed {
            continue;
        }

        let mut items = Vec::new();
        for kind in [
            SyncItemKind::Marketplace,
            SyncItemKind::Plugin,
            SyncItemKind::Package,
            SyncItemKind::McpServer,
        ] {
            if !harness.share_support().supports(kind) {
                continue;
            }

            let found = collect_kind(env, harness, kind);
            collection.excluded.extend(found.excluded);
            items.extend(limit(
                harness.id(),
                kind,
                found.items,
                &mut total,
                &mut collection.excluded,
            ));
        }

        if !items.is_empty() {
            collection.sections.push(CollectedSection {
                harness: harness.id(),
                items,
            });
        }
    }

    collection
}

fn collect_kind(env: &Env, harness: &dyn Harness, kind: SyncItemKind) -> Found {
    let paths = harness.paths(env, None);
    let mut found = Found::default();

    match (harness.id(), kind) {
        (HarnessId::Omp, SyncItemKind::Marketplace) => marketplaces(harness.id(), &paths, &mut found),
        (HarnessId::Omp, SyncItemKind::Plugin) => plugins(harness.id(), &paths, &mut found),
        (HarnessId::Omp | HarnessId::Pi, SyncItemKind::McpServer) => mcp_servers(env, harness, &mut found),
        (HarnessId::Pi, SyncItemKind::Package) => packages(harness.id(), &paths, &mut found),
        (HarnessId::OpenCode, SyncItemKind::Plugin) => opencode_plugins(env, harness, &mut found),
        (HarnessId::OpenCode, SyncItemKind::McpServer) => opencode_servers(env, harness, &mut found),
        (HarnessId::Claude, SyncItemKind::Marketplace) => claude_marketplaces(env, &mut found),
        (HarnessId::Claude, SyncItemKind::Plugin) => claude_plugins(env, &mut found),
        (HarnessId::Claude, SyncItemKind::McpServer) => claude_servers(env, harness, &mut found),
        (HarnessId::Codex, SyncItemKind::Marketplace) => codex_marketplaces(env, &mut found),
        (HarnessId::Codex, SyncItemKind::Plugin) => codex_plugins(env, &mut found),
        (HarnessId::Codex, SyncItemKind::McpServer) => codex_servers(env, harness, &mut found),
        // Kinds these harnesses do not have (see `Harness::share_support`).
        (HarnessId::Omp | HarnessId::Claude | HarnessId::Codex, SyncItemKind::Package)
        | (HarnessId::Pi, SyncItemKind::Marketplace | SyncItemKind::Plugin)
        | (HarnessId::OpenCode, SyncItemKind::Marketplace | SyncItemKind::Package) => {}
    }

    found
}

/// Sorts by id, drops repeated ids, and moves items past the per-kind and total limits to
/// `excluded` as `tooMany`.
fn limit(
    harness: HarnessId,
    kind: SyncItemKind,
    mut items: Vec<CollectedItem>,
    total: &mut usize,
    excluded: &mut Vec<SyncExcluded>,
) -> Vec<CollectedItem> {
    items.sort_by(|a, b| a.preview.id.cmp(&b.preview.id));
    items.dedup_by(|later, kept| later.preview.id == kept.preview.id);

    let mut kept = Vec::new();
    for item in items {
        if kept.len() >= MAX_ITEMS_PER_KIND || *total >= MAX_ITEMS {
            excluded.push(SyncExcluded {
                harness,
                kind,
                label: item.preview.label,
                reason: SyncExcludedReason::TooMany,
            });
            continue;
        }

        *total += 1;
        kept.push(item);
    }

    kept
}

/// The export dialog's list: the shareable items per harness (in registry order) and the excluded
/// ones.
pub fn export_preview(collection: &ExportCollection) -> SyncExportPreview {
    SyncExportPreview {
        sections: collection
            .sections
            .iter()
            .map(|section| SyncExportSection {
                harness: section.harness,
                items: section.items.iter().map(|item| item.preview.clone()).collect(),
            })
            .collect(),
        excluded: collection.excluded.clone(),
    }
}

/// The bundle of the `selected` item ids. Ids that are not in `collection` are ignored (an item may
/// have changed since the preview; ids can only pick what Rust collected), and so are repeats.
/// `app` and `created_at` (`YYYY-MM-DD`) are left out when they would not pass import validation.
///
/// Fails with `invalidInput` when more ids are given than a bundle may hold, or when none of them
/// is in `collection`.
pub fn build_bundle(
    collection: &ExportCollection,
    selected: &[String],
    app: Option<String>,
    created_at: Option<String>,
) -> Result<Bundle> {
    if selected.len() > MAX_ITEMS {
        return Err(BusinessError::InvalidInput("too many items are selected".to_owned()));
    }

    let selected: BTreeSet<&str> = selected.iter().map(String::as_str).collect();
    let app = app.filter(|app| validate::version(Some(app)).is_ok());
    let created_at = created_at.filter(|date| validate::date(date).is_ok());
    let mut bundle = Bundle::new(app, created_at);

    for section in &collection.sections {
        let picked = section
            .items
            .iter()
            .filter(|item| selected.contains(item.preview.id.as_str()));

        for item in picked {
            let entries = bundle.harnesses.entry(section.harness).or_default();

            match &item.entry {
                Entry::Marketplace(marketplace) => entries.marketplaces.push(marketplace.clone()),
                Entry::Plugin(plugin) => entries.plugins.push(plugin.clone()),
                Entry::Package(package) => entries.packages.push(package.clone()),
                Entry::McpServer(server) => entries.mcp_servers.push(server.clone()),
            }
        }
    }

    if bundle.item_count() == 0 {
        return Err(BusinessError::InvalidInput(
            "none of the selected items can be shared any more; open the export again".to_owned(),
        ));
    }

    Ok(bundle)
}

/// The `hud1:` string of `bundle`. Fails with `invalidInput` when it is over 64 KiB (the message
/// says to save the setup as a file instead).
pub fn export_code(bundle: &Bundle) -> Result<SyncExported> {
    let code = codec::encode_code(bundle)?;

    Ok(SyncExported {
        item_count: u32::try_from(bundle.item_count()).unwrap_or(u32::MAX),
        bytes: u32::try_from(code.len()).unwrap_or(u32::MAX),
        code,
    })
}

// ---------------------------------------------------------------------------------------------
// Ids and labels
// ---------------------------------------------------------------------------------------------

/// The id of an item: `<harness>:<kind>:<key>`, where kind is `marketplace`, `plugin`, `package` or
/// `mcp` and key the marketplace name, plugin id, [`package_identity`] or server name.
pub fn item_id(harness: HarnessId, kind: SyncItemKind, key: &str) -> String {
    let harness = serde_json::to_value(harness)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let kind = match kind {
        SyncItemKind::Marketplace => "marketplace",
        SyncItemKind::Plugin => "plugin",
        SyncItemKind::Package => "package",
        SyncItemKind::McpServer => "mcp",
    };

    format!("{harness}:{kind}:{key}")
}

/// What makes two PI package sources the same package, whatever their pin: `npm:<name>` without
/// `@version`; for git and URL sources, the scheme, host and repository path without any `#ref`,
/// `@ref` (which may hold `/`, as in `@feature/x`) or `.git`.
pub fn package_identity(source: &str) -> String {
    if let Some(spec) = source.strip_prefix("npm:") {
        let (name, _) = split_npm_spec(spec);

        return format!("npm:{name}");
    }

    let source = source.split_once('#').map_or(source, |(base, _)| base);
    let scheme_end = match source.find("://") {
        Some(at) => at + "://".len(),
        None => source.strip_prefix("git:").map_or(0, |_| "git:".len()),
    };

    // The host (with any `user@` and, for `git@host:owner`, the owner) ends at the first `/`.
    let Some(path_start) = source[scheme_end..].find('/').map(|at| scheme_end + at + 1) else {
        return source.to_owned();
    };

    let (head, path) = source.split_at(path_start);
    let path = path.split_once('@').map_or(path, |(repository, _)| repository);
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);

    format!("{head}{path}")
}

/// A preview row of `kind` labelled `label`, with nothing else set.
pub(super) fn preview_row(kind: SyncItemKind, label: String) -> SyncExportItem {
    SyncExportItem {
        id: String::new(),
        kind,
        label,
        detail: None,
        version: None,
        runs_program: false,
        env_keys: Vec::new(),
        header_keys: Vec::new(),
        notes: Vec::new(),
    }
}

/// `value` when it is a non-empty, display-safe, non-secret text, else [`UNKNOWN_LABEL`].
pub(super) fn safe_label(value: Option<&str>) -> String {
    match value {
        Some(text) if is_safe_text(text) => text.to_owned(),
        _ => UNKNOWN_LABEL.to_owned(),
    }
}

/// True for a non-empty text that passes [`validate::text`] and [`exportable`].
pub(super) fn is_safe_text(text: &str) -> bool {
    !text.is_empty() && validate::text(text, super::schema::MAX_TEXT_CHARS).is_ok() && exportable(text)
}

/// The export reason for a value the import validators refuse.
pub(super) fn excluded_reason(reason: SyncInvalidReason) -> SyncExcludedReason {
    match reason {
        SyncInvalidReason::ContainsSecret => SyncExcludedReason::ContainsSecret,
        SyncInvalidReason::Credentials => SyncExcludedReason::Credentials,
        SyncInvalidReason::LocalPath => SyncExcludedReason::LocalPath,
        SyncInvalidReason::UnsupportedSource | SyncInvalidReason::UnsupportedScheme => SyncExcludedReason::Unsupported,
        _ => SyncExcludedReason::Invalid,
    }
}

/// Why a source or spec may not leave this machine, checked before the import validators so the
/// reason names the actual problem: credentials, then secrets, then a local path.
pub(super) fn source_problem(value: &str) -> Option<SyncExcludedReason> {
    if validate::has_credentials(value) {
        return Some(SyncExcludedReason::Credentials);
    }

    if !exportable(value) {
        return Some(SyncExcludedReason::ContainsSecret);
    }

    if validate::is_local_path(value) {
        return Some(SyncExcludedReason::LocalPath);
    }

    None
}
