//! Codex plugins and marketplaces, as `config.toml` records them (Codex CLI 0.154):
//!
//! - Plugins: `[plugins."<name>@<marketplace>"]` with `enabled` (on unless `false`). An installed
//!   plugin lives in `<home>/plugins/cache/<marketplace>/<name>/<version>/` (the highest version
//!   when there are several), with its manifest in `.codex-plugin/plugin.json` (version,
//!   description). A plugin directory is read only when it resolves inside the plugin cache.
//! - Marketplaces: `[marketplaces.<name>]` with `source_type` (`git` or `local`), `source`,
//!   `last_updated` and, for a pinned source, `ref` or `sparse_paths`. Codex keeps a git
//!   marketplace's clone in `<home>/.tmp/marketplaces/<name>`, read only inside that folder; a
//!   local one is the folder `source` names, where only the catalog file is read. The catalog is
//!   `.agents/plugins/marketplace.json` (`{ name, plugins: [{ name, source, policy, category }] }`).
//!   Sources are shown with credentials masked.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::super::paths::codex_home;
use crate::env::Env;
use crate::harnesses::plugin_registry::{
    CatalogError, contained_catalog, display_source, is_newer, read_catalog, split_id, verified_dir,
};
use crate::model::{ExtensionScope, MarketplaceInfo, Notice, NoticeLevel, PluginInfo};
use crate::pi_family::{Document, list_dir, read_json, text_field};
use crate::security::{canonical_within, redact_text};

/// The plugin manifest below a plugin directory.
const MANIFEST: &str = ".codex-plugin/plugin.json";

/// The catalog file below a marketplace folder.
const CATALOG: &str = ".agents/plugins/marketplace.json";

const NOT_DOWNLOADED: &str = "The catalog is not downloaded yet. Update the marketplace to load its plugins.";
const OUTSIDE: &str = "The marketplace folder is outside Codex's marketplace folder, so it was not read.";
const NOT_IN_CACHE: &str = "is listed in config.toml but not installed in Codex's plugin cache.";

/// An enabled plugin whose directory was verified, for its skills and servers.
#[derive(Debug, Clone)]
pub(in crate::harnesses::codex) struct EnabledPlugin {
    /// The plugin name, the namespace of what it contributes (`<name>:<skill>`).
    pub name: String,
    /// The canonical plugin directory.
    pub dir: PathBuf,
}

/// The plugins view plus the enabled plugins whose directories may be read.
#[derive(Debug, Default)]
pub(in crate::harnesses::codex) struct Plugins {
    pub plugins: Vec<PluginInfo>,
    pub enabled: Vec<EnabledPlugin>,
    pub notices: Vec<Notice>,
}

/// One plugin of `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRecord {
    /// `name@marketplace`, as written.
    pub id: String,
    /// The version installed in the plugin cache, when there is one.
    pub version: Option<String>,
    /// False when `enabled = false`.
    pub enabled: bool,
}

/// One marketplace of `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceRecord {
    /// Its name.
    pub name: String,
    /// `git` or `local`, as written.
    pub source_type: Option<String>,
    /// The source as written (a path, `owner/repo` or a git URL); empty when missing.
    pub source: String,
    /// The source is pinned to a ref or a sparse checkout.
    pub pinned: bool,
    /// When Codex last refreshed it.
    pub updated_at: Option<String>,
}

/// The plugin cache, `<home>/plugins/cache`.
fn cache_dir(env: &Env) -> PathBuf {
    codex_home(env).join("plugins").join("cache")
}

/// The plugins of the user's `config.toml` (`user`), in file order.
pub fn plugin_records_in(env: &Env, user: &Document) -> Vec<PluginRecord> {
    let Some(plugins) = user.get("plugins").and_then(Value::as_object) else {
        return Vec::new();
    };

    plugins
        .iter()
        .filter(|(_, entry)| entry.is_object())
        .map(|(id, entry)| PluginRecord {
            id: id.clone(),
            version: installed_dir(env, id).and_then(|dir| dir_version(&dir)),
            enabled: entry.get("enabled") != Some(&Value::Bool(false)),
        })
        .collect()
}

/// [`plugin_records_in`] of the user's `config.toml`.
pub fn plugin_records(env: &Env) -> Vec<PluginRecord> {
    plugin_records_in(env, &super::super::settings::user_config(env))
}

/// The marketplaces of the user's `config.toml`, in file order.
pub fn marketplace_records(env: &Env) -> Vec<MarketplaceRecord> {
    marketplace_records_in(&super::super::settings::user_config(env))
}

fn marketplace_records_in(user: &Document) -> Vec<MarketplaceRecord> {
    let Some(marketplaces) = user.get("marketplaces").and_then(Value::as_object) else {
        return Vec::new();
    };

    marketplaces
        .iter()
        .filter(|(_, entry)| entry.is_object())
        .map(|(name, entry)| {
            let sparse = entry
                .get("sparse_paths")
                .and_then(Value::as_array)
                .is_some_and(|paths| !paths.is_empty());

            MarketplaceRecord {
                name: name.clone(),
                source_type: text_field(entry, "source_type"),
                source: text_field(entry, "source").unwrap_or_default(),
                pinned: text_field(entry, "ref").is_some() || sparse,
                updated_at: text_field(entry, "last_updated"),
            }
        })
        .collect()
}

