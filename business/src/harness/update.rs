//! Updating the harness CLIs: which npm package carries each one, the latest version npm
//! publishes ([`HarnessReleases`], `harness_releases`) and the harness's own updater
//! ([`run_update`], `harness_update`).
//!
//! The shell requests the fixed [`NpmPackage::latest_url`] of every package, nothing else, and hands
//! the response body here. Every package is looked up whichever harnesses are installed, so the
//! requests do not tell npm what this machine has. The version read from the body is validated like a
//! companion's release tag ([`release_version`]) and cached the same way: a found version for
//! [`RELEASE_TTL`](crate::ecosystem::releases::RELEASE_TTL), a failed lookup for
//! [`FAILURE_TTL`](crate::ecosystem::releases::FAILURE_TTL).

mod run;

use std::future::Future;
use std::time::Instant;

use serde::Deserialize;
use tokio::sync::Mutex;

pub use run::{UPDATE_TIMEOUT, run_update};

use crate::ecosystem::releases::{
    Lookup, MAX_RELEASE_BYTES, ReleaseCache, is_newer_release, numeric_parts, release_version, usable_version,
};
use crate::harness::OpenCodeGeneration;
use crate::model::{HarnessId, HarnessInstalled, HarnessRelease};

/// The only host the version lookups talk to (redirects elsewhere are not followed).
pub const NPM_HOST: &str = "registry.npmjs.org";

/// An npm package a harness CLI is published as.
#[derive(Debug, PartialEq, Eq)]
pub struct NpmPackage {
    /// The package name (`@openai/codex`).
    pub name: &'static str,
    /// The registry URL of its latest version.
    pub latest_url: &'static str,
}

const CLAUDE: NpmPackage = NpmPackage {
    name: "@anthropic-ai/claude-code",
    latest_url: "https://registry.npmjs.org/@anthropic-ai/claude-code/latest",
};

const CODEX: NpmPackage = NpmPackage {
    name: "@openai/codex",
    latest_url: "https://registry.npmjs.org/@openai/codex/latest",
};

const OPENCODE_1: NpmPackage = NpmPackage {
    name: "opencode-ai",
    latest_url: "https://registry.npmjs.org/opencode-ai/latest",
};

const OPENCODE_2: NpmPackage = NpmPackage {
    name: "@opencode/cli",
    latest_url: "https://registry.npmjs.org/@opencode/cli/latest",
};

const OMP: NpmPackage = NpmPackage {
    name: "@oh-my-pi/pi-coding-agent",
    latest_url: "https://registry.npmjs.org/@oh-my-pi/pi-coding-agent/latest",
};

const PI: NpmPackage = NpmPackage {
    name: "@earendil-works/pi-coding-agent",
    latest_url: "https://registry.npmjs.org/@earendil-works/pi-coding-agent/latest",
};

/// Every package, looked up on each refresh.
pub const PACKAGES: [&NpmPackage; 6] = [&CLAUDE, &CODEX, &OPENCODE_1, &OPENCODE_2, &OMP, &PI];

/// The package `harness` is published as; OpenCode 1 and 2 are told apart by the major version of
/// `installed`.
pub fn npm_package(harness: HarnessId, installed: &str) -> &'static NpmPackage {
    match harness {
        HarnessId::Claude => &CLAUDE,
        HarnessId::Codex => &CODEX,
        HarnessId::Omp => &OMP,
        HarnessId::Pi => &PI,
        HarnessId::OpenCode => match OpenCodeGeneration::from_version(installed) {
            OpenCodeGeneration::V1 => &OPENCODE_1,
            OpenCodeGeneration::V2 => &OPENCODE_2,
        },
    }
}

/// The arguments of the harness's own updater, after its program name.
pub const fn update_args(harness: HarnessId) -> &'static [&'static str] {
    match harness {
        HarnessId::Claude | HarnessId::Codex | HarnessId::Omp => &["update"],
        HarnessId::OpenCode => &["upgrade"],
        HarnessId::Pi => &["update", "--self"],
    }
}

/// The oldest version of `harness` known to have the updater command ([`update_args`]). `codex`,
/// `omp` and `pi` take a bare word as a prompt, so an older version would treat `update` as a
/// message and start an agent. Each is the earliest version with evidence of the command in the
/// upstream changelog or tags, not a verified lower bound. OpenCode's minimum depends on the
/// generation of `installed` ([`OpenCodeGeneration::from_version`]).
pub fn min_update_version(harness: HarnessId, installed: &str) -> &'static str {
    match harness {
        HarnessId::Claude => "2.0.67",
        HarnessId::Codex => "0.126.0",
        HarnessId::Omp => "18.12.5",
        // The first version with `pi update --self`.
        HarnessId::Pi => "0.70.6",
        HarnessId::OpenCode => match OpenCodeGeneration::from_version(installed) {
            OpenCodeGeneration::V1 => "1.0.0",
            // The earliest 2.x checked.
            OpenCodeGeneration::V2 => "2.2.6",
        },
    }
}

