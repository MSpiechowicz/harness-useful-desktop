//! Registered OMP marketplaces (`<data root>/marketplaces.json`, version 1) and their cached
//! catalogs.
//!
//! Each registry entry names its catalog file (`catalogPath`). Because that path comes from a
//! file, it is only read after passing `canonical_within(<plugins>/cache/marketplaces)`; anything
//! else is refused with a per-marketplace error. Source URIs are shown with credentials masked.

use std::path::Path;

use serde_json::Value;

use super::paths::{marketplaces_registry, plugins_dir};
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::harnesses::plugin_registry::{CatalogError, contained_catalog, display_source, read_catalog};
use crate::model::{MarketplaceEntry, MarketplaceInfo, Notice, NoticeLevel, PluginInfo};
use crate::pi_family::{Document, read_json, text_field};

const OUTSIDE_CACHE: &str = "The catalog path points outside the marketplace cache, so it was not read.";
const NOT_CACHED: &str = "Catalog not cached yet; refresh the marketplace to load its plugins.";

/// Marketplaces plus notices about the registry itself.
pub fn marketplaces(env: &Env, paths: &HarnessPaths, installed: &[PluginInfo]) -> (Vec<MarketplaceInfo>, Vec<Notice>) {
    let registry = read_json(&marketplaces_registry(paths));

    if let Document::Invalid(summary) = &registry {
        let notice = Notice {
            level: NoticeLevel::Warning,
            message: format!("The marketplace registry could not be read: {summary}"),
        };
        return (Vec::new(), vec![notice]);
    }

    let cache_root = plugins_dir(paths).join("cache").join("marketplaces");
    let entries = registry
        .get("marketplaces")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let marketplaces = entries
        .iter()
        .filter(|entry| entry.is_object())
        .map(|entry| marketplace(env, &cache_root, entry, installed))
        .collect();

    (marketplaces, Vec::new())
}

/// One registry entry as written, for "Share setup" ([`crate::sync`]). Nothing is redacted: the
/// caller decides what may leave the machine and never logs or shows these values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarketplaceRecord {
    /// `name`.
    pub name: Option<String>,
    /// `sourceType` (`github`, `git`, `url`, `local`, ...).
    pub source_type: Option<String>,
    /// `sourceUri`, unmasked.
    pub source_uri: Option<String>,
}

/// The object entries of the marketplace registry, in file order; empty when it is missing or
/// unreadable.
pub(crate) fn marketplace_records(paths: &HarnessPaths) -> Vec<MarketplaceRecord> {
    let registry = read_json(&marketplaces_registry(paths));
    let Some(entries) = registry.get("marketplaces").and_then(Value::as_array) else {
        return Vec::new();
    };

    entries
        .iter()
        .filter(|entry| entry.is_object())
        .map(|entry| MarketplaceRecord {
            name: text_field(entry, "name"),
            source_type: text_field(entry, "sourceType"),
            source_uri: text_field(entry, "sourceUri"),
        })
        .collect()
}

fn marketplace(env: &Env, cache_root: &Path, entry: &Value, installed: &[PluginInfo]) -> MarketplaceInfo {
    let name = text_field(entry, "name").unwrap_or_default();
    let catalog = text_field(entry, "catalogPath").map(|raw| env.expand_home(&raw));

    let (entries, error) = match catalog_entries(cache_root, catalog.as_deref(), &name, installed) {
        Ok(entries) => (entries, None),
        Err(error) => (Vec::new(), Some(error)),
    };

    MarketplaceInfo {
        source_type: text_field(entry, "sourceType").unwrap_or_else(|| "unknown".to_owned()),
        source: text_field(entry, "sourceUri")
            .map(|uri| display_source(&uri))
            .unwrap_or_default(),
        added_at: text_field(entry, "addedAt"),
        updated_at: text_field(entry, "updatedAt"),
        refreshable: true,
        name,
        entries,
        error,
    }
}

fn catalog_entries(
    cache_root: &Path,
    catalog: Option<&Path>,
    marketplace: &str,
    installed: &[PluginInfo],
) -> Result<Vec<MarketplaceEntry>, String> {
    let catalog = catalog.ok_or_else(|| "The registry entry has no catalog path.".to_owned())?;

    contained_catalog(cache_root, catalog)
        .and_then(|path| read_catalog(&path, marketplace, installed))
        .map_err(|error| match error {
            CatalogError::NotCached => NOT_CACHED.to_owned(),
            CatalogError::Outside => OUTSIDE_CACHE.to_owned(),
            CatalogError::Unreadable(summary) => summary,
        })
}
