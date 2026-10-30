//! The code index in Harness Memory: its switch through `code status|enable|disable --json`
//! against the stand-in of `fake_agent` (see `src/bin/fake_agent/memory_code.rs`), and the build
//! status files the server writes to `code-builds/`, which the watcher turns into events for a
//! toast (a new build, progress in 10 % steps, ready, error, cancelled, stale; nothing for a file
//! that did not change, and nothing for files it must not trust).

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_business::BusinessError;
use harness_business::ecosystem::memory_code::{BUILDS_DIR, CodeBuildWatcher, MAX_STATUS_BYTES, STALE_AFTER};
use harness_business::ecosystem::{
    CodeGraphSupport, FeaturesSupport, MemoryFeatures, code_graph_support, feature_server, get_code_index,
    set_code_index,
};
use harness_business::model::CodeBuildEvent;
use memory_stand_in::{CURRENT, Setup};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::memory_requests::private;
use crate::memory_stand_in;

const NAME: &str = "0123456789abcdef.json";
const OTHER: &str = "fedcba9876543210.json";

#[tokio::test]
async fn the_switch_round_trips_through_the_server_that_lists_code_index() {
    let setup = Setup::installed(CURRENT, &[]);
    let server = feature_server(
        &setup.env,
        setup.runner(),
        &MemoryFeatures::new(),
        FeaturesSupport::code_index,
        "the code index",
    )
    .await
    .expect("code-index is listed");

    let first = get_code_index(&setup.env, setup.runner(), &server).await.unwrap();
    assert_eq!((first.enabled, first.source.as_str()), (true, "default"));

    let off = set_code_index(&setup.env, setup.runner(), &server, false)
        .await
        .unwrap();
    assert_eq!((off.enabled, off.source.as_str()), (false, "file"));
    assert!(
        !get_code_index(&setup.env, setup.runner(), &server)
            .await
            .unwrap()
            .enabled
    );

    let on = set_code_index(&setup.env, setup.runner(), &server, true).await.unwrap();
    assert!(on.enabled);

    let commands: Vec<Vec<String>> = (0..setup.runs().len()).map(|run| setup.run_args(run)).collect();
    assert_eq!(
        commands[1..],
        [
            vec!["code", "status", "--json"],
            vec!["code", "disable", "--json"],
            vec!["code", "status", "--json"],
            vec!["code", "enable", "--json"],
        ]
    );
}

async fn support_of(setup: &Setup) -> CodeGraphSupport {
    code_graph_support(&setup.env, setup.runner(), &MemoryFeatures::new()).await
}

#[tokio::test]
async fn the_code_graph_of_the_hooks_needs_the_feature_and_follows_the_switch() {
    let on = Setup::installed(CURRENT, &[]);
    assert_eq!(support_of(&on).await, CodeGraphSupport { listed: true, on: true });

    let off = Setup::installed(CURRENT, &[("FAKE_CODE_INDEX", "off")]);
    assert_eq!(
        support_of(&off).await,
        CodeGraphSupport {
            listed: true,
            on: false
        }
    );

    let older = Setup::installed(CURRENT, &[("FAKE_MEMORY_FEATURES", "embedder")]);
    assert_eq!(support_of(&older).await, CodeGraphSupport::default());

    let absent = Setup::without_server(CURRENT, &[]);
    assert_eq!(support_of(&absent).await, CodeGraphSupport::default());
}

#[tokio::test]
async fn the_environment_overrides_are_reported_as_their_source() {
    let setup = Setup::installed(CURRENT, &[("FAKE_CODE_INDEX", "off")]);

    let state = get_code_index(&setup.env, setup.runner(), &setup.binary).await.unwrap();

    assert_eq!((state.enabled, state.source.as_str()), (false, "env"));
}

#[tokio::test]
async fn a_server_that_cannot_write_the_choice_answers_with_its_line() {
    let setup = Setup::installed(CURRENT, &[("FAKE_CODE_REFUSE", "1")]);

    let refused = set_code_index(&setup.env, setup.runner(), &setup.binary, false).await;

    match refused {
        Err(BusinessError::Process(line)) => assert!(line.contains("cannot write code.toml"), "{line}"),
        other => panic!("expected the server's line, got {other:?}"),
    }
}

#[tokio::test]
async fn an_older_server_is_unsupported_and_a_missing_one_not_found() {
    let older = Setup::installed(CURRENT, &[("FAKE_MEMORY_FEATURES", "embedder")]);

    let refused = older
        .feature_server(FeaturesSupport::code_index, "the code index")
        .await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");

    let usage = get_code_index(&older.env, older.runner(), &older.binary).await;
    assert!(
        matches!(usage, Err(BusinessError::Unsupported(_))),
        "the usage error of an older server: {usage:?}"
    );

    let absent = Setup::without_server(CURRENT, &[]);
    let refused = absent
        .feature_server(FeaturesSupport::code_index, "the code index")
        .await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");
}

