//! OpenCode sessions through its CLI, replayed (see `opencode_session_support`): listing,
//! session id checks, spawn settings, deleting and CLI failures (concurrent calls are in
//! `sessions_opencode_concurrency.rs`).

mod common;
mod opencode_session_support;
mod session_support;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::time::Duration;

use common::CANARY;
use harness_business::harness::{CliSessions, Harness};
use harness_business::harnesses::OpenCode;
use harness_business::harnesses::opencode::OpenCodeSessions;
use harness_business::model::{HarnessId, IpcErrorCode};
use harness_business::process::ExecOutput;
use opencode_session_support::{Fixture, LIST_ARGS, MAIN, Replay, SUB, argv, error_code, failed, ok, opencode};
use pretty_assertions::assert_eq;
use serde_json::json;
use session_support::session;

#[test]
fn the_project_sessions_are_listed_newest_first() {
    let fixture = Fixture::new();
    let runner = Replay::answering([fixture.list_output()]);
    let sessions = OpenCodeSessions::new();

    let listed = sessions.list(&fixture.ctx(fixture.project(), &runner)).unwrap();

    let summary = |index: usize| {
        let item = &listed[index];
        (
            item.session.id.as_str(),
            item.title.as_deref(),
            item.created_at.as_str(),
            item.updated_at.as_str(),
        )
    };
    assert_eq!(listed.len(), 2);
    assert_eq!(
        summary(0),
        (
            MAIN,
            Some("Fix the greeting"),
            "2025-10-16T07:33:20Z",
            "2025-10-16T07:34:50Z"
        )
    );
    assert_eq!(
        summary(1),
        (
            SUB,
            Some("Explain the build"),
            "2025-10-15T03:46:40Z",
            "2025-10-15T03:47:30Z"
        )
    );
    assert!(listed.iter().all(|item| item.session.harness == HarnessId::OpenCode));

    let spec = &runner.specs()[0];
    assert_eq!(spec.program, fixture.binary.path);
    assert_eq!(argv(spec), LIST_ARGS);
    assert_eq!(spec.cwd.as_deref(), Some(fixture.project()));
    assert_eq!(spec.timeout, Duration::from_secs(15));
    assert_eq!(spec.max_output_bytes, 4 * 1024 * 1024);
}

#[test]
fn a_subdirectory_project_keeps_only_its_own_sessions() {
    let fixture = Fixture::new();
    let sub = fixture.project().join("sub");
    let runner = Replay::answering([fixture.list_output()]);

    // OpenCode lists the whole worktree's sessions in `sub` too; the parent's session is dropped.
    let listed = OpenCodeSessions::new().list(&fixture.ctx(&sub, &runner)).unwrap();

    let ids: Vec<&str> = listed.iter().map(|item| item.session.id.as_str()).collect();
    assert_eq!(ids, [SUB]);
}

#[test]
fn sessions_of_other_directories_never_leak_into_the_list() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let sibling = project.with_file_name("demo-other");
    fs::create_dir_all(&sibling).unwrap();

    let entry = |id: &str, directory: String| json!({ "id": id, "title": id, "updated": 1, "created": 1, "directory": directory });
    let list = json!([
        entry("ses_mine", project.join("src").to_string_lossy().into_owned()),
        entry("ses_sibling", sibling.to_string_lossy().into_owned()),
        entry("ses_climbs", format!("{}/../demo-other", project.to_string_lossy())),
        entry(
            "ses_climbs_missing",
            format!("{}/gone/../../elsewhere", project.to_string_lossy())
        ),
        entry("ses_parent", fixture.home.home.to_string_lossy().into_owned()),
        entry("ses_relative", "demo".to_owned()),
    ]);
    let runner = Replay::answering([ok(list.to_string())]);

    let listed = OpenCodeSessions::new().list(&fixture.ctx(project, &runner)).unwrap();

    let ids: Vec<&str> = listed.iter().map(|item| item.session.id.as_str()).collect();
    assert_eq!(ids, ["ses_mine"]);
}

#[test]
fn empty_output_lists_nothing() {
    let fixture = Fixture::new();

    for stdout in ["", "\n", "  \r\n"] {
        let runner = Replay::answering([ok(stdout)]);
        let listed = OpenCodeSessions::new()
            .list(&fixture.ctx(fixture.project(), &runner))
            .unwrap();

        assert!(listed.is_empty(), "{stdout:?}");
    }
}

