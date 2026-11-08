//! Copying what waits for the team database (`harness-memory sync --json`): when the app looks, what
//! it counts as work, when it runs the server's command, and that two runs never overlap. Run
//! against the stand-in server of `fake_agent` (`src/bin/fake_agent/memory_sync.rs`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use harness_business::ecosystem::memory_requests::memory_data_dir;
use harness_business::ecosystem::memory_sync::{
    MAX_WAITING_BYTES, PENDING_SYNC_FILE, PENDING_UPDATES_FILE, SYNC_INTERVAL, SYNC_TIMEOUT, Waiting, waiting_entries,
};
use harness_business::ecosystem::{MemoryFeatures, MemorySync, SyncOutcome};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Notify;

use crate::memory_stand_in;

const OFFLINE_SAVES: &str = r#"{"notes": [{"team": "t", "id": 4, "scope": "global", "name": "A"}],
    "scripts": [{"team": "t", "id": 2, "scope": "global", "name": "b"}], "someday": true}"#;
const QUEUED_UPDATES: &str = r#"{"updates": [{"seq": 1, "id": 9}, {"seq": 2, "id": 9}]}"#;

/// The data folder of the setup's home, created (Windows has no data folder until `LOCALAPPDATA`
/// says where).
fn data_dir(setup: &mut Setup) -> PathBuf {
    if memory_data_dir(&setup.env).is_none() {
        let local = setup.env.home.join("AppData").join("Local");
        setup.env = setup.env.clone().with_var("LOCALAPPDATA", local.to_string_lossy());
    }

    let dir = memory_data_dir(&setup.env).expect("a data folder");
    std::fs::create_dir_all(&dir).unwrap();

    dir
}

/// A setup whose server is installed, with `vars`, and its data folder.
fn waiting(vars: Vars) -> (Setup, PathBuf) {
    let mut setup = Setup::installed(CURRENT, vars);
    let data = data_dir(&mut setup);

    (setup, data)
}

/// One turn, run against the setup's server; its outcome.
async fn turn(setup: &Setup, sync: &Arc<MemorySync>) -> SyncOutcome {
    let turn = sync.begin(Instant::now(), true).expect("a turn is due");

    turn.run(&setup.env, setup.runner(), &MemoryFeatures::new()).await
}

/// The commands run that were `sync`.
fn syncs(setup: &Setup) -> usize {
    (0..setup.runs().len())
        .filter(|run| setup.run_args(*run).first().is_some_and(|arg| arg == "sync"))
        .count()
}

