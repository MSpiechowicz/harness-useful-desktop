//! Marketplace catalogs (`{ plugins: [{ name, version?, description?, source }] }`, the shape OMP
//! caches and Claude Code reads from `.claude-plugin/marketplace.json`), the update marks they give
//! installed plugins ([`mark_updates`]) and how a marketplace source is shown ([`display_source`]).
//!
//! A catalog path that comes from a file is read only after [`contained_catalog`] placed it inside
//! the harness's marketplace cache.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::error::BusinessError;
use crate::model::{MarketplaceEntry, MarketplaceInfo, PluginInfo};
use crate::pi_family::{Document, read_json, text_field};
use crate::security::{canonical_within, redact_text};

/// A GitHub `owner/repo` shorthand: GitHub's own character rules for both names.
pub static GITHUB_REPO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([A-Za-z0-9](?:[A-Za-z0-9-]{0,38}))/([A-Za-z0-9._-]{1,100})$").expect("valid regex")
});

/// One part of a name between `-`, `_` and `.`: a short word, an acronym-prefixed word (`MSpiechowicz`),
/// CamelCase humps of two or more lowercase letters, or a number. Random strings (hex, base36, mixed
/// case like `QwErTy`) do not fit.
static NAME_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[A-Z]{0,3}[a-z]{2,}(?:[A-Z][a-z]{2,})*[0-9]{0,4}|[A-Z]{2,6}|[a-z]|[0-9]{1,6}|v?[0-9]+)$")
        .expect("valid regex")
});

/// Why a catalog could not be listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// The catalog file is not there (not downloaded yet).
    NotCached,
    /// The path resolves outside the allowed directory, so it was not read.
    Outside,
    /// It could not be read or parsed; a secret-free summary.
    Unreadable(String),
}

/// The canonical catalog path, refused unless it lies inside `root`.
pub fn contained_catalog(root: &Path, catalog: &Path) -> Result<PathBuf, CatalogError> {
    match canonical_within(root, catalog) {
        Ok(path) => Ok(path),
        Err(BusinessError::NotFound(_)) => Err(CatalogError::NotCached),
        Err(BusinessError::Forbidden(_)) => Err(CatalogError::Outside),
        Err(error) => Err(CatalogError::Unreadable(error.to_string())),
    }
}

/// The plugins of the catalog at `path`, each marked installed when `installed` holds
/// `<name>@<marketplace>`. Entries without a name are skipped.
pub fn read_catalog(
    path: &Path,
    marketplace: &str,
    installed: &[PluginInfo],
) -> Result<Vec<MarketplaceEntry>, CatalogError> {
    let document = read_json(path);
    let plugins = match &document {
        Document::Parsed(value) => value
            .get("plugins")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        Document::Missing => return Err(CatalogError::NotCached),
        Document::Invalid(summary) => return Err(CatalogError::Unreadable(summary.clone())),
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

/// The source URI with credentials masked. A GitHub `owner/repo` made of word-like names is a public
/// name, so it skips the entropy check that would mistake the whole shorthand for a random key; each
/// name still goes through the token rules. Anything else, including random-looking names, is masked
/// as one string exactly as before.
pub fn display_source(uri: &str) -> String {
    match GITHUB_REPO.captures(uri) {
        Some(parts) if is_word_like(&parts[1]) && is_word_like(&parts[2]) => {
            format!("{}/{}", redact_text(&parts[1]), redact_text(&parts[2]))
        }
        _ => redact_text(uri),
    }
}

/// True when every part of `name` between `-`, `_` and `.` reads like a word or a number (see
/// [`display_source`]).
pub fn is_word_like(name: &str) -> bool {
    name.split(['-', '_', '.']).all(|part| NAME_PART.is_match(part))
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
        let token = concat!("ghp", "_0123456789abcdefABCDEF0123456789abcd");
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
            concat!("deploy/", "3f9a1c7e5b2d8046af19", "c3e7b5d2084f6a1c9e3b"),
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
