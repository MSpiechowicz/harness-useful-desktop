//! Claude Code marketplaces: `<config>/plugins/known_marketplaces.json`, an object keyed by name
//! (`{ <name>: { source: { source: "github", repo } | { source: "git", url } | { source:
//! "directory", path } | …, installLocation, lastUpdated, autoUpdate? } }`), and the catalog each
//! one keeps in `.claude-plugin/marketplace.json`.
//!
//! A downloaded marketplace lives in `<config>/plugins/marketplaces`, so its catalog is read only
//! inside that folder. A `directory` (or `file`) marketplace is a folder the user picked, so only its
//! catalog file is read there, through [`SafeReader`](crate::security::SafeReader). Marketplaces
//! named in the settings' `extraKnownMarketplaces` that Claude has not added yet are listed
//! without a catalog. Sources are shown with credentials masked.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::plugins::plugins_dir;
use super::settings::Settings;
use crate::env::Env;
use crate::harnesses::plugin_registry::{CatalogError, contained_catalog, display_source, read_catalog};
use crate::model::{MarketplaceEntry, MarketplaceInfo, Notice, NoticeLevel, PluginInfo};
use crate::pi_family::{Document, read_json, text_field};

/// The catalog file below a marketplace folder.
const CATALOG: &str = ".claude-plugin/marketplace.json";

const NOT_DOWNLOADED: &str = "The catalog is not downloaded yet. Update the marketplace to load its plugins.";
const OUTSIDE: &str = "The marketplace folder is outside Claude Code's plugins folder, so it was not read.";
const NOT_ADDED: &str = "Listed in your settings but not added to Claude Code yet.";

/// Marketplaces plus notices about the marketplace list itself.
pub(super) fn marketplaces(
    env: &Env,
    settings: &Settings,
    installed: &[PluginInfo],
) -> (Vec<MarketplaceInfo>, Vec<Notice>) {
    let entries = match known_entries(env) {
        Ok(entries) => entries,
        Err(summary) => {
            let notice = Notice {
                level: NoticeLevel::Warning,
                message: format!("The marketplace list could not be read: {summary}"),
            };
            return (Vec::new(), vec![notice]);
        }
    };

    let downloads = plugins_dir(env).join("marketplaces");

    let mut marketplaces: Vec<MarketplaceInfo> = entries
        .iter()
        .filter(|(_, entry)| entry.is_object())
        .map(|(name, entry)| {
            let (entries, error) = match catalog_entries(env, &downloads, name, entry, installed) {
                Ok(entries) => (entries, None),
                Err(error) => (Vec::new(), Some(error)),
            };

            MarketplaceInfo {
                updated_at: text_field(entry, "lastUpdated"),
                entries,
                error,
                ..described(name, entry.get("source"))
            }
        })
        .collect();

    for (name, entry) in settings.merged("extraKnownMarketplaces") {
        if marketplaces.iter().any(|known| known.name == name) {
            continue;
        }

        marketplaces.push(MarketplaceInfo {
            error: Some(NOT_ADDED.to_owned()),
            ..described(&name, entry.get("source"))
        });
    }

    (marketplaces, Vec::new())
}

/// The entries of `known_marketplaces.json` by name, as written (a missing file has none, an
/// entry that is not an object is kept); the error is a secret-free summary.
pub(in crate::harnesses::claude) fn known_entries(env: &Env) -> Result<Map<String, Value>, String> {
    match read_json(&plugins_dir(env).join("known_marketplaces.json")) {
        Document::Invalid(summary) => Err(summary),
        known => Ok(known.value().and_then(Value::as_object).cloned().unwrap_or_default()),
    }
}

/// Where a marketplace `source` object points, as written: its `repo` (GitHub), `url` (git or a
/// catalog URL), `path` (a folder or file) or `package` (npm).
pub(in crate::harnesses::claude) fn source_location(source: &Value) -> Option<String> {
    ["repo", "url", "path", "package"]
        .iter()
        .find_map(|key| text_field(source, key))
}

/// A marketplace without a catalog yet: its name and source.
fn described(name: &str, source: Option<&Value>) -> MarketplaceInfo {
    let source = source.cloned().unwrap_or(Value::Null);
    let shown = source_location(&source);

    MarketplaceInfo {
        name: name.to_owned(),
        source_type: text_field(&source, "source").unwrap_or_else(|| "unknown".to_owned()),
        source: shown.map(|source| display_source(&source)).unwrap_or_default(),
        added_at: None,
        updated_at: None,
        entries: Vec::new(),
        error: None,
    }
}

fn catalog_entries(
    env: &Env,
    downloads: &Path,
    name: &str,
    entry: &Value,
    installed: &[PluginInfo],
) -> Result<Vec<MarketplaceEntry>, String> {
    let source = entry.get("source").cloned().unwrap_or(Value::Null);
    let picked = |key: &str| {
        text_field(&source, key)
            .map(|raw| env.expand_home(&raw))
            .filter(|path| path.is_absolute())
    };

    let catalog = match text_field(&source, "source").as_deref() {
        Some("directory") => picked("path").map(|dir| Ok(dir.join(CATALOG))),
        Some("file") => picked("path").map(Ok),
        _ => text_field(entry, "installLocation")
            .map(|raw| contained_catalog(downloads, &env.expand_home(&raw).join(CATALOG))),
    };

    let catalog: Result<PathBuf, CatalogError> = catalog.unwrap_or(Err(CatalogError::NotCached));

    catalog
        .and_then(|path| read_catalog(&path, name, installed))
        .map_err(|error| match error {
            CatalogError::NotCached => NOT_DOWNLOADED.to_owned(),
            CatalogError::Outside => OUTSIDE.to_owned(),
            CatalogError::Unreadable(summary) => summary,
        })
}
