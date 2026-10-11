//! The latest GitHub release of each companion tool (`ecosystem_releases`).
//!
//! The shell requests [`release_endpoint`] of every companion, nothing else, and hands the
//! response body here. All three are looked up whichever are installed, so the requests do not
//! tell GitHub what this machine has. This module owns the fixed URLs, validates the `tag_name` it
//! reads from the body, compares it with the installed version ([`is_newer_release`]) and caches
//! the outcome: a found release for [`RELEASE_TTL`], a failed lookup for [`FAILURE_TTL`] so a
//! later refresh retries.

use std::collections::HashMap;
use std::future::Future;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Deserialize;
use tokio::sync::Mutex;

use super::catalog::ECOSYSTEM;
use crate::model::{EcosystemId, EcosystemInstalled, EcosystemRelease};

/// The only host the release lookups talk to (redirects elsewhere are not followed).
pub const RELEASE_HOST: &str = "api.github.com";

/// Largest release response read; a longer body is dropped unparsed.
pub const MAX_RELEASE_BYTES: usize = 256 * 1024;

/// Longest accepted release tag.
pub const MAX_TAG_LEN: usize = 32;

/// How long a found release is reused before GitHub is asked again.
pub const RELEASE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// How long a failed lookup is remembered, so a refresh soon after retries.
pub const FAILURE_TTL: Duration = Duration::from_secs(5 * 60);

/// Longest installed version compared; anything longer is not a version the UI showed.
const MAX_INSTALLED_LEN: usize = 64;

/// `v1.25.0`, `0.2.39`, `1.0.0-rc.1`: up to four numeric parts and an optional suffix of
/// non-empty, dot-separated ASCII alphanumeric parts.
static TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^v?[0-9]+(?:\.[0-9]+){0,3}(?:[-.][0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?$").expect("static regex is valid")
});

/// The GitHub API URL of the latest release of `id`.
pub const fn release_endpoint(id: EcosystemId) -> &'static str {
    match id {
        EcosystemId::Dashboard => "https://api.github.com/repos/MSpiechowicz/harness-useful-dashboard/releases/latest",
        EcosystemId::Skills => "https://api.github.com/repos/MSpiechowicz/harness-useful-skills/releases/latest",
        EcosystemId::Sidebar => "https://api.github.com/repos/MSpiechowicz/harness-useful-sidebar/releases/latest",
    }
}

/// The version a release tag names, without a leading `v`; `None` unless the tag is a short
/// semver-like string (at most [`MAX_TAG_LEN`] characters).
pub fn release_version(tag: &str) -> Option<String> {
    if tag.len() > MAX_TAG_LEN || !TAG.is_match(tag) {
        return None;
    }

    Some(tag.strip_prefix('v').unwrap_or(tag).to_owned())
}

/// The version of the release in a GitHub "latest release" response body; `None` for a body over
/// [`MAX_RELEASE_BYTES`], invalid JSON, or a missing or invalid `tag_name`.
pub fn parse_release(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
    }

    if body.len() > MAX_RELEASE_BYTES {
        return None;
    }

    let release: Release = serde_json::from_slice(body).ok()?;

    release_version(&release.tag_name)
}

/// Whether a cached lookup made `age` ago is still used: a found release for [`RELEASE_TTL`], a
/// failure for [`FAILURE_TTL`].
pub fn is_fresh(found: bool, age: Duration) -> bool {
    let ttl = if found { RELEASE_TTL } else { FAILURE_TTL };

    age < ttl
}

/// The outcome of one release lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// The latest release's version (validated, without a leading `v`).
    Latest(String),
    /// The request, the status or the body was not usable.
    Failed,
}

/// Lookups by companion, with the time each was made.
#[derive(Debug, Default)]
pub struct ReleaseCache {
    entries: HashMap<EcosystemId, (Lookup, Instant)>,
}

impl ReleaseCache {
    /// The lookup of `id` when it is still fresh at `now` (see [`is_fresh`]).
    pub fn get(&self, id: EcosystemId, now: Instant) -> Option<&Lookup> {
        let (lookup, at) = self.entries.get(&id)?;
        let found = matches!(lookup, Lookup::Latest(_));

        is_fresh(found, now.saturating_duration_since(*at)).then_some(lookup)
    }

    /// Remembers `lookup` of `id`, made at `at`.
    pub fn put(&mut self, id: EcosystemId, lookup: Lookup, at: Instant) {
        self.entries.insert(id, (lookup, at));
    }
}

/// The release lookups of the running app and their cache.
#[derive(Debug, Default)]
pub struct CompanionReleases {
    cache: Mutex<ReleaseCache>,
}