/// The installed plugins (see the module docs).
pub(in crate::harnesses::codex) fn installed(env: &Env, user: &Document) -> Plugins {
    let mut result = Plugins::default();

    for record in plugin_records_in(env, user) {
        let (name, marketplace) = split_id(&record.id);
        let dir = installed_dir(env, &record.id);
        let manifest = dir.as_deref().and_then(manifest);

        if dir.is_none() {
            result.notices.push(Notice {
                level: NoticeLevel::Info,
                message: format!("{} {NOT_IN_CACHE}", redact_text(&record.id)),
            });
        }

        let description = manifest
            .as_ref()
            .and_then(|manifest| text_field(manifest, "description"))
            .map(|text| redact_text(&text));

        result.plugins.push(PluginInfo {
            id: record.id.clone(),
            name: name.to_owned(),
            version: record.version.clone(),
            description,
            enabled: record.enabled,
            scope: ExtensionScope::User,
            marketplace: marketplace.map(str::to_owned),
            features: Vec::new(),
            path: dir.as_ref().map(|dir| dir.to_string_lossy().into_owned()),
            latest_version: None,
            update_available: false,
        });

        if let Some(dir) = dir.filter(|_| record.enabled) {
            result.enabled.push(EnabledPlugin {
                name: name.to_owned(),
                dir,
            });
        }
    }

    result
}

/// The canonical `<cache>/<marketplace>/<name>/<version>` of the highest version installed; `None`
/// when the id has no marketplace, nothing is installed or the folder resolves outside the cache.
fn installed_dir(env: &Env, id: &str) -> Option<PathBuf> {
    let (name, Some(marketplace)) = split_id(id) else {
        return None;
    };

    let cache = cache_dir(env);
    let versions = cache.join(marketplace).join(name);

    list_dir(&versions)
        .into_iter()
        .filter(|entry| entry.is_dir && !entry.name.starts_with('.'))
        .max_by(|left, right| {
            if is_newer(&left.name, &right.name) {
                std::cmp::Ordering::Greater
            } else if is_newer(&right.name, &left.name) {
                std::cmp::Ordering::Less
            } else {
                left.name.cmp(&right.name)
            }
        })
        .and_then(|entry| verified_dir(std::slice::from_ref(&cache), &entry.path))
}

/// The version of a plugin directory: its manifest's `version`, else the folder name.
fn dir_version(dir: &Path) -> Option<String> {
    manifest(dir)
        .and_then(|manifest| text_field(&manifest, "version"))
        .or_else(|| dir.file_name().map(|name| name.to_string_lossy().into_owned()))
}

/// The plugin manifest, when it resolves inside the plugin directory and parses.
fn manifest(dir: &Path) -> Option<Value> {
    let path = canonical_within(dir, &dir.join(MANIFEST)).ok()?;

    read_json(&path).value().cloned()
}

/// The marketplaces of `config.toml` with their catalogs (see the module docs).
pub(in crate::harnesses::codex) fn marketplaces(
    env: &Env,
    user: &Document,
    installed: &[PluginInfo],
) -> Vec<MarketplaceInfo> {
    let downloads = codex_home(env).join(".tmp").join("marketplaces");

    marketplace_records_in(user)
        .into_iter()
        .map(|record| {
            let (entries, error) = match catalog_entries(env, &downloads, &record, installed) {
                Ok(entries) => (entries, None),
                Err(error) => (Vec::new(), Some(error)),
            };

            MarketplaceInfo {
                source_type: record.source_type.clone().unwrap_or_else(|| "unknown".to_owned()),
                source: display_source(&record.source),
                added_at: None,
                updated_at: record.updated_at.clone(),
                entries,
                error,
                name: record.name,
            }
        })
        .collect()
}

fn catalog_entries(
    env: &Env,
    downloads: &Path,
    record: &MarketplaceRecord,
    installed: &[PluginInfo],
) -> Result<Vec<crate::model::MarketplaceEntry>, String> {
    let catalog = match record.source_type.as_deref() {
        Some("local") => Some(env.expand_home(&record.source))
            .filter(|dir| dir.is_absolute())
            .map(|dir| Ok(dir.join(CATALOG))),
        _ => Some(contained_catalog(
            downloads,
            &downloads.join(&record.name).join(CATALOG),
        )),
    };

    let catalog: Result<PathBuf, CatalogError> = catalog.unwrap_or(Err(CatalogError::NotCached));

    catalog
        .and_then(|path| read_catalog(&path, &record.name, installed))
        .map_err(|error| match error {
            CatalogError::NotCached => NOT_DOWNLOADED.to_owned(),
            CatalogError::Outside => OUTSIDE.to_owned(),
            CatalogError::Unreadable(summary) => summary,
        })
}
