//! Companion release lookups: the fixed URLs, tag validation, version comparison and caching.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use harness_business::ecosystem::ECOSYSTEM;
use harness_business::ecosystem::releases::{
    CompanionReleases, FAILURE_TTL, Lookup, MAX_RELEASE_BYTES, RELEASE_HOST, RELEASE_TTL, ReleaseCache, is_fresh,
    is_newer_release, parse_release, release_endpoint, release_version,
};
use harness_business::model::{EcosystemId, EcosystemInstalled, EcosystemRelease};
use pretty_assertions::assert_eq;
use serde_json::json;

const DASHBOARD_URL: &str = "https://api.github.com/repos/MSpiechowicz/harness-useful-dashboard/releases/latest";
const MEMORY_URL: &str = "https://api.github.com/repos/MSpiechowicz/harness-useful-memory/releases/latest";
const SKILLS_URL: &str = "https://api.github.com/repos/MSpiechowicz/harness-useful-skills/releases/latest";
const SIDEBAR_URL: &str = "https://api.github.com/repos/MSpiechowicz/harness-useful-sidebar/releases/latest";

/// Every lookup round asks for all four, whichever are installed (sorted like `requested`).
const ALL_URLS: [&str; 4] = [DASHBOARD_URL, MEMORY_URL, SIDEBAR_URL, SKILLS_URL];

fn installed(id: EcosystemId, version: &str) -> EcosystemInstalled {
    EcosystemInstalled {
        id,
        version: version.to_owned(),
    }
}

fn body(tag: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({ "tag_name": tag, "name": "Release", "assets": [] })).unwrap()
}

