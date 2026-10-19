//! `memory_remote_test`: `harness-memory check` gets the typed URL and token through its environment
//! (never its command line), uses the saved ones when none is typed, and its answer comes back
//! redacted. Run against `fake_agent` copied as `harness-memory` (see its docs) and a stub runner.
//!
//! The copy runs in one test so no other test of this binary forks while it is open for writing.

mod common;
mod memory_cli_support;

use std::ffi::OsString;
use std::path::Path;

use common::set_mode;
use harness_business::ecosystem::memory_remote::CHECK_TIMEOUT;
use harness_business::ecosystem::{KindsSupport, memory_remote_test};
use harness_business::env::Env;
use harness_business::model::{MemoryRemoteKind, MemoryRemoteTest};
use harness_business::process::{ExecOutput, ProcessRunner, TokioRunner};
use memory_cli_support::{install, installed_at, stub};
use pretty_assertions::assert_eq;

const PASSWORD: &str = "pa55-w0rd-secret";
const TOKEN: &str = "tok-e2e-secret";

fn save_config(env: &Env, text: &str) {
    let path = harness_business::ecosystem::memory_config_path(env).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    set_mode(&path, 0o600);
}

/// A server that can use Amazon DocumentDB.
const WITH_DOCUMENTDB: KindsSupport = KindsSupport::Listed { documentdb: true };

fn test(
    env: &Env,
    runner: &dyn ProcessRunner,
    kind: MemoryRemoteKind,
    url: Option<&str>,
    token: Option<&str>,
) -> MemoryRemoteTest {
    test_with(env, runner, kind, WITH_DOCUMENTDB, url, token)
}

fn test_with(
    env: &Env,
    runner: &dyn ProcessRunner,
    kind: MemoryRemoteKind,
    kinds: KindsSupport,
    url: Option<&str>,
    token: Option<&str>,
) -> MemoryRemoteTest {
    let future = memory_remote_test(
        env,
        runner,
        kind,
        kinds,
        url.map(str::to_owned),
        token.map(str::to_owned),
    );

    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

fn assert_hidden(result: &MemoryRemoteTest) {
    for secret in [PASSWORD, TOKEN] {
        assert!(!result.message.contains(secret), "{}", result.message);
    }
}

#[test]
fn the_installed_server_checks_the_typed_or_the_saved_database() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    let postgres = format!("postgres://team:{PASSWORD}@db.example.com/memory_ok");

    let missing = test(&env, &TokioRunner, MemoryRemoteKind::Postgres, Some(&postgres), None);
    assert_eq!(
        missing,
        MemoryRemoteTest {
            ok: false,
            message: "Harness Memory is not installed".to_owned()
        }
    );

    install(&server, Some(Path::new(env!("CARGO_BIN_EXE_fake_agent"))));

    // The fake answers `check` only when it is its whole command line, from the environment only.
    let ok = test(&env, &TokioRunner, MemoryRemoteKind::Postgres, Some(&postgres), None);
    assert_eq!(ok.message, "ok: Fake at postgres (schema 1) token unset");
    assert!(ok.ok);

    let libsql = test(
        &env,
        &TokioRunner,
        MemoryRemoteKind::Libsql,
        Some("libsql://ok.turso.io"),
        Some(TOKEN),
    );
    assert_eq!(libsql.message, "ok: Fake at libsql (schema 1) token set");

    let blank = test(
        &env,
        &TokioRunner,
        MemoryRemoteKind::Libsql,
        Some("libsql://ok.turso.io"),
        Some(""),
    );
    assert_eq!(
        blank.message, "ok: Fake at libsql (schema 1) token blank",
        "an empty token tests without one"
    );

    // A server that echoes the secrets has them masked.
    let echoed = test(
        &env,
        &TokioRunner,
        MemoryRemoteKind::Libsql,
        Some("https://fail.example.com/db"),
        Some(TOKEN),
    );
    assert!(!echoed.ok);
    assert!(echoed.message.starts_with("error: cannot reach"), "{}", echoed.message);
    assert_hidden(&echoed);

    let echoed = test(
        &env,
        &TokioRunner,
        MemoryRemoteKind::Postgres,
        Some(&format!("postgres://team:{PASSWORD}@fail.example.com/db")),
        None,
    );
    assert!(!echoed.ok);
    assert_hidden(&echoed);

    // Without a typed URL the saved one is used: nothing is put in the environment.
    let unsaved = test(&env, &TokioRunner, MemoryRemoteKind::Postgres, None, None);
    assert!(!unsaved.ok);
    assert!(
        unsaved.message.contains("no database URL is saved"),
        "{}",
        unsaved.message
    );

    save_config(&env, "database_url = \"postgres://h/db\"\n");
    let other_kind = test(&env, &TokioRunner, MemoryRemoteKind::Mongodb, None, None);
    assert!(
        other_kind.message.contains("not a MongoDB URL"),
        "{}",
        other_kind.message
    );

    let saved = test(&env, &TokioRunner, MemoryRemoteKind::Postgres, None, None);
    assert_eq!(
        saved,
        MemoryRemoteTest {
            ok: false,
            message: "fake_agent: no team database is configured".to_owned()
        },
        "the fake reads no file, so an unset variable is all it sees"
    );

    // A typed URL is validated first, without echoing it.
    let invalid = test(
        &env,
        &TokioRunner,
        MemoryRemoteKind::Libsql,
        Some(&format!("libsql://u:{PASSWORD}@t.turso.io")),
        None,
    );
    assert!(!invalid.ok);
    assert!(invalid.message.contains("no user, password"), "{}", invalid.message);
    assert_hidden(&invalid);
}

