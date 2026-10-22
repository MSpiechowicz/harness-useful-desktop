//! OpenCode 2 sessions through its CLI, replayed (see `opencode_session_support`): listing, the
//! commands and spawn settings of each generation, deleting and missing sessions. The fixtures in
//! `tests/fixtures/sessions/opencode2` are real `opencode session list --standalone --format json`
//! and `opencode session export --standalone` output of OpenCode 2.1.2, with the project directory
//! replaced by `{{PROJECT}}`.

use crate::opencode_session_support;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::Duration;

use harness_business::harness::{CliSessions, Harness, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::harnesses::opencode::OpenCodeSessions;
use harness_business::model::{HarnessId, IpcErrorCode};
use harness_business::process::ExecOutput;
use opencode_session_support::{Fixture, Replay, argv, error_code, failed, ok, opencode};
use pretty_assertions::assert_eq;
use serde_json::json;

/// The session with an edit, a shell call and reads.
const EDIT: &str = "ses_ea283f985ffeX5BRVy9GzbD4gY";

/// The newest session of the recorded list.
const NEWEST: &str = "ses_ea27cfa99ffe21eKppwXzXASSa";

const LIST_ARGS: [&str; 7] = ["session", "list", "--standalone", "--format", "json", "-n", "5000"];

fn v2() -> OpenCodeSessions {
    OpenCodeSessions::with_generation(OpenCodeGeneration::V2)
}

/// A recorded OpenCode 2 fixture with `{{PROJECT}}` replaced by `project`.
fn recorded(name: &str, project: &Path) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sessions/opencode2")
        .join(name);
    let text = fs::read_to_string(path).expect("fixture");

    let quoted = serde_json::to_string(&project.to_string_lossy()).unwrap();
    text.replace("{{PROJECT}}", quoted.trim_matches('"'))
}

fn list_output(fixture: &Fixture) -> ExecOutput {
    ok(recorded("session-list.json", fixture.project()))
}

fn export_output(fixture: &Fixture) -> ExecOutput {
    ok(recorded("export-edit.json", fixture.project()))
}

#[test]
fn the_project_sessions_are_listed_newest_first() {
    let fixture = Fixture::new();
    let runner = Replay::answering([list_output(&fixture)]);

    let listed = v2().list(&fixture.ctx(fixture.project(), &runner)).unwrap();

    assert_eq!(listed.len(), 13);
    assert!(listed.iter().all(|item| item.session.harness == HarnessId::OpenCode));

    let newest = &listed[0];
    assert_eq!(newest.session.id, NEWEST);
    assert_eq!(newest.title.as_deref(), Some("Original 300-Word Story"));
    assert_eq!(newest.cwd, fixture.project().to_string_lossy());

    let edit = listed.iter().find(|item| item.session.id == EDIT).expect("listed");
    assert_eq!(
        edit.title.as_deref(),
        Some("Append recorded to notes.txt and list files")
    );
    assert_eq!(edit.created_at, "2026-10-21T08:02:51Z");
    assert_eq!(edit.updated_at, "2026-10-21T08:03:18Z");

    let updated: Vec<&str> = listed.iter().map(|item| item.updated_at.as_str()).collect();
    let mut sorted = updated.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(updated, sorted);

    let spec = &runner.specs()[0];
    assert_eq!(spec.program, fixture.binary.path);
    assert_eq!(argv(spec), LIST_ARGS);
    assert_eq!(spec.cwd.as_deref(), Some(fixture.project()));
    assert_eq!(spec.timeout, Duration::from_secs(15));
    assert_eq!(spec.max_output_bytes, 4 * 1024 * 1024);
}

#[test]
fn an_empty_list_an_untitled_session_and_other_directories() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let elsewhere = fixture.home.home.join("Projects/elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();

    let entry = |id: &str, directory: &Path| json!({ "id": id, "updated": 1_792_570_093_507_i64, "created": 1_792_570_089_016_i64, "projectId": "global", "directory": directory });
    let list = json!([
        entry("ses_untitled", &project.join("src")),
        entry("ses_elsewhere", &elsewhere)
    ]);
    let runner = Replay::answering([ok("[]\n"), ok(list.to_string())]);
    let ctx = fixture.ctx(project, &runner);

    assert!(v2().list(&ctx).unwrap().is_empty());

    let listed = v2().list(&ctx).unwrap();
    let ids: Vec<&str> = listed.iter().map(|item| item.session.id.as_str()).collect();
    assert_eq!(ids, ["ses_untitled"]);
    assert_eq!(listed[0].title, None);
}

#[test]
fn a_subdirectory_project_keeps_only_its_own_sessions() {
    let fixture = Fixture::new();
    let runner = Replay::answering([list_output(&fixture)]);

    // Every recorded session ran in the project root, which `sub` does not contain.
    let listed = v2()
        .list(&fixture.ctx(&fixture.project().join("sub"), &runner))
        .unwrap();

    assert!(listed.is_empty(), "{listed:?}");
}