/// Whether `installed` is at least `minimum`, compared numerically. A pre-release or build suffix
/// (`-beta.1`, `+build`) counts as its numeric base, and a version that is not numbers at all (or
/// is `None`) is not enough: [`is_newer_release`] would call any of them never outdated.
pub fn has_update_command(harness: HarnessId, installed: Option<&str>) -> bool {
    let Some(installed) = installed else {
        return false;
    };

    let base = installed.trim().split(['-', '+']).next().unwrap_or_default();
    let minimum = min_update_version(harness, installed);

    numeric_parts(base).is_some() && !is_newer_release(minimum, base)
}

/// The updater's command line for display (`pi update --self`).
pub fn update_command(harness: HarnessId) -> String {
    format!("{} {}", harness.binary_name(), update_args(harness).join(" "))
}

/// The latest versions npm publishes and their cache.
#[derive(Debug, Default)]
pub struct HarnessReleases {
    cache: Mutex<ReleaseCache<&'static str>>,
}

impl HarnessReleases {
    /// The latest version of each harness in `installed` (the first entry per harness with a
    /// non-empty version), in input order. Every package without a fresh cached lookup is fetched,
    /// installed or not, in parallel with `fetch`, which gets [`NpmPackage::latest_url`] and
    /// returns the response body of a `200`, or `None` on any failure. Harnesses whose lookup
    /// failed are left out. With nothing to compare (no entry with a version) nothing is fetched.
    pub async fn lookup<F, Fut>(&self, installed: &[HarnessInstalled], fetch: F) -> Vec<HarnessRelease>
    where
        F: Fn(&'static str) -> Fut,
        Fut: Future<Output = Option<Vec<u8>>>,
    {
        let wanted = wanted(installed);
        if wanted.is_empty() {
            return Vec::new();
        }

        // Held across the requests on purpose: a second caller waits and reads the cache instead of
        // sending the same requests again.
        let mut cache = self.cache.lock().await;

        let now = Instant::now();
        let [first, second, third, fourth, fifth, sixth] = PACKAGES.map(|package| {
            let stale = cache.get(package.name, now).is_none();

            fetch_stale(package, stale, &fetch)
        });
        let fetched = tokio::join!(first, second, third, fourth, fifth, sixth);

        let done = Instant::now();
        for (package, lookup) in PACKAGES
            .into_iter()
            .zip([fetched.0, fetched.1, fetched.2, fetched.3, fetched.4, fetched.5])
        {
            if let Some(lookup) = lookup {
                cache.put(package.name, lookup, done);
            }
        }

        wanted
            .iter()
            .filter_map(|entry| {
                let package = npm_package(entry.harness, &entry.version);

                match cache.get(package.name, done) {
                    Some(Lookup::Latest(latest)) => Some(release_of(entry, latest)),
                    _ => None,
                }
            })
            .collect()
    }
}

/// The comparison of `installed` with the `latest` version.
fn release_of(installed: &HarnessInstalled, latest: &str) -> HarnessRelease {
    HarnessRelease {
        harness: installed.harness,
        latest_version: latest.to_owned(),
        update_available: is_newer_release(latest, &installed.version),
        update_command: update_command(installed.harness),
    }
}

/// The version of the `latest` document of an npm package; `None` for a body over
/// [`MAX_RELEASE_BYTES`], invalid JSON, or a missing or invalid `version`.
fn parse_latest(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Latest {
        version: String,
    }

    if body.len() > MAX_RELEASE_BYTES {
        return None;
    }

    let latest: Latest = serde_json::from_slice(body).ok()?;

    release_version(&latest.version)
}

/// The first entry per harness with a usable version, trimmed.
fn wanted(installed: &[HarnessInstalled]) -> Vec<HarnessInstalled> {
    let mut wanted: Vec<HarnessInstalled> = Vec::new();

    for entry in installed {
        let Some(version) = usable_version(&entry.version) else {
            continue;
        };

        if !wanted.iter().any(|seen| seen.harness == entry.harness) {
            wanted.push(HarnessInstalled {
                harness: entry.harness,
                version: version.to_owned(),
            });
        }
    }

    wanted
}

/// The lookup of `package` when it is `stale` in the cache; `None` (no request) otherwise.
async fn fetch_stale<F, Fut>(package: &NpmPackage, stale: bool, fetch: &F) -> Option<Lookup>
where
    F: Fn(&'static str) -> Fut,
    Fut: Future<Output = Option<Vec<u8>>>,
{
    if !stale {
        return None;
    }

    let Some(body) = fetch(package.latest_url).await else {
        return Some(Lookup::Failed);
    };

    match parse_latest(&body) {
        Some(latest) => Some(Lookup::Latest(latest)),
        None => {
            tracing::warn!(
                package = package.name,
                "ignoring an npm response without a valid version"
            );
            Some(Lookup::Failed)
        }
    }
}