#[test]
fn the_secrets_go_through_the_environment_with_a_20_second_limit() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let url = format!("postgres://team:{PASSWORD}@db.example.com/memory");
    let runner = stub(ExecOutput {
        status: Some(0),
        stdout: "\nok: PostgreSQL at postgres://team@db.example.com/memory (schema 3)\nmore\n".to_owned(),
        ..ExecOutput::default()
    });

    let result = test(&env, &runner, MemoryRemoteKind::Postgres, Some(&url), Some(TOKEN));

    assert_eq!(
        result,
        MemoryRemoteTest {
            ok: true,
            message: "ok: PostgreSQL at postgres://••••@db.example.com/memory (schema 3)".to_owned()
        },
        "the app's own redaction masks URL user info too"
    );

    let checks = runner.runs.lock().unwrap();
    let spec = &checks[0];
    assert_eq!(spec.args, [OsString::from("check")]);
    assert_eq!(spec.timeout, CHECK_TIMEOUT);
    assert_eq!(CHECK_TIMEOUT.as_secs(), 20);
    assert_eq!(
        spec.env,
        [(OsString::from("HARNESS_MEMORY_DATABASE_URL"), OsString::from(&url))],
        "a PostgreSQL check gets no token"
    );
    for removed in [
        "HARNESS_MEMORY_DATABASE_URL",
        "HARNESS_MEMORY_DATABASE_TOKEN",
        "HARNESS_MEMORY_DATABASE_KIND",
        "HARNESS_MEMORY_CONFIG",
    ] {
        assert!(spec.env_remove_prefixes.contains(&OsString::from(removed)), "{removed}");
    }
}

#[test]
fn a_silent_failure_or_a_timeout_is_explained() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let check = |answer: ExecOutput| {
        test(
            &env,
            &stub(answer),
            MemoryRemoteKind::Libsql,
            Some("libsql://t.turso.io"),
            None,
        )
    };

    // The saved token is used (none typed), and masked when the server quotes it.
    save_config(
        &env,
        &format!("database_url = \"libsql://t.turso.io\"\ndatabase_token = \"{TOKEN}\"\n"),
    );
    let failed = check(ExecOutput {
        status: Some(1),
        stderr: format!("error: authentication failed for token {TOKEN}\nsecond line\n"),
        ..ExecOutput::default()
    });
    assert!(!failed.ok);
    assert!(
        failed.message.starts_with("error: authentication failed"),
        "{}",
        failed.message
    );
    assert_hidden(&failed);

    let silent = check(ExecOutput {
        status: Some(3),
        ..ExecOutput::default()
    });
    assert_eq!(silent.message, "harness-memory check failed (exit code 3)");

    let slow = check(ExecOutput {
        timed_out: true,
        ..ExecOutput::default()
    });
    assert_eq!(
        slow,
        MemoryRemoteTest {
            ok: false,
            message: "Harness Memory did not answer within 20 seconds".to_owned()
        }
    );
}

/// An Amazon DocumentDB cluster's URL as the AWS console shows it.
fn documentdb_url() -> String {
    format!(
        "mongodb://team:{PASSWORD}@memory.cluster-c1.eu-west-1.docdb.amazonaws.com:27017/?tls=true&tlsCAFile=global-bundle.pem&replicaSet=rs0&retryWrites=false"
    )
}

fn answered_ok() -> ExecOutput {
    ExecOutput {
        status: Some(0),
        stdout: "ok: DocumentDB\n".to_owned(),
        ..ExecOutput::default()
    }
}

fn kind_var(spec: &harness_business::process::ExecSpec) -> Option<OsString> {
    spec.env
        .iter()
        .find(|(name, _)| name == "HARNESS_MEMORY_DATABASE_KIND")
        .map(|(_, value)| value.clone())
}