impl CompanionReleases {
    /// The latest release of each companion in `installed` (the first entry per id with a
    /// non-empty version), in input order. Every companion without a fresh cached lookup is
    /// fetched, installed or not, in parallel with `fetch`, which gets [`release_endpoint`] and
    /// returns the response body of a `200`, or `None` on any failure. Failed lookups are left out
    /// of the result. With nothing to compare (no entry with a version) nothing is fetched.
    pub async fn lookup<F, Fut>(&self, installed: &[EcosystemInstalled], fetch: F) -> Vec<EcosystemRelease>
    where
        F: Fn(&'static str) -> Fut,
        Fut: Future<Output = Option<Vec<u8>>>,
    {
        let wanted = wanted(installed);
        if wanted.is_empty() {
            return Vec::new();
        }

        // Held across the requests on purpose: a second caller (the view opened twice, a refresh
        // during a lookup) waits and reads the cache instead of sending the same requests again.
        let mut cache = self.cache.lock().await;

        let now = Instant::now();
        let missing: Vec<EcosystemId> = ECOSYSTEM
            .into_iter()
            .filter(|id| cache.get(*id, now).is_none())
            .collect();

        // One future per companion; those with a fresh lookup finish at once without a request.
        let [first, second, third] = ECOSYSTEM.map(|id| fetch_missing(id, &missing, &fetch));
        let fetched = tokio::join!(first, second, third);

        let done = Instant::now();
        for (id, lookup) in ECOSYSTEM.into_iter().zip([fetched.0, fetched.1, fetched.2]) {
            if let Some(lookup) = lookup {
                cache.put(id, lookup, done);
            }
        }

        wanted
            .iter()
            .filter_map(|entry| match cache.get(entry.id, done) {
                Some(Lookup::Latest(latest)) => Some(release_of(entry, latest)),
                _ => None,
            })
            .collect()
    }
}

/// The comparison of `installed` with the `latest` release.
pub fn release_of(installed: &EcosystemInstalled, latest: &str) -> EcosystemRelease {
    EcosystemRelease {
        id: installed.id,
        latest_version: latest.to_owned(),
        update_available: is_newer_release(latest, &installed.version),
    }
}

/// Whether release version `latest` is higher than `installed`. Both must be dot-separated
/// numbers (a leading `v` is ignored, a missing part counts as zero); anything else, such as a
/// pre-release suffix or a part too large for `u64`, never counts as newer.
pub fn is_newer_release(latest: &str, installed: &str) -> bool {
    let (Some(latest), Some(installed)) = (numeric_parts(latest), numeric_parts(installed)) else {
        return false;
    };

    let len = latest.len().max(installed.len());
    let at = |parts: &[u64], index: usize| parts.get(index).copied().unwrap_or(0);

    (0..len)
        .map(|index| (at(&latest, index), at(&installed, index)))
        .find(|(a, b)| a != b)
        .is_some_and(|(a, b)| a > b)
}

/// The numeric parts of `version`; `None` when any part is not a plain decimal `u64`.
fn numeric_parts(version: &str) -> Option<Vec<u64>> {
    let version = version.strip_prefix('v').unwrap_or(version);

    version
        .split('.')
        .map(|part| {
            // `parse` alone would accept a leading `+`.
            let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());

            if digits { part.parse().ok() } else { None }
        })
        .collect()
}

/// The first entry per companion with a usable version, trimmed.
fn wanted(installed: &[EcosystemInstalled]) -> Vec<EcosystemInstalled> {
    let mut wanted: Vec<EcosystemInstalled> = Vec::new();

    for entry in installed {
        let version = entry.version.trim();
        let usable = !version.is_empty() && version.len() <= MAX_INSTALLED_LEN;

        if usable && !wanted.iter().any(|seen| seen.id == entry.id) {
            wanted.push(EcosystemInstalled {
                id: entry.id,
                version: version.to_owned(),
            });
        }
    }

    wanted
}

/// The lookup of `id` when it is `missing` from the cache; `None` (no request) otherwise.
async fn fetch_missing<F, Fut>(id: EcosystemId, missing: &[EcosystemId], fetch: &F) -> Option<Lookup>
where
    F: Fn(&'static str) -> Fut,
    Fut: Future<Output = Option<Vec<u8>>>,
{
    if !missing.contains(&id) {
        return None;
    }

    let Some(body) = fetch(release_endpoint(id)).await else {
        return Some(Lookup::Failed);
    };

    match parse_release(&body) {
        Some(latest) => Some(Lookup::Latest(latest)),
        None => {
            tracing::warn!(companion = ?id, "ignoring a release response without a valid tag");
            Some(Lookup::Failed)
        }
    }
}