#[test]
fn every_opencode2_spawn_is_standalone_without_plugins_flags_or_self_update() {
    let fixture = Fixture::new();
    let runner = Replay::answering([
        list_output(&fixture),
        export_output(&fixture),
        ok(format!("Session {EDIT} deleted\n")),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = v2();

    sessions.list(&ctx).unwrap();
    sessions.messages(&ctx, &opencode(EDIT), None, 10).unwrap();
    sessions.files_touched(&ctx, &opencode(EDIT)).unwrap();
    sessions.delete(&ctx, &opencode(EDIT)).unwrap();

    let specs = runner.specs();
    assert_eq!(specs.len(), 3, "{:?}", runner.argv());

    let env = |name: &str| (OsString::from(name), OsString::from("1"));
    for spec in &specs {
        for name in [
            "OPENCODE_DISABLE_AUTOUPDATE",
            "OPENCODE_DISABLE_PROJECT_CONFIG",
            "OPENCODE_CONFIG_PROJECT_DISABLE",
            "OPENCODE_DISABLE_MODELS_FETCH",
            "OPENCODE_FILEWATCHER_DISABLE",
            "OPENCODE_DISABLE_FILEWATCHER",
        ] {
            assert!(spec.env.contains(&env(name)), "{name}: {:?}", argv(spec));
        }
        assert_eq!(spec.cwd.as_deref(), Some(fixture.project()));
    }

    // The allow-list: every argv, the session id as a placeholder. Nothing else ever runs.
    let used: BTreeSet<String> = runner
        .argv()
        .iter()
        .map(|argv| argv.join(" ").replace(EDIT, "<id>"))
        .collect();
    let allowed = BTreeSet::from([
        "session list --standalone --format json -n 5000".to_owned(),
        "session export --standalone <id>".to_owned(),
        "session delete --standalone <id>".to_owned(),
    ]);
    assert_eq!(used, allowed);
}

#[test]
fn the_harness_picks_the_commands_of_its_generation() {
    let fixture = Fixture::new();

    let commands = |generation: OpenCodeGeneration, list: ExecOutput, export: ExecOutput, id: &str| {
        let runner = Replay::answering([list, export]);
        let harness = OpenCode::with_generation(generation);
        let cli = harness.cli_sessions().expect("OpenCode sessions go through its CLI");
        let ctx = fixture.ctx(fixture.project(), &runner);

        cli.list(&ctx).unwrap();
        cli.messages(&ctx, &opencode(id), None, 10).unwrap();

        runner.argv()
    };

    let v2_argv = commands(
        OpenCodeGeneration::V2,
        list_output(&fixture),
        export_output(&fixture),
        EDIT,
    );
    assert_eq!(v2_argv[0], LIST_ARGS);
    assert_eq!(v2_argv[1], ["session", "export", "--standalone", EDIT]);

    let v1_argv = commands(
        OpenCodeGeneration::V1,
        fixture.list_output(),
        fixture.export_output(),
        opencode_session_support::MAIN,
    );
    assert_eq!(v1_argv[0], opencode_session_support::LIST_ARGS);
    assert_eq!(v1_argv[1], ["export", opencode_session_support::MAIN, "--pure"]);

    for argv in &v2_argv {
        assert!(argv.iter().any(|arg| arg == "--standalone"), "{argv:?}");
        assert!(!argv.iter().any(|arg| arg == "--pure"), "{argv:?}");
    }
    for argv in &v1_argv {
        assert!(argv.iter().any(|arg| arg == "--pure"), "{argv:?}");
        assert!(!argv.iter().any(|arg| arg == "--standalone"), "{argv:?}");
    }
}

#[test]
fn deleting_runs_the_cli_and_drops_the_cached_list() {
    let fixture = Fixture::new();
    let without_edit = recorded("session-list.json", fixture.project()).replace(EDIT, "ses_gone_from_fixture");
    let runner = Replay::answering([
        list_output(&fixture),
        ok(format!("Session {EDIT} deleted\n")),
        ok(without_edit),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = v2();

    sessions.list(&ctx).unwrap();
    sessions.list(&ctx).unwrap();
    assert_eq!(runner.specs().len(), 1, "the second list is cached");

    sessions.delete(&ctx, &opencode(EDIT)).unwrap();
    let listed = sessions.list(&ctx).unwrap();

    assert!(listed.iter().all(|item| item.session.id != EDIT));
    let commands: Vec<String> = runner.argv().iter().map(|argv| argv.join(" ")).collect();
    assert_eq!(
        commands,
        [
            LIST_ARGS.join(" "),
            format!("session delete --standalone {EDIT}"),
            LIST_ARGS.join(" "),
        ]
    );
}

#[test]
fn a_session_missing_from_the_project_or_the_cli_is_not_found() {
    let fixture = Fixture::new();
    let not_found = recorded("delete-not-found.stderr", fixture.project());
    let runner = Replay::answering([
        // Not in the project's list (cached, then listed again): nothing is deleted.
        ok("[]"),
        ok("[]"),
        // Listed, but gone by the time it is deleted or exported.
        list_output(&fixture),
        failed(&not_found),
        failed(&not_found),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = v2();

    assert_eq!(
        error_code(sessions.delete(&ctx, &opencode(EDIT))),
        IpcErrorCode::NotFound
    );
    assert_eq!(runner.specs().len(), 2, "{:?}", runner.argv());

    // The empty list is cached for a moment; a new instance lists again.
    let sessions = v2();
    assert_eq!(
        error_code(sessions.delete(&ctx, &opencode(EDIT))),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        error_code(sessions.messages(&ctx, &opencode(EDIT), None, 10)),
        IpcErrorCode::NotFound
    );
}

#[test]
fn hostile_session_ids_are_refused_before_anything_runs() {
    let fixture = Fixture::new();
    let runner = Replay::default();
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = v2();

    for id in ["--standalone", "-n", "ses_x;y", "ses_../x", "abc", ""] {
        let session = opencode(id);

        assert_eq!(
            error_code(sessions.messages(&ctx, &session, None, 10)),
            IpcErrorCode::InvalidInput,
            "{id:?}"
        );
        assert_eq!(
            error_code(sessions.delete(&ctx, &session)),
            IpcErrorCode::InvalidInput,
            "{id:?}"
        );
    }

    assert!(runner.specs().is_empty(), "spawned: {:?}", runner.argv());
}