struct Data {
    _dir: TempDir,
    root: PathBuf,
}

impl Data {
    /// A data folder with a private `code-builds/` in it, as the server creates them.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("harness-memory");
        std::fs::create_dir_all(root.join(BUILDS_DIR)).unwrap();
        private(&root.join(BUILDS_DIR));

        Self { _dir: dir, root }
    }

    fn builds(&self) -> PathBuf {
        self.root.join(BUILDS_DIR)
    }

    fn write(&self, name: &str, status: &Value) -> PathBuf {
        let path = self.builds().join(name);
        std::fs::write(&path, status.to_string()).unwrap();
        path
    }
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn building(done: u64) -> Value {
    json!({
        "version": 1, "buildId": 7, "root": "/work/demo", "repoName": "demo", "kind": "full",
        "phase": "building", "filesTotal": 100, "filesDone": done,
        "startedAt": seconds(SystemTime::now()), "finishedAt": null, "error": null
    })
}

fn finished(phase: &str, error: Option<&str>, finished_at: SystemTime) -> Value {
    json!({
        "version": 1, "buildId": 7, "root": "/work/demo", "repoName": "demo", "kind": "full",
        "phase": phase, "filesTotal": 100, "filesDone": 100,
        "startedAt": seconds(finished_at) - 5, "finishedAt": seconds(finished_at), "error": error
    })
}

fn phases(events: &[CodeBuildEvent]) -> Vec<(&str, u64)> {
    events
        .iter()
        .map(|event| (event.phase.as_str(), event.files_done))
        .collect()
}

#[test]
fn a_new_build_is_announced_and_so_is_its_end() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(NAME, &building(0));
    let started = watcher.poll(now);
    assert_eq!(phases(&started), [("building", 0)]);
    assert_eq!(started[0].repo_name, "demo");
    assert_eq!(started[0].root, "/work/demo");
    assert_eq!((started[0].build_id, started[0].files_total), (7, 100));

    data.write(NAME, &finished("ready", None, now));
    let ready = watcher.poll(now);
    assert_eq!(phases(&ready), [("ready", 100)]);
    assert_eq!(ready[0].error, None);
}

#[test]
fn an_unchanged_file_is_announced_once_however_often_it_is_read() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(NAME, &building(30));
    assert_eq!(watcher.poll(now).len(), 1);

    for _ in 0..20 {
        assert!(watcher.poll(now).is_empty());
    }
}

#[test]
fn progress_is_announced_in_steps_of_ten_percent() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();
    let mut announced = Vec::new();

    for done in [0, 3, 9, 10, 14, 19, 20, 55, 56, 99] {
        data.write(NAME, &building(done));
        announced.extend(watcher.poll(now));
    }

    assert_eq!(
        phases(&announced),
        [
            ("building", 0),
            ("building", 10),
            ("building", 20),
            ("building", 55),
            ("building", 99)
        ]
    );
}

#[test]
fn a_build_that_stops_reporting_is_stale_and_resumes_as_building() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(NAME, &building(40));
    assert_eq!(phases(&watcher.poll(now)), [("building", 40)]);

    assert!(watcher.poll(now + STALE_AFTER - Duration::from_secs(1)).is_empty());

    let later = now + STALE_AFTER + Duration::from_secs(1);
    assert_eq!(phases(&watcher.poll(later)), [("stale", 40)]);
    assert!(watcher.poll(later).is_empty(), "stale is announced once");

    // The server wrote again: the file is fresh at the next look.
    assert_eq!(phases(&watcher.poll(now)), [("building", 40)]);
}

#[test]
fn a_cancelled_build_is_not_an_error_and_a_failed_one_carries_its_reason() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(NAME, &building(5));
    watcher.poll(now);

    data.write(NAME, &finished("error", Some("cancelled"), now));
    let cancelled = watcher.poll(now);
    assert_eq!(phases(&cancelled), [("cancelled", 100)]);
    assert_eq!(cancelled[0].error, None);

    data.write(NAME, &building(5));
    data.write(OTHER, &finished("error", Some("disk full"), now));
    let events = watcher.poll(now);
    assert_eq!(
        events.iter().map(|event| event.phase.as_str()).collect::<Vec<_>>(),
        ["building", "error"],
        "ordered by file name, and a build that starts again after being cancelled is announced"
    );
    assert_eq!(events[1].error.as_deref(), Some("disk full"));
}

