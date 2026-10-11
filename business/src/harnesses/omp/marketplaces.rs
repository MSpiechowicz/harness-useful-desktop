//! Registered OMP marketplaces (`<data root>/marketplaces.json`, version 1) and their cached
//! catalogs.
//!
//! Each registry entry names its catalog file (`catalogPath`). Because that path comes from a
//! file, it is only read after passing `canonical_within(<plugins>/cache/marketplaces)`; anything
//! else is refused with a per-marketplace error. Source URIs are shown with credentials masked.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
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

/// A GitHub `owner/repo` shorthand: GitHub's own character rules for both names.
static GITHUB_REPO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([A-Za-z0-9](?:[A-Za-z0-9-]{0,38}))/([A-Za-z0-9._-]{1,100})$").expect("valid regex")
});

/// One part of a name between `-`, `_` and `.`: a short word, an acronym-prefixed word (`MSpiechowicz`),
/// CamelCase humps of two or more lowercase letters, or a number. Random strings (hex, base36, mixed
/// case like `QwErTy`) do not fit.
static NAME_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[A-Z]{0,3}[a-z]{2,}(?:[A-Z][a-z]{2,})*[0-9]{0,4}|[A-Z]{2,6}|[a-z]|[0-9]{1,6}|v?[0-9]+)$")
        .expect("valid regex")
});

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
            .map(|uri| display_source(&uri))
            .unwrap_or_default(),
        added_at: text_field(entry, "addedAt"),
        updated_at: text_field(entry, "updatedAt"),
        name,
        entries,
        error,
    }
}

/// The source URI with credentials masked. A GitHub `owner/repo` made of word-like names is a public
/// name, so it skips the entropy check that would mistake the whole shorthand for a random key; each
/// name still goes through the token rules. Anything else, including random-looking names, is masked
/// as one string exactly as before.
fn display_source(uri: &str) -> String {
    match GITHUB_REPO.captures(uri) {
        Some(parts) if is_word_like(&parts[1]) && is_word_like(&parts[2]) => {
            format!("{}/{}", redact_text(&parts[1]), redact_text(&parts[2]))
        }
        _ => redact_text(uri),
    }
}

fn is_word_like(name: &str) -> bool {
    name.split(['-', '_', '.']).all(|part| NAME_PART.is_match(part))
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

/// Marks each marketplace plugin with the version its catalog offers and whether that is newer
/// than the installed one. Plugins without a cached catalog entry are left untouched.
pub fn mark_updates(plugins: &mut [PluginInfo], marketplaces: &[MarketplaceInfo]) {
    for plugin in plugins {
        let Some(marketplace) = plugin.marketplace.as_deref() else {
            continue;
        };
        let name = plugin.id.rsplit_once('@').map_or(plugin.id.as_str(), |(name, _)| name);
        let latest = marketplaces
            .iter()
            .find(|candidate| candidate.name == marketplace)
            .and_then(|candidate| candidate.entries.iter().find(|entry| entry.name == name))
            .and_then(|entry| entry.version.clone());

        plugin.update_available = match (&latest, &plugin.version) {
            (Some(latest), Some(installed)) => is_newer(latest, installed),
            (Some(_), None) => true,
            (None, _) => false,
        };
        plugin.latest_version = latest;
    }
}

/// Whether `candidate` is a higher version than `current`: numeric dot-separated parts compared
/// in order, a missing part counting as zero; anything unparsable falls back to plain inequality.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Option<Vec<u64>> {
        version
            .trim_start_matches('v')
            .split(['-', '+'])
            .next()?
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect()
    }

    match (parts(candidate), parts(current)) {
        (Some(a), Some(b)) => {
            let len = a.len().max(b.len());
            let at = |list: &Vec<u64>, index: usize| list.get(index).copied().unwrap_or(0);
            (0..len)
                .map(|index| (at(&a, index), at(&b, index)))
                .find(|(x, y)| x != y)
                .is_some_and(|(x, y)| x > y)
        }
        _ => candidate != current,
    }
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

#[cfg(test)]
mod tests {
    use super::{display_source, is_newer};
    use crate::security::redact_text;

    #[test]
    fn shows_github_repos_in_full() {
        for repo in [
            "MSpiechowicz/oh-my-pi-useful-skills",
            "MSpiechowicz/harness-usage-dashboard",
            "anthropics/claude-plugins.official",
            "MSpiechowicz/oh-my-pi-anvil",
            "vercel/next.js",
            "HarnessUseful/HarnessDesktop",
            "acme/tool-v2",
        ] {
            assert_eq!(display_source(repo), repo);
        }
    }

    #[test]
    fn still_masks_credentials_in_sources() {
        let token = "ghp_0123456789abcdefABCDEF0123456789abcd";
        let shown = display_source(&format!("someone/{token}"));
        assert!(!shown.contains(token), "token leaked: {shown}");

        let url = format!("https://x-access-token:{token}@github.com/someone/repo.git");
        assert!(!display_source(&url).contains(token));
        assert_eq!(display_source(&url), redact_text(&url));

        let opaque = "Zq8rT2vLx9KpW4mN7bYc3HdF6gJs5AeU1oRi";
        assert!(!display_source(opaque).contains(opaque));
    }

    /// Random strings shaped like `owner/repo` must not slip past the entropy check by being split at `/`.
    #[test]
    fn masks_random_strings_shaped_like_a_repo() {
        for secret in [
            "Xk3vQ9pLm2RtZ8wNa7Bc/Df4Gh6Jk1LqPs5TuVyW",
            "deploy/3f9a1c7e5b2d8046af19c3e7b5d2084f6a1c9e3b",
            "team/QwErTyUiOpAsDfGhJkLzXcVbNmQaZwSxEdCrFv",
            "org/k3j9x2m7q8w1p5z0r4t6y9u2i8o3a7s1d5f0g4h6",
            "aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bC2d/fG4hJ6kL8nP0qR2tU4wX6zA8cE0gI2k",
        ] {
            assert_eq!(display_source(secret), redact_text(secret), "{secret}");
            assert!(!display_source(secret).contains(secret), "{secret}");
        }
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("1.3.0", "1.2.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2", "1.9.9"));
        assert!(is_newer("v0.2.30", "0.2.28"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("1.2.0", "1.3.0"));
        assert!(!is_newer("0.2.28", "0.2.28-dev"));
    }

    #[test]
    fn falls_back_to_inequality_for_odd_versions() {
        assert!(is_newer("nightly-b", "nightly-a"));
        assert!(!is_newer("nightly", "nightly"));
    }
}