#[test]
fn malformed_entries_are_skipped_and_garbage_is_an_error() {
    let fixture = Fixture::new();
    let directory = fixture.project().to_string_lossy().into_owned();
    let list = json!([
        { "id": "ses_good", "title": " ", "updated": 5, "directory": directory },
        { "title": "no id", "directory": directory },
        { "id": 42, "directory": directory },
        { "id": "../x", "directory": directory },
        { "id": "not-a-session", "directory": directory },
        { "id": "ses_nodirectory" },
        { "id": "ses_badtime", "updated": "soon", "created": -3, "directory": directory },
        "a string",
        null
    ]);
    let runner = Replay::answering([ok(list.to_string())]);

    let listed = OpenCodeSessions::new()
        .list(&fixture.ctx(fixture.project(), &runner))
        .unwrap();

    let ids: Vec<&str> = listed.iter().map(|item| item.session.id.as_str()).collect();
    assert_eq!(ids, ["ses_good", "ses_badtime"]);
    assert_eq!(listed[0].title, None, "a blank title is no title");
    assert_eq!(listed[1].created_at, "1970-01-01T00:00:00Z");

    for garbage in ["not json", "{\"id\": \"ses_x\"}"] {
        let runner = Replay::answering([ok(garbage)]);
        let result = OpenCodeSessions::new().list(&fixture.ctx(fixture.project(), &runner));

        assert_eq!(error_code(result), IpcErrorCode::Protocol, "{garbage}");
    }
}

#[test]
fn hostile_session_ids_are_refused_before_anything_runs() {
    let fixture = Fixture::new();
    let runner = Replay::default();
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    let hostile = [
        "../x",
        "-rf",
        "--pure",
        "ses_x;y",
        "ses_a b",
        "ses_../x",
        "ses_x/../y",
        "abc",
        "",
        ".ses_x",
        "ses_x\n",
    ];

    for id in hostile {
        let session = opencode(id);

        assert_eq!(
            error_code(sessions.messages(&ctx, &session, None, 10)),
            IpcErrorCode::InvalidInput,
            "{id:?}"
        );
        assert_eq!(
            error_code(sessions.files_touched(&ctx, &session)),
            IpcErrorCode::InvalidInput,
            "{id:?}"
        );
        assert_eq!(
            error_code(sessions.delete(&ctx, &session)),
            IpcErrorCode::InvalidInput,
            "{id:?}"
        );
    }

    let foreign = session(HarnessId::Omp, MAIN);
    assert_eq!(
        error_code(sessions.messages(&ctx, &foreign, None, 10)),
        IpcErrorCode::InvalidInput
    );
    assert_eq!(error_code(sessions.delete(&ctx, &foreign)), IpcErrorCode::InvalidInput);

    assert!(runner.specs().is_empty(), "spawned: {:?}", runner.argv());
}