#[test]
fn a_build_that_ended_before_the_app_looked_is_not_announced() {
    let data = Data::new();
    let now = SystemTime::now();

    data.write(NAME, &finished("ready", None, now - Duration::from_secs(120)));
    assert!(CodeBuildWatcher::new(&data.root).poll(now).is_empty());

    data.write(NAME, &finished("ready", None, now - Duration::from_secs(2)));
    assert_eq!(phases(&CodeBuildWatcher::new(&data.root).poll(now)), [("ready", 100)]);

    data.write(NAME, &finished("ready", None, now - Duration::from_secs(16 * 60)));
    assert!(CodeBuildWatcher::new(&data.root).poll(now).is_empty());
}

#[test]
fn a_page_that_starts_listening_gets_the_builds_in_progress() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(NAME, &building(42));
    data.write(OTHER, &finished("ready", None, now));
    watcher.poll(now);

    assert_eq!(phases(&watcher.active()), [("building", 42)]);
}

#[test]
fn files_the_app_must_not_trust_are_ignored() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    // Wrong names.
    for name in [
        "0123456789ABCDEF.json",
        "0123456789abcde.json",
        "0123456789abcdef0.json",
        "0123456789abcdef.txt",
    ] {
        data.write(name, &building(1));
    }

    // Garbage, an unknown version, kind and phase, and a file too large.
    data.write("1111111111111111.json", &json!("not a status"));
    std::fs::write(data.builds().join("2222222222222222.json"), "{ not json").unwrap();
    data.write("3333333333333333.json", &json!({ "version": 2, "buildId": 1 }));
    data.write("4444444444444444.json", &{
        let mut status = building(1);
        status["kind"] = json!("incremental");
        status
    });
    data.write("5555555555555555.json", &{
        let mut status = building(1);
        status["phase"] = json!("paused");
        status
    });
    data.write("6666666666666666.json", &{
        let mut status = building(1);
        status["repoName"] = json!("x".repeat(MAX_STATUS_BYTES as usize));
        status
    });

    assert!(watcher.poll(now).is_empty());
    assert!(watcher.active().is_empty());

    // A folder named like a status file is not read as one.
    std::fs::create_dir(data.builds().join(OTHER)).unwrap();
    assert!(watcher.poll(now).is_empty());
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_or_a_folder_others_may_change_is_not_read() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let data = Data::new();
    let now = SystemTime::now();
    let outside = data.root.join("outside.json");
    std::fs::write(&outside, building(1).to_string()).unwrap();
    symlink(&outside, data.builds().join(NAME)).unwrap();

    assert!(
        CodeBuildWatcher::new(&data.root).poll(now).is_empty(),
        "a link is not followed"
    );

    std::fs::remove_file(data.builds().join(NAME)).unwrap();
    data.write(NAME, &building(1));
    assert_eq!(CodeBuildWatcher::new(&data.root).poll(now).len(), 1);

    std::fs::set_permissions(data.builds(), std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(CodeBuildWatcher::new(&data.root).poll(now).is_empty());
}

#[test]
fn a_missing_folder_has_nothing_to_announce() {
    let dir = tempfile::tempdir().unwrap();

    assert!(CodeBuildWatcher::new(dir.path()).poll(SystemTime::now()).is_empty());
}

#[test]
fn a_total_that_becomes_known_in_the_same_step_is_announced() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    let mut uncounted = building(0);
    uncounted["filesTotal"] = json!(0);
    data.write(NAME, &uncounted);
    assert_eq!(watcher.poll(now).len(), 1);

    data.write(NAME, &building(0));
    let counted = watcher.poll(now);
    assert_eq!(
        counted.len(),
        1,
        "the total is news even when the 10 % step is the same"
    );
    assert_eq!(counted[0].files_total, 100);
}

#[test]
fn a_finish_time_too_far_out_to_represent_is_ignored_without_a_panic() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(
        NAME,
        &json!({
            "version": 1, "buildId": 1, "root": "/x", "repoName": "x", "kind": "full",
            "phase": "ready", "finishedAt": u64::MAX
        }),
    );

    assert!(watcher.poll(now).is_empty());
    assert!(watcher.active().is_empty());
}

#[test]
fn invisible_formatting_characters_and_new_lines_are_removed_from_what_the_server_wrote() {
    let data = Data::new();
    let mut watcher = CodeBuildWatcher::new(&data.root);
    let now = SystemTime::now();

    data.write(
        NAME,
        &json!({
            "version": 1, "buildId": 1, "root": "/wo\u{202E}rk/\u{200B}demo\u{FEFF}", "repoName": "de\u{2066}m\u{E0041}o\u{2028}\u{FE0F}\u{2069}\u{200F}",
            "kind": "full", "phase": "error", "filesTotal": 1, "filesDone": 0,
            "startedAt": 0, "finishedAt": seconds(now), "error": "first\nsecond\r\n\u{202E}third\u{200D}"
        }),
    );

    let events = watcher.poll(now);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].root, "/work/demo");
    assert_eq!(events[0].repo_name, "demo");
    assert_eq!(events[0].error.as_deref(), Some("first second third"));
}
