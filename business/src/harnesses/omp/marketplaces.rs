//! Registered OMP marketplaces (`<data root>/marketplaces.json`, version 1) and their cached
//! catalogs.
//!
//! Each registry entry names its catalog file (`catalogPath`). Because that path comes from a
//! file, it is only read after passing `canonical_within(<plugins>/cache/marketplaces)`; anything
//! else is refused with a per-marketplace error. Source URIs are shown with credentials masked.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::paths::{marketplaces_registry, plugins_dir};
use crate::env::Env;
use crate::error::BusinessError;
use crate::harness::HarnessPaths;
use crate::model::{MarketplaceEntry, MarketplaceInfo, Notice, NoticeLevel, PluginInfo};
use crate::pi_family::{Document, read_json, text_field};
use crate::security::{canonical_within, redact_text};

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
            .map(|uri| redact_text(&uri))
            .unwrap_or_default(),
        added_at: text_field(entry, "addedAt"),
        updated_at: text_field(entry, "updatedAt"),
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
    let path = contained_catalog(cache_root, catalog)?;

    let document = read_json(&path);
    let plugins = match &document {
        Document::Parsed(value) => value
            .get("plugins")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        Document::Missing => return Err(NOT_CACHED.to_owned()),
        Document::Invalid(summary) => return Err(summary.clone()),
    };

    let entries = plugins
        .iter()
        .filter_map(|plugin| {
            let name = text_field(plugin, "name")?;
            let id = format!("{name}@{marketplace}");

            Some(MarketplaceEntry {
                installed: installed.iter().any(|plugin| plugin.id == id),
                version: text_field(plugin, "version"),
                description: text_field(plugin, "description"),
                name,
            })
        })
        .collect();

    Ok(entries)
}

/// The canonical catalog path, refused unless it lies inside `cache_root`.
fn contained_catalog(cache_root: &Path, catalog: &Path) -> Result<PathBuf, String> {
    match canonical_within(cache_root, catalog) {
        Ok(path) => Ok(path),
        Err(BusinessError::NotFound(_)) => Err(NOT_CACHED.to_owned()),
        Err(BusinessError::Forbidden(_)) => Err(OUTSIDE_CACHE.to_owned()),
        Err(error) => Err(error.to_string()),
    }
}
