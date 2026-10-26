//! `harness-memory trust` (`memory_scripts_trust`): each id with its reviewed hash and the project
//! on its command line, a time limit, a changed script back as `conflict`, and its error line back
//! redacted. Run against a stub runner and `fake_agent`
//! copied as `harness-memory` (see its docs).
//!
//! The copy runs in one test so no other test of this binary forks while it is open for writing.

use crate::memory_cli_support;

use std::ffi::OsString;
use std::path::Path;

use harness_business::ecosystem::memory_trust::{MAX_TRUSTED_IDS, trust_args, trust_timeout};
use harness_business::ecosystem::{Repos, trust_scripts};
use harness_business::env::Env;
use harness_business::model::ScriptTrustItem;
use harness_business::process::{ExecOutput, ProcessRunner, TokioRunner};
use harness_business::{BusinessError, Result};
use memory_cli_support::{install, installed_at, stub};
use pretty_assertions::assert_eq;

fn trust(env: &Env, runner: &dyn ProcessRunner, items: &[ScriptTrustItem], project: Option<&Path>) -> Result<()> {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(trust_scripts(env, runner, items, Repos::Project(project)))
}

/// Script `id` with the hash the stand-in gives it (`id` as 64 hex digits).
fn reviewed(id: i64) -> ScriptTrustItem {
    item(id, &format!("{id:064x}"))
}

fn item(id: i64, sha256: &str) -> ScriptTrustItem {
    ScriptTrustItem {
        id,
        sha256: sha256.to_owned(),
    }
}

fn texts(args: &[OsString]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[test]
fn each_id_with_its_hash_then_the_project_go_on_the_command_line() {
    let project = Path::new("/home/u/projects/demo");
    let sha = "0123456789abcdef".repeat(4);

    assert_eq!(
        texts(&trust_args(&[item(12, &sha), reviewed(7)], Repos::Project(Some(project))).unwrap()),
        [
            "trust".to_owned(),
            format!("12={sha}"),
            format!("7={:064x}", 7),
            "--project".to_owned(),
            "/home/u/projects/demo".to_owned()
        ]
    );
    assert_eq!(
        texts(&trust_args(&[reviewed(3)], Repos::Project(None)).unwrap()),
        ["trust", &format!("3={:064x}", 3)]
    );
    assert_eq!(
        texts(&trust_args(&[reviewed(3)], Repos::Key("git:github.com/acme/widget")).unwrap()),
        [
            "trust",
            &format!("3={:064x}", 3),
            "--repo",
            "git:github.com/acme/widget"
        ]
    );
    assert_eq!(
        texts(&trust_args(&[reviewed(3)], Repos::All).unwrap()),
        ["trust", &format!("3={:064x}", 3), "--all-repos"]
    );

    let too_many: Vec<ScriptTrustItem> = (1..=i64::try_from(MAX_TRUSTED_IDS).unwrap() + 1)
        .map(reviewed)
        .collect();
    let bad_ids = [vec![], vec![reviewed(0)], vec![reviewed(5), item(-1, &sha)], too_many];
    let bad_hashes = [
        "",
        &sha[..63],
        &format!("{sha}0"),
        &sha.to_uppercase(),
        &format!("{}g", &sha[..63]),
        &format!("{} --all", &sha[..58]),
    ];

    let bad = bad_ids
        .into_iter()
        .chain(bad_hashes.iter().map(|hash| vec![reviewed(5), item(6, hash)]));

    for items in bad {
        let result = trust_args(&items, Repos::Project(None));
        assert!(
            matches!(result, Err(BusinessError::InvalidInput(_))),
            "{items:?}: {result:?}"
        );
    }
}

#[test]
fn the_server_runs_in_the_home_folder_with_a_time_limit_that_counts_the_scripts() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let runner = stub(ExecOutput {
        status: Some(0),
        stdout: "trusted 12\n".to_owned(),
        ..ExecOutput::default()
    });
    let project = home.path().join("demo");

    trust(&env, &runner, &[reviewed(12)], Some(&project)).expect("trusted");

    let runs = runner.runs.lock().unwrap();
    let spec = &runs[0];
    assert_eq!(spec.program, server);
    assert_eq!(
        texts(&spec.args),
        [
            "trust".to_owned(),
            format!("12={:064x}", 12),
            "--project".to_owned(),
            project.to_string_lossy().into_owned()
        ]
    );
    assert_eq!(spec.cwd.as_deref(), Some(home.path()));
    assert_eq!(spec.timeout, trust_timeout(1));
}

