//! The harness CLIs' npm packages, their updater commands and the latest-version lookup.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use harness_business::ecosystem::releases::{FAILURE_TTL, Lookup, MAX_RELEASE_BYTES, RELEASE_TTL, ReleaseCache};
use harness_business::harness::update::{
    HarnessReleases, NPM_HOST, PACKAGES, npm_package, update_args, update_command,
};
use harness_business::model::{HarnessId, HarnessInstalled, HarnessRelease};
use pretty_assertions::assert_eq;
use serde_json::json;

const CLAUDE_URL: &str = "https://registry.npmjs.org/@anthropic-ai/claude-code/latest";
const CODEX_URL: &str = "https://registry.npmjs.org/@openai/codex/latest";
const OPENCODE_1_URL: &str = "https://registry.npmjs.org/opencode-ai/latest";
const OPENCODE_2_URL: &str = "https://registry.npmjs.org/@opencode/cli/latest";
const OMP_URL: &str = "https://registry.npmjs.org/@oh-my-pi/pi-coding-agent/latest";
const PI_URL: &str = "https://registry.npmjs.org/@earendil-works/pi-coding-agent/latest";

/// Every lookup round asks for all six, whichever harnesses are installed (sorted like `requested`).
const ALL_URLS: [&str; 6] = [OMP_URL, PI_URL, CLAUDE_URL, CODEX_URL, OPENCODE_1_URL, OPENCODE_2_URL];

fn installed(harness: HarnessId, version: &str) -> HarnessInstalled {
    HarnessInstalled {
        harness,
        version: version.to_owned(),
    }
}

fn body(version: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({ "name": "package", "version": version, "dist": {}, "bin": {} })).unwrap()
}

/// A fetcher answering from a map (URL to body; a URL not in it fails) and noting each URL it
/// is asked for.
struct FakeNpm {
    bodies: HashMap<&'static str, Vec<u8>>,
    asked: Mutex<Vec<&'static str>>,
}

impl FakeNpm {
    fn new(answers: Vec<(&'static str, Option<Vec<u8>>)>) -> Self {
        let bodies = answers
            .into_iter()
            .filter_map(|(url, answer)| Some((url, answer?)))
            .collect();

        Self {
            bodies,
            asked: Mutex::default(),
        }
    }

    async fn fetch(&self, url: &'static str) -> Option<Vec<u8>> {
        self.asked.lock().unwrap().push(url);

        self.bodies.get(url).cloned()
    }

    /// The URLs asked for so far, in the order of [`ALL_URLS`], and forgotten.
    fn take_requested(&self) -> Vec<&'static str> {
        let mut urls = std::mem::take(&mut *self.asked.lock().unwrap());
        urls.sort_unstable_by_key(|url| ALL_URLS.iter().position(|known| known == url));
        urls
    }
}

#[test]
fn each_harness_has_its_package_and_updater_command() {
    let cases = [
        (HarnessId::Claude, "1.0.0", "@anthropic-ai/claude-code", "claude update"),
        (HarnessId::Codex, "0.1.0", "@openai/codex", "codex update"),
        (HarnessId::OpenCode, "1.19.3", "opencode-ai", "opencode upgrade"),
        (HarnessId::OpenCode, "2.1.2", "@opencode/cli", "opencode upgrade"),
        (HarnessId::Omp, "18.6.1", "@oh-my-pi/pi-coding-agent", "omp update"),
        (
            HarnessId::Pi,
            "1.1.0",
            "@earendil-works/pi-coding-agent",
            "pi update --self",
        ),
    ];

    for (harness, version, package, command) in cases {
        assert_eq!(npm_package(harness, version).name, package, "{harness:?} {version}");
        assert_eq!(update_command(harness), command, "{harness:?}");
    }

    assert_eq!(update_args(HarnessId::Pi), ["update", "--self"]);
    assert_eq!(update_args(HarnessId::OpenCode), ["upgrade"]);
}

#[test]
fn opencode_major_two_and_later_is_the_second_package() {
    for (version, package) in [
        ("1.19.3", "opencode-ai"),
        ("v2.0.0", "@opencode/cli"),
        ("3.0.0", "@opencode/cli"),
        ("unknown", "opencode-ai"),
    ] {
        assert_eq!(npm_package(HarnessId::OpenCode, version).name, package, "{version}");
    }
}

#[test]
fn every_package_has_a_fixed_https_registry_url() {
    assert_eq!(PACKAGES.map(|package| package.latest_url).len(), ALL_URLS.len());

    for package in PACKAGES {
        let url = package.latest_url;

        assert_eq!(url, format!("https://{NPM_HOST}/{}/latest", package.name));
        assert!(ALL_URLS.contains(&url), "{url}");
        assert!(!url.contains('?'), "{url}");
    }
}

#[tokio::test]
async fn installed_harnesses_are_compared_with_their_latest_version() {
    let npm = FakeNpm::new(vec![
        (CLAUDE_URL, Some(body("2.1.319"))),
        (PI_URL, Some(body("1.5.0"))),
        (OMP_URL, Some(body("18.18.1"))),
    ]);
    let releases = HarnessReleases::default();

    let found = releases
        .lookup(
            &[
                installed(HarnessId::Claude, "2.1.300"),
                installed(HarnessId::Pi, "1.5.0"),
                installed(HarnessId::Omp, "18.6.1"),
            ],
            |url| npm.fetch(url),
        )
        .await;

    let release = |harness, latest: &str, update_available, command: &str| HarnessRelease {
        harness,
        latest_version: latest.to_owned(),
        update_available,
        update_command: command.to_owned(),
    };
    assert_eq!(
        found,
        vec![
            release(HarnessId::Claude, "2.1.319", true, "claude update"),
            release(HarnessId::Pi, "1.5.0", false, "pi update --self"),
            release(HarnessId::Omp, "18.18.1", true, "omp update"),
        ]
    );
    assert_eq!(npm.take_requested(), ALL_URLS);
}

