//! Export readers for marketplaces and plugins (OMP) and packages (PI). See [`super::collect`].

use super::collect::{
    CollectedItem, Entry, Found, excluded_reason, is_safe_text, item_id, package_identity, preview_row, safe_label,
    source_problem,
};
use super::schema::{Marketplace, Package, Plugin};
use super::secrets::exportable;
use super::validate;
use crate::harness::HarnessPaths;
use crate::harnesses::omp::{MarketplaceRecord, PluginOrigin, PluginRecord, marketplace_records, user_plugin_records};
use crate::harnesses::pi::{PackageRecord, user_package_records};
use crate::model::{HarnessId, SyncExcludedReason, SyncExportItem, SyncItemKind, SyncNote};

/// The marketplace registry: local marketplaces and sources with secrets or credentials are left out.
pub(super) fn marketplaces(harness: HarnessId, paths: &HarnessPaths, found: &mut Found) {
    let kind = SyncItemKind::Marketplace;

    for record in marketplace_records(paths) {
        let label = safe_label(record.name.as_deref());

        match marketplace(record) {
            Ok(marketplace) => found.items.push(marketplace_item(harness, marketplace)),
            Err(reason) => found.exclude(harness, kind, label, reason),
        }
    }
}

/// The export row of a shareable marketplace.
pub(super) fn marketplace_item(harness: HarnessId, marketplace: Marketplace) -> CollectedItem {
    let kind = SyncItemKind::Marketplace;

    CollectedItem {
        preview: SyncExportItem {
            id: item_id(harness, kind, &marketplace.name),
            detail: Some(marketplace.source.clone()),
            ..preview_row(kind, marketplace.name.clone())
        },
        entry: Entry::Marketplace(marketplace),
    }
}

fn marketplace(record: MarketplaceRecord) -> Result<Marketplace, SyncExcludedReason> {
    let name = record.name.ok_or(SyncExcludedReason::Invalid)?;
    validate::marketplace_name(&name).map_err(excluded_reason)?;

    if record.source_type.as_deref() == Some("local") {
        return Err(SyncExcludedReason::LocalPath);
    }

    let source = record.source_uri.ok_or(SyncExcludedReason::Invalid)?;
    if let Some(problem) = source_problem(&source) {
        return Err(problem);
    }

    let marketplace = Marketplace { name, source };
    validate::marketplace(&marketplace).map_err(excluded_reason)?;

    Ok(marketplace)
}

/// The user plugins: project-scope installs and npm plugins that are not installed from the
/// registry (`file:`, `link:`, git, URLs) are left out. The version is informational.
pub(super) fn plugins(harness: HarnessId, paths: &HarnessPaths, found: &mut Found) {
    let kind = SyncItemKind::Plugin;

    for record in user_plugin_records(paths) {
        let label = safe_label(Some(&record.id));

        match plugin(record) {
            Ok(plugin) => found.items.push(plugin_item(harness, plugin)),
            Err(reason) => found.exclude(harness, kind, label, reason),
        }
    }
}

/// The export row of a shareable plugin whose version is informational (the import installs the
/// latest).
pub(super) fn plugin_item(harness: HarnessId, plugin: Plugin) -> CollectedItem {
    let kind = SyncItemKind::Plugin;
    let notes = plugin.version.iter().map(|_| SyncNote::VersionInformational).collect();

    CollectedItem {
        preview: SyncExportItem {
            id: item_id(harness, kind, &plugin.id),
            version: plugin.version.clone(),
            notes,
            ..preview_row(kind, plugin.id.clone())
        },
        entry: Entry::Plugin(plugin),
    }
}

fn plugin(record: PluginRecord) -> Result<Plugin, SyncExcludedReason> {
    let PluginRecord { id, version, origin } = record;

    let marketplace = match origin {
        PluginOrigin::Marketplace { user_scope: false } => return Err(SyncExcludedReason::ProjectScope),
        PluginOrigin::Marketplace { user_scope: true } => match id.rsplit_once('@') {
            Some((name, marketplace)) if !name.is_empty() => Some(marketplace.to_owned()),
            _ => None,
        },
        PluginOrigin::Npm { spec } => {
            npm_spec(spec.as_deref())?;
            None
        }
    };

    if !exportable(&id) {
        return Err(SyncExcludedReason::ContainsSecret);
    }

    let plugin = Plugin {
        id,
        marketplace,
        version: version.filter(|version| validate::version(Some(version)).is_ok()),
    };
    validate::plugin(&plugin).map_err(excluded_reason)?;

    Ok(plugin)
}

/// An npm dependency spec is shareable when the import's `omp plugin install <name>` installs the
/// same thing: a registry version, range or tag (no `:`, `/` or `\`). Specs naming a path, a git
/// repository or a URL are left out.
fn npm_spec(spec: Option<&str>) -> Result<(), SyncExcludedReason> {
    let spec = spec.ok_or(SyncExcludedReason::Invalid)?;
    if let Some(problem) = source_problem(spec) {
        return Err(problem);
    }

    if spec.contains([':', '/', '\\']) {
        return Err(SyncExcludedReason::Unsupported);
    }

    Ok(())
}

/// PI's user packages: local and unknown kinds, and sources with secrets or credentials, are left
/// out. The source is shared as written, with any pin it has; resource filters are not shared
/// (noted on the item).
pub(super) fn packages(harness: HarnessId, paths: &HarnessPaths, found: &mut Found) {
    let kind = SyncItemKind::Package;

    for record in user_package_records(paths) {
        let label = package_label(&record.source);
        let notes = if record.filtered {
            vec![SyncNote::FiltersNotShared]
        } else {
            Vec::new()
        };

        match package(record) {
            Ok(package) => found.items.push(CollectedItem {
                preview: SyncExportItem {
                    id: item_id(harness, kind, &package_identity(&package.source)),
                    notes,
                    ..preview_row(kind, package.source.clone())
                },
                entry: Entry::Package(package),
            }),
            Err(reason) => found.exclude(harness, kind, label, reason),
        }
    }
}

fn package(record: PackageRecord) -> Result<Package, SyncExcludedReason> {
    match record.kind {
        "local" => return Err(SyncExcludedReason::LocalPath),
        "unknown" => return Err(SyncExcludedReason::Unsupported),
        _ => {}
    }

    if let Some(problem) = source_problem(&record.source) {
        return Err(problem);
    }

    let package = Package {
        source: record.source,
        version: None,
    };
    validate::package(&package).map_err(excluded_reason)?;

    Ok(package)
}

/// A name for a package that is left out, never its source: the npm package name, or the last path
/// segment of a git or URL source or of an npm spec naming a path or a URL (without `@ref` and
/// `.git`); `unknown` when that is not safe.
fn package_label(source: &str) -> String {
    let identity = package_identity(source);
    let name = match identity.strip_prefix("npm:") {
        Some(name) if !name.contains(':') && !name.starts_with(['/', '.', '~']) => name,
        _ => identity.rsplit(['/', ':', '\\']).next().unwrap_or_default(),
    };

    if (name.contains('@') && !name.starts_with('@')) || !is_safe_text(name) {
        return safe_label(None);
    }

    name.to_owned()
}
