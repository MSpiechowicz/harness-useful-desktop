//! Session totals across every project (`harness_session_totals`): OMP and PI session files in
//! temporary homes; OpenCode has no session files to count.

use crate::session_support;

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, OpenCode, Pi};
use harness_business::model::{IpcErrorCode, SessionTotals};
use harness_business::pi_family::sessions::{SessionStore, session_totals};
use pretty_assertions::assert_eq;
use session_support::{OMP_RELEASE, PI_NAMED, SessionHome, session_home, set_mtime};

/// The OMP fixture files that count: release, main, its copy, the fork and the draft.
const OMP_FIXTURE_SESSIONS: u32 = 5;

/// The newest OMP fixture file (the draft).
const OMP_FIXTURE_NEWEST: &str = "2026-10-09T12:00:30Z";

fn totals(count: u32, last_used: Option<&str>) -> SessionTotals {
    SessionTotals {
        count,
        last_used: last_used.map(str::to_owned),
        capped: false,
    }
}

/// A second project next to the demo one.
fn other_project(home: &SessionHome) -> PathBuf {
    let other = home.home.join("Projects/other");
    fs::create_dir_all(&other).unwrap();
    other
}

/// Copies the session file of `id` from `from` into `to` as session `new_id`, written at `time`.
fn copy_session(from: &Path, id: &str, to: &Path, new_id: &str, time: &str) -> PathBuf {
    let source = fs::read_dir(from)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with(&format!("_{id}.jsonl")))
        .expect("fixture session");

    fs::create_dir_all(to).unwrap();
    let target = to.join(format!("2026-10-10T08-00-00-000Z_{new_id}.jsonl"));
    fs::copy(source, &target).unwrap();
    set_mtime(&target, time);
    target
}

#[test]
fn omp_counts_the_session_files_of_every_project_and_their_newest_time() {
    let home = SessionHome::with_fixtures(&[(&Omp, "omp")]);
    let demo = Omp.sessions_dir(&home.env, &home.project);
    let other = Omp.sessions_dir(&home.env, &other_project(&home));

    copy_session(
        &demo,
        OMP_RELEASE,
        &other,
        "0199eeee-0000-7000-8000-000000000010",
        "2026-10-10T08:00:00Z",
    );

    // Neither hidden, empty, misnamed files nor the files of companion folders count.
    fs::write(
        other.join(".2026-10-10T09-00-00-000Z_0199eeee-0000-7000-8000-000000000011.jsonl"),
        "{}\n",
    )
    .unwrap();
    fs::write(
        other.join("2026-10-10T09-00-00-000Z_0199eeee-0000-7000-8000-000000000012.jsonl"),
        "",
    )
    .unwrap();
    fs::write(other.join("readme.txt"), "notes\n").unwrap();
    let companion = other.join("2026-10-10T08-00-00-000Z_0199eeee-0000-7000-8000-000000000010");
    fs::create_dir_all(&companion).unwrap();
    fs::write(
        companion.join("subagent_0199eeee-0000-7000-8000-000000000013.jsonl"),
        "{}\n",
    )
    .unwrap();

    let found = Omp.session_totals(&home.env).unwrap();

    assert_eq!(found, totals(OMP_FIXTURE_SESSIONS + 1, Some("2026-10-10T08:00:00Z")));
}

#[cfg(unix)]
#[test]
fn symlinks_into_another_folder_are_not_followed() {
    let home = SessionHome::with_fixtures(&[(&Omp, "omp")]);
    let demo = Omp.sessions_dir(&home.env, &home.project);
    let root = demo.parent().unwrap().to_path_buf();

    // A folder outside the store with a newer session, linked in as a project folder and as a file.
    let outside = home.home.join("outside");
    let linked = copy_session(
        &demo,
        OMP_RELEASE,
        &outside,
        "0199eeee-0000-7000-8000-000000000020",
        "2026-10-11T08:00:00Z",
    );
    std::os::unix::fs::symlink(&outside, root.join("-linked-project-")).unwrap();
    std::os::unix::fs::symlink(
        &linked,
        demo.join("2026-10-11T08-00-00-000Z_0199eeee-0000-7000-8000-000000000021.jsonl"),
    )
    .unwrap();

    let found = Omp.session_totals(&home.env).unwrap();

    assert_eq!(found, totals(OMP_FIXTURE_SESSIONS, Some(OMP_FIXTURE_NEWEST)));
}