#[test]
fn every_spawn_runs_without_plugins_project_config_or_self_update() {
    let fixture = Fixture::new();
    let runner = Replay::answering([fixture.list_output(), fixture.export_output(), ok("Session deleted")]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    sessions.list(&ctx).unwrap();
    sessions.messages(&ctx, &opencode(MAIN), None, 10).unwrap();
    sessions.files_touched(&ctx, &opencode(MAIN)).unwrap();
    sessions.delete(&ctx, &opencode(MAIN)).unwrap();

    let specs = runner.specs();
    assert_eq!(specs.len(), 3, "{:?}", runner.argv());

    let update_off = (OsString::from("OPENCODE_DISABLE_AUTOUPDATE"), OsString::from("1"));
    // Starting up with the project config writes `.opencode/.gitignore` and installs packages
    // into the project.
    let project_config_off = (OsString::from("OPENCODE_DISABLE_PROJECT_CONFIG"), OsString::from("1"));

    for spec in &specs {
        assert!(spec.env.contains(&update_off), "{:?}", argv(spec));
        assert!(spec.env.contains(&project_config_off), "{:?}", argv(spec));
        assert!(argv(spec).iter().any(|arg| arg == "--pure"), "{:?}", argv(spec));
        assert_eq!(spec.program, fixture.binary.path);
        assert_eq!(spec.cwd.as_deref(), Some(fixture.project()));

        let binary_dir = fixture.binary.path.parent().unwrap();
        assert!(spec.search_dirs.iter().any(|dir| dir == binary_dir));
    }

    // The allow-list: every argv, the session id as a placeholder. Nothing else ever runs.
    let used: BTreeSet<String> = runner
        .argv()
        .iter()
        .map(|argv| argv.join(" ").replace(MAIN, "<id>"))
        .collect();
    let allowed = BTreeSet::from([
        "session list --pure --format json".to_owned(),
        "export <id> --pure".to_owned(),
        "session delete --pure <id>".to_owned(),
    ]);
    assert_eq!(used, allowed);
}

#[test]
fn deleting_runs_the_cli_and_drops_the_cached_list() {
    let fixture = Fixture::new();
    let runner = Replay::answering([
        fixture.list_output(),
        ok("Session deleted"),
        ok(fixture
            .recorded("session-list.json", fixture.project())
            .replace(MAIN, "ses_gone_from_fixture")),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    sessions.list(&ctx).unwrap();
    sessions.list(&ctx).unwrap();
    assert_eq!(runner.specs().len(), 1, "the second list is cached");

    sessions.delete(&ctx, &opencode(MAIN)).unwrap();
    let listed = sessions.list(&ctx).unwrap();

    assert!(listed.iter().all(|item| item.session.id != MAIN));
    assert_eq!(
        runner.argv(),
        [
            LIST_ARGS.map(str::to_owned).to_vec(),
            vec![
                "session".to_owned(),
                "delete".to_owned(),
                "--pure".to_owned(),
                MAIN.to_owned()
            ],
            LIST_ARGS.map(str::to_owned).to_vec(),
        ]
    );
}

#[test]
fn only_a_listed_session_is_deleted() {
    let fixture = Fixture::new();
    let sub = fixture.project().join("sub");
    let runner = Replay::answering([fixture.list_output(), fixture.list_output()]);

    // MAIN belongs to the parent directory: `session delete` would find it, the list does not
    // (neither the cached one nor a new one).
    let refused = OpenCodeSessions::new().delete(&fixture.ctx(&sub, &runner), &opencode(MAIN));

    assert_eq!(error_code(refused), IpcErrorCode::NotFound);
    assert_eq!(
        runner.argv(),
        [
            LIST_ARGS.map(str::to_owned).to_vec(),
            LIST_ARGS.map(str::to_owned).to_vec()
        ]
    );
}

#[test]
fn a_session_newer_than_the_cached_list_is_deleted() {
    let fixture = Fixture::new();
    let without_main = fixture
        .recorded("session-list.json", fixture.project())
        .replace(MAIN, "ses_older_than_main");
    let runner = Replay::answering([ok(without_main), fixture.list_output(), ok("Session deleted")]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    // The list is cached before MAIN existed.
    sessions.list(&ctx).unwrap();
    sessions.delete(&ctx, &opencode(MAIN)).unwrap();

    let commands: Vec<String> = runner.argv().iter().map(|argv| argv[..2].join(" ")).collect();
    assert_eq!(commands, ["session list", "session list", "session delete"]);
}

#[test]
fn cli_failures_are_short_and_redacted() {
    let fixture = Fixture::new();
    let recorded = fixture.recorded("export-not-found.stderr", fixture.project());
    let secret = format!("API_KEY={CANARY}-0123456789abcdef");
    let overflowing = |stderr: &str| ExecOutput {
        truncated: true,
        ..failed(stderr)
    };
    let runner = Replay::answering([
        failed(&recorded),
        failed(&format!("Error: provider rejected key {secret}\n")),
        failed(""),
        overflowing(&recorded),
        overflowing("Error: database is locked\n"),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    let missing = sessions.messages(&ctx, &opencode(MAIN), None, 10);
    assert_eq!(error_code(missing), IpcErrorCode::NotFound);

    let failure = sessions.messages(&ctx, &opencode(MAIN), None, 10).expect_err("failed");
    assert_eq!(failure.code(), IpcErrorCode::Process);
    assert!(!failure.to_string().contains(CANARY), "{failure}");
    assert!(failure.to_string().contains("provider rejected key"), "{failure}");

    let silent = sessions.list(&ctx).expect_err("failed");
    assert_eq!(silent.code(), IpcErrorCode::Process);

    // A failure says why even when its output overflowed the cap.
    let missing = sessions.messages(&ctx, &opencode(MAIN), None, 10);
    assert_eq!(error_code(missing), IpcErrorCode::NotFound);

    let locked = sessions.list(&ctx).expect_err("failed");
    assert_eq!(locked.code(), IpcErrorCode::Process);
    assert!(locked.to_string().contains("database is locked"), "{locked}");
}

#[test]
fn oversized_or_slow_output_is_an_error() {
    let fixture = Fixture::new();
    let runner = Replay::answering([
        ExecOutput {
            truncated: true,
            ..fixture.export_output()
        },
        ExecOutput {
            timed_out: true,
            ..ExecOutput::default()
        },
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    let large = sessions
        .messages(&ctx, &opencode(MAIN), None, 10)
        .expect_err("too large");
    assert_eq!(large.code(), IpcErrorCode::InvalidInput);
    assert!(large.to_string().contains("too large"), "{large}");

    assert_eq!(error_code(sessions.list(&ctx)), IpcErrorCode::Timeout);
}

#[test]
fn the_harness_exposes_the_cli_sessions() {
    let fixture = Fixture::new();
    let runner = Replay::answering([fixture.list_output()]);
    let harness = OpenCode::default();
    let cli = harness.cli_sessions().expect("OpenCode sessions go through its CLI");

    let listed = cli.list(&fixture.ctx(fixture.project(), &runner)).unwrap();

    assert_eq!(listed.len(), 2);
    assert_eq!(
        OpenCode::default()
            .list_sessions(&fixture.home.env, fixture.project(), &Default::default())
            .map(drop)
            .map_err(|error| error.code()),
        Err(IpcErrorCode::Unsupported)
    );
}