#[test]
fn the_time_allowed_grows_with_the_scripts_up_to_two_minutes() {
    assert_eq!(trust_timeout(1).as_secs(), 21);
    assert_eq!(trust_timeout(50).as_secs(), 70);
    assert_eq!(trust_timeout(100).as_secs(), 120);
    assert_eq!(trust_timeout(MAX_TRUSTED_IDS).as_secs(), 120);
}

#[test]
fn the_last_server_line_decides_the_message() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);

    let failed = |stderr: &str| {
        let runner = stub(ExecOutput {
            status: Some(1),
            stderr: stderr.to_owned(),
            ..ExecOutput::default()
        });

        trust(&env, &runner, &[reviewed(12)], None)
    };

    // Warnings come first and the server's own verdict last, so the first line is not the one to show.
    let process = failed(
        "warning: slow store\nharness-memory: script 3 not found\nharness-memory: the team database cannot be reached\n",
    );
    assert!(
        matches!(&process, Err(BusinessError::Process(message)) if message == "harness-memory: the team database cannot be reached"),
        "{process:?}"
    );

    let conflict = failed("harness-memory: trusted 4\nharness-memory: script 7 changed since you reviewed it\n");
    assert!(
        matches!(&conflict, Err(BusinessError::Conflict(message)) if message == "harness-memory: script 7 changed since you reviewed it"),
        "{conflict:?}"
    );

    let unprefixed = failed("first\n\nsecond\n");
    assert!(
        matches!(&unprefixed, Err(BusinessError::Process(message)) if message == "second"),
        "{unprefixed:?}"
    );
}

#[test]
fn a_server_that_does_not_answer_in_time_is_a_process_error() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let runner = stub(ExecOutput {
        timed_out: true,
        ..ExecOutput::default()
    });

    let result = trust(&env, &runner, &[reviewed(12)], None);
    assert!(
        matches!(&result, Err(BusinessError::Process(message)) if message.contains("within 21 seconds")),
        "{result:?}"
    );
}

#[test]
fn the_installed_server_trusts_known_ids_and_its_errors_come_back_redacted() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());

    let missing = trust(&env, &TokioRunner, &[reviewed(12)], None);
    assert!(matches!(missing, Err(BusinessError::NotFound(_))), "{missing:?}");

    install(&server, Some(Path::new(env!("CARGO_BIN_EXE_fake_agent"))));

    trust(&env, &TokioRunner, &[reviewed(12), reviewed(7)], Some(home.path())).expect("fixture ids are trusted");

    // Script 7 was edited after the user looked at it: the UI loads it again and asks.
    let changed = trust(&env, &TokioRunner, &[reviewed(12), item(7, &"ab".repeat(32))], None);
    assert!(
        matches!(&changed, Err(BusinessError::Conflict(message)) if message == "harness-memory: script 7 changed since you reviewed it"),
        "{changed:?}"
    );
    assert_eq!(
        changed.unwrap_err().code(),
        harness_business::model::IpcErrorCode::Conflict
    );

    let unknown = trust(&env, &TokioRunner, &[reviewed(42)], None);
    assert!(
        matches!(&unknown, Err(BusinessError::Process(message)) if message == "error: script 42 not found"),
        "{unknown:?}"
    );

    let failing = trust(&env, &TokioRunner, &[reviewed(500)], None);
    let Err(BusinessError::Process(message)) = &failing else {
        panic!("expected a process error, got {failing:?}");
    };
    assert!(message.contains("the team database cannot be reached"), "{message}");
    assert!(!message.contains("hunter2"), "{message}");
}