#[tokio::test]
async fn opencode_is_compared_with_the_package_of_its_major_version() {
    let answers = vec![
        (OPENCODE_1_URL, Some(body("1.19.5"))),
        (OPENCODE_2_URL, Some(body("2.2.6"))),
    ];

    for (version, latest) in [("1.19.3", "1.19.5"), ("2.1.2", "2.2.6")] {
        let npm = FakeNpm::new(answers.clone());

        let found = HarnessReleases::default()
            .lookup(&[installed(HarnessId::OpenCode, version)], |url| npm.fetch(url))
            .await;

        assert_eq!(found.len(), 1, "{version}");
        assert_eq!(found[0].latest_version, latest, "{version}");
        assert!(found[0].update_available, "{version}");
    }
}

#[tokio::test]
async fn a_suffixed_installed_version_is_not_offered_an_update() {
    let npm = FakeNpm::new(vec![(CODEX_URL, Some(body("0.167.0")))]);

    let found = HarnessReleases::default()
        .lookup(&[installed(HarnessId::Codex, "0.160.0-alpha.2")], |url| npm.fetch(url))
        .await;

    assert_eq!(found.len(), 1);
    assert!(!found[0].update_available);
}

#[tokio::test]
async fn only_harnesses_with_a_version_are_compared_but_all_packages_are_requested() {
    let npm = FakeNpm::new(vec![(CODEX_URL, Some(body("0.167.0")))]);
    let releases = HarnessReleases::default();

    let none = releases.lookup(&[], |url| npm.fetch(url)).await;
    let blank = releases
        .lookup(&[installed(HarnessId::Pi, "  ")], |url| npm.fetch(url))
        .await;

    assert!(none.is_empty());
    assert!(blank.is_empty());
    assert!(npm.take_requested().is_empty());

    let twice = releases
        .lookup(
            &[
                installed(HarnessId::Codex, " 0.100.0 "),
                installed(HarnessId::Codex, "0.167.0"),
            ],
            |url| npm.fetch(url),
        )
        .await;

    assert_eq!(twice.len(), 1);
    assert!(twice[0].update_available);
    assert_eq!(npm.take_requested(), ALL_URLS);
}

#[tokio::test]
async fn found_versions_are_reused_and_failed_lookups_are_left_out() {
    let npm = FakeNpm::new(vec![
        (CLAUDE_URL, None),
        (CODEX_URL, Some(b"{\"error\":\"Not found\"}".to_vec())),
        (PI_URL, Some(body("1.5.0"))),
    ]);
    let releases = HarnessReleases::default();
    let all = [
        installed(HarnessId::Claude, "2.1.0"),
        installed(HarnessId::Codex, "0.100.0"),
        installed(HarnessId::Pi, "1.1.0"),
    ];

    let first = releases.lookup(&all, |url| npm.fetch(url)).await;
    assert_eq!(npm.take_requested(), ALL_URLS);

    let second = releases.lookup(&all, |url| npm.fetch(url)).await;

    let pi_only = vec![HarnessRelease {
        harness: HarnessId::Pi,
        latest_version: "1.5.0".to_owned(),
        update_available: true,
        update_command: "pi update --self".to_owned(),
    }];
    assert_eq!(first, pi_only);
    assert_eq!(second, pi_only);
    // Neither the found version nor the recent failures are asked for again.
    assert!(npm.take_requested().is_empty());
}

#[tokio::test]
async fn only_a_valid_version_field_of_a_bounded_body_is_used() {
    let mut padded = body("1.5.0");
    padded.truncate(padded.len() - 1);
    padded.extend(format!(r#","pad":"{}"}}"#, "x".repeat(MAX_RELEASE_BYTES)).bytes());

    let answers = [
        b"not json".to_vec(),
        br#"{"name":"pi"}"#.to_vec(),
        br#"{"version":15}"#.to_vec(),
        body("latest"),
        body("1.5.0 && rm"),
        padded,
    ];

    for answer in answers {
        let npm = FakeNpm::new(vec![(PI_URL, Some(answer))]);

        let found = HarnessReleases::default()
            .lookup(&[installed(HarnessId::Pi, "1.1.0")], |url| npm.fetch(url))
            .await;

        assert!(found.is_empty());
    }
}

#[test]
fn the_package_cache_keeps_found_versions_longer_than_failures() {
    let start = Instant::now();
    let mut cache: ReleaseCache<&'static str> = ReleaseCache::default();

    cache.put("@openai/codex", Lookup::Latest("0.167.0".to_owned()), start);
    cache.put("opencode-ai", Lookup::Failed, start);

    let later = start + FAILURE_TTL;
    assert_eq!(
        cache.get("@openai/codex", later),
        Some(&Lookup::Latest("0.167.0".to_owned()))
    );
    assert_eq!(cache.get("opencode-ai", later), None);
    assert_eq!(cache.get("@openai/codex", start + RELEASE_TTL), None);
    assert_eq!(RELEASE_TTL, Duration::from_secs(6 * 3600));
}