/// A fetcher answering from `answers` (URL → body) and recording every URL it was asked for.
struct FakeGitHub {
    answers: Vec<(&'static str, Option<Vec<u8>>)>,
    requested: Mutex<Vec<&'static str>>,
}

impl FakeGitHub {
    fn new(answers: Vec<(&'static str, Option<Vec<u8>>)>) -> Self {
        Self {
            answers,
            requested: Mutex::new(Vec::new()),
        }
    }

    async fn fetch(&self, url: &'static str) -> Option<Vec<u8>> {
        self.requested.lock().unwrap().push(url);

        self.answers
            .iter()
            .find(|(known, _)| *known == url)
            .and_then(|(_, answer)| answer.clone())
    }

    fn requested(&self) -> Vec<&'static str> {
        let mut urls = self.requested.lock().unwrap().clone();
        urls.sort_unstable();
        urls
    }
}

#[test]
fn each_companion_has_its_fixed_github_release_url() {
    assert_eq!(release_endpoint(EcosystemId::Dashboard), DASHBOARD_URL);
    assert_eq!(release_endpoint(EcosystemId::Memory), MEMORY_URL);
    assert_eq!(release_endpoint(EcosystemId::Skills), SKILLS_URL);
    assert_eq!(release_endpoint(EcosystemId::Sidebar), SIDEBAR_URL);

    for id in ECOSYSTEM {
        let url = release_endpoint(id);

        assert!(url.starts_with(&format!("https://{RELEASE_HOST}/")), "{url}");
        assert!(!url.contains('?'), "{url}");
    }
}

#[test]
fn release_tags_lose_their_v_and_must_look_like_versions() {
    assert_eq!(release_version("v1.25.0").as_deref(), Some("1.25.0"));
    assert_eq!(release_version("0.2.39").as_deref(), Some("0.2.39"));
    assert_eq!(release_version("v1.0.74").as_deref(), Some("1.0.74"));
    assert_eq!(release_version("v2").as_deref(), Some("2"));
    assert_eq!(release_version("v1.2.3.4").as_deref(), Some("1.2.3.4"));
    assert_eq!(release_version("v1.0.0-rc.1").as_deref(), Some("1.0.0-rc.1"));

    for tag in [
        "",
        "v",
        "latest",
        "vv1.0.0",
        "1.0.0 ",
        " 1.0.0",
        "1.0.0\n",
        "1..0",
        "1.0.0-",
        "1.0.0+build",
        "1.0.0-<script>",
        "https://example.com/1.0.0",
        "v1.0.0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "１.０.０",
    ] {
        assert_eq!(release_version(tag), None, "{tag:?}");
    }
}

#[test]
fn only_the_tag_name_of_a_bounded_json_body_is_read() {
    assert_eq!(parse_release(&body("v1.25.0")).as_deref(), Some("1.25.0"));

    assert_eq!(parse_release(b"not json"), None);
    assert_eq!(parse_release(br#"{"name":"v1.25.0"}"#), None);
    assert_eq!(parse_release(br#"{"tag_name":12}"#), None);
    assert_eq!(parse_release(&body("nightly")), None);

    let mut padded = body("v1.25.0");
    padded.truncate(padded.len() - 1);
    padded.extend(format!(r#","pad":"{}"}}"#, "x".repeat(MAX_RELEASE_BYTES)).bytes());
    assert_eq!(parse_release(&padded), None);
}

#[test]
fn only_a_higher_plain_numeric_release_counts_as_newer() {
    assert!(is_newer_release("1.10.0", "1.9.9"));
    assert!(is_newer_release("0.2.40", "0.2.36"));
    assert!(is_newer_release("v1.0.1", "1.0.0"));
    assert!(is_newer_release("1.0.0.1", "1.0.0"));
    assert!(is_newer_release("2", "1.99.99"));

    assert!(!is_newer_release("1.9.9", "1.10.0"));
    assert!(!is_newer_release("1.0", "1.0.0"));
    assert!(!is_newer_release("1.0.0", "1.0"));
    assert!(!is_newer_release("1.0.0", "v1.0.0"));

    // Anything not plainly numeric on either side is never an update.
    assert!(!is_newer_release("1.0.0.beta", "1.0.0"));
    assert!(!is_newer_release("1.2.3.rc1", "1.2.2"));
    assert!(!is_newer_release("1.0.0-rc.1", "0.9.0"));
    assert!(!is_newer_release("99999999999999999999", "1.0.0"));
    assert!(!is_newer_release("2.0.0", "99999999999999999999"));
    assert!(!is_newer_release("dev", "1.0.0"));
    assert!(!is_newer_release("2.0.0", "dev"));
    assert!(!is_newer_release("1..0", "0.1"));
    assert!(!is_newer_release("+2.0.0", "1.0.0"));
    assert!(!is_newer_release("", "1.0.0"));
}

#[test]
fn found_releases_stay_fresh_for_six_hours_and_failures_for_five_minutes() {
    assert_eq!(RELEASE_TTL, Duration::from_secs(6 * 3600));
    assert_eq!(FAILURE_TTL, Duration::from_secs(300));

    assert!(is_fresh(true, Duration::ZERO));
    assert!(is_fresh(true, RELEASE_TTL - Duration::from_secs(1)));
    assert!(!is_fresh(true, RELEASE_TTL));

    assert!(is_fresh(false, FAILURE_TTL - Duration::from_secs(1)));
    assert!(!is_fresh(false, FAILURE_TTL));
}

#[test]
fn the_cache_forgets_each_lookup_when_it_goes_stale() {
    let start = Instant::now();
    let mut cache = ReleaseCache::default();

    cache.put(EcosystemId::Skills, Lookup::Latest("0.2.40".to_owned()), start);
    cache.put(EcosystemId::Sidebar, Lookup::Failed, start);

    let later = start + FAILURE_TTL;
    assert_eq!(
        cache.get(EcosystemId::Skills, later),
        Some(&Lookup::Latest("0.2.40".to_owned()))
    );
    assert_eq!(cache.get(EcosystemId::Sidebar, later), None);
    assert_eq!(cache.get(EcosystemId::Dashboard, start), None);

    assert_eq!(cache.get(EcosystemId::Skills, start + RELEASE_TTL), None);
}

#[tokio::test]
async fn installed_companions_are_compared_with_their_latest_release() {
    let github = FakeGitHub::new(vec![
        (SKILLS_URL, Some(body("v0.2.40"))),
        (DASHBOARD_URL, Some(body("v1.22.1"))),
        (MEMORY_URL, Some(body("v1.0.0"))),
    ]);
    let releases = CompanionReleases::default();

    let found = releases
        .lookup(
            &[
                installed(EcosystemId::Skills, "0.2.36"),
                installed(EcosystemId::Dashboard, "1.22.1"),
                // The copy bundled with app builds up to 0.16 reported the app's version.
                installed(EcosystemId::Memory, "0.16.0"),
            ],
            |url| github.fetch(url),
        )
        .await;

    assert_eq!(
        found,
        vec![
            EcosystemRelease {
                id: EcosystemId::Skills,
                latest_version: "0.2.40".to_owned(),
                update_available: true,
            },
            EcosystemRelease {
                id: EcosystemId::Dashboard,
                latest_version: "1.22.1".to_owned(),
                update_available: false,
            },
            EcosystemRelease {
                id: EcosystemId::Memory,
                latest_version: "1.0.0".to_owned(),
                update_available: true,
            },
        ]
    );
    assert_eq!(github.requested(), ALL_URLS);
}

#[tokio::test]
async fn only_companions_with_a_version_are_compared_but_all_are_requested() {
    let github = FakeGitHub::new(vec![(SKILLS_URL, Some(body("v0.2.40")))]);
    let releases = CompanionReleases::default();

    let none = releases.lookup(&[], |url| github.fetch(url)).await;
    let blank = releases
        .lookup(&[installed(EcosystemId::Sidebar, "  ")], |url| github.fetch(url))
        .await;

    assert!(none.is_empty());
    assert!(blank.is_empty());
    assert!(github.requested().is_empty());

    let twice = releases
        .lookup(
            &[
                installed(EcosystemId::Skills, " 0.2.40 "),
                installed(EcosystemId::Skills, "0.0.1"),
            ],
            |url| github.fetch(url),
        )
        .await;

    assert_eq!(twice.len(), 1);
    assert!(!twice[0].update_available);
    assert_eq!(github.requested(), ALL_URLS);
}

#[tokio::test]
async fn a_failed_lookup_is_left_out_and_not_retried_at_once() {
    let github = FakeGitHub::new(vec![
        (DASHBOARD_URL, None),
        (SKILLS_URL, Some(b"{\"message\":\"API rate limit exceeded\"}".to_vec())),
        (SIDEBAR_URL, Some(body("v1.0.74"))),
    ]);
    let releases = CompanionReleases::default();
    let all = [
        installed(EcosystemId::Dashboard, "1.22.1"),
        installed(EcosystemId::Skills, "0.2.36"),
        installed(EcosystemId::Sidebar, "1.0.70"),
    ];

    let first = releases.lookup(&all, |url| github.fetch(url)).await;
    let second = releases.lookup(&all, |url| github.fetch(url)).await;

    let sidebar_only = vec![EcosystemRelease {
        id: EcosystemId::Sidebar,
        latest_version: "1.0.74".to_owned(),
        update_available: true,
    }];
    assert_eq!(first, sidebar_only);
    assert_eq!(second, sidebar_only);
    assert_eq!(github.requested(), ALL_URLS);
}

#[tokio::test]
async fn the_requests_are_the_same_whichever_companions_are_installed() {
    for id in ECOSYSTEM {
        let github = FakeGitHub::new(vec![(release_endpoint(id), Some(body("v9.0.0")))]);

        let found = CompanionReleases::default()
            .lookup(&[installed(id, "1.0.0")], |url| github.fetch(url))
            .await;

        assert_eq!(found.iter().map(|release| release.id).collect::<Vec<_>>(), vec![id]);
        assert_eq!(github.requested(), ALL_URLS, "{id:?}");
    }
}