#[tokio::test]
async fn nothing_runs_while_nothing_waits_or_there_is_no_server_or_data_folder() {
    let sync = Arc::new(MemorySync::new());

    let (setup, data) = waiting(&[]);
    assert_eq!(turn(&setup, &sync).await, SyncOutcome::Idle, "no file");

    for empty in ["", "  \n", r#"{"notes": [], "scripts": []}"#, "{}"] {
        std::fs::write(data.join(PENDING_SYNC_FILE), empty).unwrap();
        std::fs::write(data.join(PENDING_UPDATES_FILE), r#"{"updates": []}"#).unwrap();

        assert_eq!(
            turn(&setup, &Arc::new(MemorySync::new())).await,
            SyncOutcome::Idle,
            "{empty:?}"
        );
    }

    assert_eq!(syncs(&setup), 0);

    let mut absent = Setup::installed(CURRENT, &[]);
    let data = data_dir(&mut absent);
    std::fs::remove_dir(&data).unwrap();
    assert_eq!(turn(&absent, &Arc::new(MemorySync::new())).await, SyncOutcome::Idle);
    assert!(
        absent.runs().is_empty(),
        "the data folder is never created or the server asked"
    );

    let mut missing = Setup::without_server(CURRENT, &[]);
    let data = data_dir(&mut missing);
    std::fs::write(data.join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();
    assert_eq!(turn(&missing, &Arc::new(MemorySync::new())).await, SyncOutcome::Idle);
    assert!(missing.runs().is_empty());
}

#[tokio::test]
async fn waiting_notes_scripts_or_updates_run_the_sync_command_once() {
    for (file, text) in [
        (PENDING_SYNC_FILE, OFFLINE_SAVES),
        (PENDING_UPDATES_FILE, QUEUED_UPDATES),
    ] {
        let (setup, data) = waiting(&[]);
        std::fs::write(data.join(file), text).unwrap();

        let outcome = turn(&setup, &Arc::new(MemorySync::new())).await;

        let SyncOutcome::Synced { before, report } = outcome else {
            panic!("{file}: {outcome:?}");
        };
        assert_eq!(before, 2, "{file}");
        assert!(report.online && report.waiting() == 0, "{file}: {report:?}");

        assert_eq!(syncs(&setup), 1, "{file}");
        let run = setup.runs().into_iter().find(|run| run.args[0] == "sync").unwrap();
        assert_eq!(run.args, ["sync", "--json"]);
        assert_eq!(run.program, setup.binary);
        assert_eq!(run.timeout, SYNC_TIMEOUT);
        assert_eq!(run.cwd.as_deref(), Some(setup.env.home.as_path()));
    }
}

#[tokio::test]
async fn what_still_waits_is_read_from_the_answer() {
    let (setup, data) = waiting(&[("FAKE_MEMORY_SYNC", "offline")]);
    std::fs::write(data.join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();

    let SyncOutcome::Synced { before, report } = turn(&setup, &Arc::new(MemorySync::new())).await else {
        panic!("it ran");
    };

    assert_eq!(before, 2);
    assert!(!report.online);
    assert_eq!((report.notes, report.scripts, report.updates), (2, 1, 3));
    assert_eq!(report.waiting(), 6);
}

#[tokio::test]
async fn a_server_without_the_sync_command_is_not_run() {
    let (setup, data) = waiting(&[("FAKE_MEMORY_FEATURES", "notes-hide,offline-update-queue")]);
    std::fs::write(data.join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();

    assert_eq!(
        turn(&setup, &Arc::new(MemorySync::new())).await,
        SyncOutcome::Unsupported
    );
    assert_eq!(syncs(&setup), 0);

    let (older, data) = waiting(&[("FAKE_MEMORY_FEATURES", "none")]);
    std::fs::write(data.join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();

    assert_eq!(
        turn(&older, &Arc::new(MemorySync::new())).await,
        SyncOutcome::Unsupported
    );
    assert_eq!(syncs(&older), 0);
}

#[tokio::test]
async fn a_failed_run_is_reported_and_tried_again_by_the_next_turn() {
    let failing: Vars = &[("FAKE_MEMORY_SYNC", "failing")];
    let garbled: Vars = &[("FAKE_MEMORY_SYNC", "garbled")];

    for (broken, vars) in [("failing", failing), ("garbled", garbled)] {
        let (setup, data) = waiting(vars);
        std::fs::write(data.join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();
        let sync = Arc::new(MemorySync::new());

        assert_eq!(turn(&setup, &sync).await, SyncOutcome::Failed, "{broken}");
        assert!(sync.begin(Instant::now(), true).is_none(), "{broken}: not at once");

        let later = Instant::now() + SYNC_INTERVAL + Duration::from_secs(1);
        let again = sync.begin(later, true).expect("due again");
        assert_eq!(
            again.run(&setup.env, setup.runner(), &MemoryFeatures::new()).await,
            SyncOutcome::Failed
        );
        assert_eq!(syncs(&setup), 2, "{broken}");
    }
}

#[test]
fn a_turn_comes_once_per_interval_and_never_while_another_runs() {
    let sync = Arc::new(MemorySync::new());
    let now = Instant::now();

    let first = sync.begin(now, true).expect("the first one is due at once");
    assert!(
        sync.begin(now + SYNC_INTERVAL * 10, true).is_none(),
        "one is still running"
    );

    drop(first);
    assert!(
        sync.begin(Instant::now() + SYNC_INTERVAL / 2, true).is_none(),
        "too soon"
    );

    let next = sync.begin(Instant::now() + SYNC_INTERVAL + Duration::from_secs(1), true);
    assert!(next.is_some(), "an interval after the last one ended");
}

#[tokio::test]
async fn nothing_syncs_while_project_memory_is_off_and_it_resumes_when_on() {
    let (setup, data) = waiting(&[]);
    std::fs::write(data.join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();
    let sync = Arc::new(MemorySync::new());

    for _ in 0..3 {
        assert!(
            sync.begin(Instant::now(), false).is_none(),
            "no turn while the switch is off"
        );
    }

    assert!(setup.runs().is_empty(), "no server ran: {:?}", setup.runs());

    let outcome = turn(&setup, &sync).await;
    assert!(matches!(outcome, SyncOutcome::Synced { before: 2, .. }), "{outcome:?}");
    assert_eq!(syncs(&setup), 1, "it resumed at once, with no interval to wait out");
}

/// A runner that holds `sync` until it is released, and answers everything else as `inner` does.
struct Gated<'a> {
    inner: &'a dyn ProcessRunner,
    started: Notify,
    release: Notify,
}

impl ProcessRunner for Gated<'_> {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        Box::pin(async move {
            if spec.args.first().is_some_and(|arg| arg == "sync") {
                self.started.notify_one();
                self.release.notified().await;
            }

            self.inner.run(spec).await
        })
    }
}

#[tokio::test]
async fn a_turn_does_not_begin_while_the_sync_command_is_still_running() {
    let (setup, data) = waiting(&[]);
    std::fs::write(data.join(PENDING_UPDATES_FILE), QUEUED_UPDATES).unwrap();

    let gated = Gated {
        inner: setup.runner(),
        started: Notify::new(),
        release: Notify::new(),
    };
    let sync = Arc::new(MemorySync::new());
    let running = sync.begin(Instant::now(), true).expect("due");
    let features = MemoryFeatures::new();

    let (outcome, ()) = tokio::join!(running.run(&setup.env, &gated, &features), async {
        gated.started.notified().await;

        let much_later = Instant::now() + SYNC_INTERVAL * 10;
        assert!(sync.begin(much_later, true).is_none(), "the command is still running");

        gated.release.notify_one();
    });

    assert!(matches!(outcome, SyncOutcome::Synced { .. }), "{outcome:?}");
    assert_eq!(syncs(&setup), 1);
    assert!(
        sync.begin(Instant::now() + SYNC_INTERVAL + Duration::from_secs(1), true)
            .is_some(),
        "the next one comes once it ended"
    );
}

#[test]
fn entries_of_both_files_are_counted_and_unknown_fields_are_tolerated() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(waiting_entries(dir.path()), Waiting::default(), "no files");

    std::fs::write(dir.path().join(PENDING_SYNC_FILE), OFFLINE_SAVES).unwrap();
    assert_eq!(
        waiting_entries(dir.path()),
        Waiting {
            entries: 2,
            unreadable: 0
        }
    );

    std::fs::write(dir.path().join(PENDING_UPDATES_FILE), QUEUED_UPDATES).unwrap();
    assert_eq!(
        waiting_entries(dir.path()),
        Waiting {
            entries: 4,
            unreadable: 0
        }
    );

    std::fs::write(dir.path().join(PENDING_SYNC_FILE), json!({ "notes": [1] }).to_string()).unwrap();
    assert_eq!(
        waiting_entries(dir.path()).entries,
        3,
        "a list the file leaves out is empty"
    );
}

#[test]
fn a_file_that_cannot_be_read_waits_for_nothing_and_is_counted_as_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let saves = dir.path().join(PENDING_SYNC_FILE);
    let updates = dir.path().join(PENDING_UPDATES_FILE);

    let unreadable = |expected: usize| {
        assert_eq!(
            waiting_entries(dir.path()),
            Waiting {
                entries: 0,
                unreadable: expected
            }
        );
    };

    std::fs::write(&saves, r#"{"notes": [{"id": 1}"#).unwrap();
    unreadable(1);

    std::fs::write(&saves, "[1, 2, 3]").unwrap();
    unreadable(1);

    std::fs::write(&updates, r#"{"updates": "none"}"#).unwrap();
    unreadable(2);

    std::fs::remove_file(&updates).unwrap();
    std::fs::remove_file(&saves).unwrap();

    let oversized = format!(
        r#"{{"notes": [{{"name": "{}"}}]}}"#,
        "x".repeat(usize::try_from(MAX_WAITING_BYTES).unwrap())
    );
    std::fs::write(&saves, oversized).unwrap();
    unreadable(1);

    std::fs::remove_file(&saves).unwrap();
    std::fs::create_dir(&saves).unwrap();
    unreadable(1);
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_never_followed() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let target: &Path = &elsewhere.path().join("saves.json");
    std::fs::write(target, OFFLINE_SAVES).unwrap();

    std::os::unix::fs::symlink(target, dir.path().join(PENDING_SYNC_FILE)).unwrap();

    assert_eq!(
        waiting_entries(dir.path()),
        Waiting {
            entries: 0,
            unreadable: 1
        }
    );
}