#[test]
fn a_typed_documentdb_host_is_tested_as_the_kind_asked_for() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let url = documentdb_url();
    let runner = stub(answered_ok());

    let documentdb = test(&env, &runner, MemoryRemoteKind::Documentdb, Some(&url), None);
    assert!(documentdb.ok, "{}", documentdb.message);

    let mongodb = test(&env, &runner, MemoryRemoteKind::Mongodb, Some(&url), None);
    assert!(mongodb.ok, "{}", mongodb.message);

    let atlas = "mongodb+srv://team@cluster0.example.mongodb.net/memory";
    assert!(test(&env, &runner, MemoryRemoteKind::Mongodb, Some(atlas), None).ok);

    let checks = runner.runs.lock().unwrap();
    assert_eq!(
        checks[0].env,
        [
            (OsString::from("HARNESS_MEMORY_DATABASE_URL"), OsString::from(&url)),
            (
                OsString::from("HARNESS_MEMORY_DATABASE_KIND"),
                OsString::from("documentdb")
            ),
        ]
    );
    assert_eq!(
        kind_var(&checks[1]),
        Some(OsString::from("mongodb")),
        "MongoDB names the kind, so the server does not take the host for DocumentDB"
    );
    assert_eq!(kind_var(&checks[2]), None, "another host needs no kind");

    for spec in checks.iter() {
        assert!(
            spec.env_remove_prefixes
                .contains(&OsString::from("HARNESS_MEMORY_DATABASE_KIND")),
            "the app's own value is never passed on"
        );
    }
}

#[test]
fn a_saved_mongodb_url_is_tested_as_the_kind_asked_for() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let runner = stub(answered_ok());
    let check = |kind| test(&env, &runner, kind, None, None);
    let docdb = "mongodb://u@db.cluster-1.us-east-1.docdb.amazonaws.com:27017/m";

    // No `database_kind`: a DocumentDB host is DocumentDB, read by the server from the file.
    save_config(&env, &format!("database_url = \"{docdb}\"\n"));
    assert!(check(MemoryRemoteKind::Documentdb).ok);

    // Asked as MongoDB, the saved URL goes through the environment as MongoDB (no retyping).
    assert!(check(MemoryRemoteKind::Mongodb).ok);

    // `database_kind = "mongodb"`, asked as DocumentDB: the URL and the kind go through the
    // environment.
    save_config(
        &env,
        &format!("database_url = \"{docdb}\"\ndatabase_kind = \"mongodb\"\n"),
    );
    assert!(check(MemoryRemoteKind::Documentdb).ok);
    assert!(check(MemoryRemoteKind::Mongodb).ok);

    // `database_kind = "documentdb"` (any case) asked as DocumentDB is read from the file.
    save_config(
        &env,
        "database_url = \"mongodb://db.example.com/m\"\ndatabase_kind = \"DocumentDB\"\n",
    );
    assert!(check(MemoryRemoteKind::Documentdb).ok);

    // Another scheme is still refused.
    save_config(&env, "database_url = \"postgres://u@db.example.com/m\"\n");
    assert_eq!(
        check(MemoryRemoteKind::Documentdb).message,
        "the saved URL is not an Amazon DocumentDB URL, so enter one to test it"
    );
    assert_eq!(
        check(MemoryRemoteKind::Mongodb).message,
        "the saved URL is not a MongoDB URL, so enter one to test it"
    );

    let checks = runner.runs.lock().unwrap();
    let env_of = |index: usize| {
        let spec = &checks[index];
        let value = |name: &str| {
            spec.env
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        (
            value("HARNESS_MEMORY_DATABASE_URL").is_some(),
            value("HARNESS_MEMORY_DATABASE_KIND"),
        )
    };
    assert_eq!(checks.len(), 5, "the postgres URL never ran");
    assert_eq!(env_of(0), (false, None), "saved kind matches: the file is read");
    assert_eq!(
        env_of(1),
        (true, Some(OsString::from("mongodb"))),
        "switched to MongoDB: named, so the DocumentDB host does not decide"
    );
    assert_eq!(env_of(2), (true, Some(OsString::from("documentdb"))));
    assert_eq!(env_of(3), (false, None));
    assert_eq!(env_of(4), (false, None));
}

#[test]
fn documentdb_is_not_tested_with_a_server_that_cannot_use_it() {
    let home = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(home.path());
    install(&server, None);
    let runner = stub(answered_ok());
    let url = documentdb_url();

    let cases = [
        (
            KindsSupport::Listed { documentdb: false },
            "Update Harness Memory to use Amazon DocumentDB",
        ),
        (
            KindsSupport::Unsupported,
            "Update Harness Memory to use Amazon DocumentDB",
        ),
        (
            KindsSupport::NotInstalled,
            "Install Harness Memory to use Amazon DocumentDB",
        ),
        (
            KindsSupport::Unknown,
            "Harness Memory could not say whether it can use Amazon DocumentDB. Try again.",
        ),
    ];

    for (kinds, message) in cases {
        let result = test_with(&env, &runner, MemoryRemoteKind::Documentdb, kinds, Some(&url), None);
        assert_eq!(
            result,
            MemoryRemoteTest {
                ok: false,
                message: message.to_owned()
            }
        );
    }

    assert!(runner.runs.lock().unwrap().is_empty(), "the server is never run");

    let mongodb = test_with(
        &env,
        &runner,
        MemoryRemoteKind::Mongodb,
        KindsSupport::Unsupported,
        Some(&url),
        None,
    );
    assert!(mongodb.ok, "MongoDB needs no DocumentDB support");
}
