//! `memory_remote_test`: `harness-memory check` gets the typed URL and token through its environment
//! (never its command line), uses the saved ones when none is typed, and its answer comes back
//! redacted. Run against `fake_agent` copied as `harness-memory` (see its docs) and a stub runner.
//!
//! The copy runs in one test so no other test of this binary forks while it is open for writing.

mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::set_mode;
use harness_business::Result;
use harness_business::ecosystem::memory_remote::CHECK_TIMEOUT;
use harness_business::ecosystem::memory_remote_test;
use harness_business::env::Env;
use harness_business::model::{MemoryRemoteKind, MemoryRemoteTest};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, TokioRunner};
use pretty_assertions::assert_eq;

const PASSWORD: &str = "pa55-w0rd-secret";
const TOKEN: &str = "tok-e2e-secret";

/// Where the installer puts `harness-memory` under `home` (and the env that finds it there).
fn installed_at(home: &Path) -> (Env, PathBuf) {
    let env = Env::for_tests(home);

    if cfg!(windows) {
        let local = home.join("AppData").join("Local");
        let path = local.join("Programs").join("harness-memory").join("harness-memory.exe");

        return (env.with_var("LOCALAPPDATA", local.to_string_lossy()), path);
    }

    (env, home.join(".local").join("bin").join("harness-memory"))
}

/// Places `source` as the installed server.
fn install(path: &Path, source: Option<&Path>) {
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    set_mode(dir, 0o755);

    match source {
        Some(source) => {
            std::fs::copy(source, path).unwrap();
        }
        None => std::fs::write(path, "").unwrap(),
    }

    set_mode(path, 0o755);
}

fn save_config(env: &Env, text: &str) {
    let path = harness_business::ecosystem::memory_config_path(env).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    set_mode(&path, 0o600);
}

fn test(
    env: &Env,
    runner: &dyn ProcessRunner,
    kind: MemoryRemoteKind,
    url: Option<&str>,
    token: Option<&str>,
) -> MemoryRemoteTest {
    let future = memory_remote_test(env, runner, kind, url.map(str::to_owned), token.map(str::to_owned));

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

/// Answers `--version` as Harness Memory, and `check` with `answer`; keeps the `check` spec.
struct Stub {
    answer: ExecOutput,
    checks: Mutex<Vec<ExecSpec>>,
}

impl ProcessRunner for Stub {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let output = if spec.args == [OsString::from("--version")] {
            ExecOutput {
                status: Some(0),
                stdout: "harness-memory 1.2.0\n".to_owned(),
                ..ExecOutput::default()
            }
        } else {
            self.checks.lock().unwrap().push(spec.clone());
            self.answer.clone()
        };

        Box::pin(async move { Ok(output) })
    }
}

fn stub(answer: ExecOutput) -> Stub {
    Stub {
        answer,
        checks: Mutex::new(Vec::new()),
    }
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

    let checks = runner.checks.lock().unwrap();
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