#[test]
fn pi_counts_the_session_files_of_every_project() {
    let home = SessionHome::with_fixtures(&[(&Pi, "pi")]);
    let demo = Pi.sessions_dir(&home.env, &home.project);
    let other = Pi.sessions_dir(&home.env, &other_project(&home));

    copy_session(
        &demo,
        PI_NAMED,
        &other,
        "9d3e5f70-0000-4000-8000-000000000001",
        "2026-10-12T07:00:00Z",
    );

    let found = Pi.session_totals(&home.env).unwrap();

    assert_eq!(found, totals(3, Some("2026-10-12T07:00:00Z")));
}

#[test]
fn a_configured_session_folder_holds_every_project_flat() {
    let home = SessionHome::with_fixtures(&[(&Pi, "pi")]);
    let demo = Pi.sessions_dir(&home.env, &home.project);
    let shared = home.home.join("chats");
    copy_session(
        &demo,
        PI_NAMED,
        &shared,
        "9d3e5f70-0000-4000-8000-000000000002",
        "2026-10-12T07:00:00Z",
    );

    // The user's `sessionDir` replaces the per-project folders.
    let agent_dir = Pi.paths(&home.env, None).agent_dir;
    fs::create_dir_all(&agent_dir).unwrap();
    fs::write(agent_dir.join("settings.json"), r#"{ "sessionDir": "~/chats" }"#).unwrap();

    let found = Pi.session_totals(&home.env).unwrap();
    assert_eq!(found, totals(1, Some("2026-10-12T07:00:00Z")));

    // So does the environment, for OMP as well.
    let env = home
        .env
        .clone()
        .with_var("PI_CODING_AGENT_SESSION_DIR", shared.to_string_lossy());
    assert_eq!(Omp.session_totals(&env).unwrap(), found);
    assert_eq!(Pi.session_totals(&env).unwrap(), found);
}

#[test]
fn a_relative_session_folder_has_no_machine_wide_totals() {
    let home = session_home();
    let env = home.env.clone().with_var("PI_CODING_AGENT_SESSION_DIR", "chats");

    for harness in [&Omp as &dyn Harness, &Pi] {
        let error = harness.session_totals(&env).expect_err("relative folder");
        assert_eq!(error.code(), IpcErrorCode::Unsupported);
    }
}

#[test]
fn an_empty_store_has_no_sessions_and_no_last_use() {
    let home = session_home();

    assert_eq!(Omp.session_totals(&home.env).unwrap(), totals(0, None));
    assert_eq!(Pi.session_totals(&home.env).unwrap(), totals(0, None));

    // An existing folder without session files is the same.
    fs::create_dir_all(Omp.sessions_dir(&home.env, &home.project)).unwrap();
    assert_eq!(Omp.session_totals(&home.env).unwrap(), totals(0, None));
}

#[test]
fn the_walk_stops_at_its_entry_budget() {
    let home = session_home();
    let shared = home.home.join("chats");
    fs::create_dir_all(&shared).unwrap();

    for index in 0..4 {
        let file = shared.join(format!(
            "2026-10-10T08-00-0{index}-000Z_0199eeee-0000-7000-8000-00000000003{index}.jsonl"
        ));
        fs::write(&file, "{}\n").unwrap();
    }

    let store = SessionStore::Shared(shared);

    let exact = session_totals(&store, 4);
    assert_eq!((exact.count, exact.capped), (4, false));

    let cut = session_totals(&store, 3);
    assert_eq!((cut.count, cut.capped), (3, true));

    // Project folders count against the same budget.
    let nested = session_totals(&SessionStore::PerProject(home.home.clone()), 1);
    assert!(nested.capped);
    assert_eq!(nested.count, 0);
}

#[test]
fn opencode_has_no_session_files_to_count() {
    let home = session_home();

    let error = OpenCode::default().session_totals(&home.env).expect_err("CLI sessions");
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
}
