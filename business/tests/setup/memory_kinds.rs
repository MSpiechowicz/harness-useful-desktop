//! `harness-memory kinds`: which team databases the installed server can use. The answer is cached
//! per binary, an older server (a usage error) is cached as one without Amazon DocumentDB, and a
//! failed probe is not cached. Run against a rule-based stand-in.

use crate::common;
use crate::memory_cli_support;
use crate::team_support;

use harness_business::ecosystem::{KindsSupport, MemoryKinds};
use harness_business::env::Env;
use harness_business::process::ExecOutput;
use memory_cli_support::{install, installed, installed_at};
use pretty_assertions::assert_eq;
use team_support::{Cli, exits, wait};

fn kinds(stdout: &str) -> Cli {
    Cli::default().starting(&["kinds"], exits(0, stdout, ""))
}

#[test]
fn a_server_that_lists_documentdb_can_use_it_and_is_asked_once() {
    let (_dir, env) = installed();
    let probe = MemoryKinds::new();
    let cli = kinds("postgres\nlibsql\nmongodb\nDocumentDB \n");

    let listed = KindsSupport::Listed { documentdb: true };
    assert_eq!(wait(probe.support(&env, &cli)), listed);
    assert_eq!(wait(probe.support(&env, &cli)), listed);
    assert_eq!(cli.commands(), [["kinds"]]);
    assert!(listed.documentdb());
}

#[test]
fn a_server_that_does_not_list_documentdb_cannot_use_it() {
    let (_dir, env) = installed();
    let answer = wait(MemoryKinds::new().support(&env, &kinds("postgres\nlibsql\nmongodb\n")));

    assert_eq!(answer, KindsSupport::Listed { documentdb: false });
    assert!(!answer.documentdb());
}

#[test]
fn an_older_server_has_no_documentdb_and_the_answer_is_cached() {
    let (_dir, env) = installed();
    let probe = MemoryKinds::new();
    let usage = Cli::default().starting(&["kinds"], exits(2, "", "usage: harness-memory [mcp | check | …]"));

    assert_eq!(wait(probe.support(&env, &usage)), KindsSupport::Unsupported);
    assert_eq!(wait(probe.support(&env, &usage)), KindsSupport::Unsupported);
    assert_eq!(usage.commands().len(), 1, "the answer is cached");

    let unknown = Cli::default().starting(&["kinds"], exits(1, "", "error: unrecognized subcommand 'kinds'"));
    let probe = MemoryKinds::new();
    assert_eq!(wait(probe.support(&env, &unknown)), KindsSupport::Unsupported);
    assert_eq!(wait(probe.support(&env, &unknown)), KindsSupport::Unsupported);
    assert_eq!(unknown.commands().len(), 1, "an unknown command is cached too");
}

#[test]
fn a_failed_probe_answers_without_documentdb_and_is_asked_again() {
    let (_dir, env) = installed();
    let probe = MemoryKinds::new();

    let broken = Cli::default().starting(&["kinds"], exits(1, "", "harness-memory: something went wrong"));
    assert_eq!(wait(probe.support(&env, &broken)), KindsSupport::Unsupported);
    assert_eq!(wait(probe.support(&env, &broken)), KindsSupport::Unsupported);
    assert_eq!(
        broken.commands().len(),
        2,
        "a failure that is not a usage error is not cached"
    );

    let slow = Cli::default().starting(
        &["kinds"],
        ExecOutput {
            timed_out: true,
            ..ExecOutput::default()
        },
    );
    assert_eq!(wait(probe.support(&env, &slow)), KindsSupport::Unknown);
    assert_eq!(wait(probe.support(&env, &slow)), KindsSupport::Unknown);
    assert_eq!(slow.commands().len(), 2);

    let listed = kinds("mongodb\ndocumentdb\n");
    assert_eq!(
        wait(probe.support(&env, &listed)),
        KindsSupport::Listed { documentdb: true }
    );
}

#[test]
fn without_harness_memory_there_is_no_documentdb() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path());
    let cli = Cli::default();

    assert_eq!(wait(MemoryKinds::new().support(&env, &cli)), KindsSupport::NotInstalled);
    assert!(cli.commands().is_empty());
}

/// Replaces the binary at `path` with `content`, keeping its modification time.
fn replace_keeping_time(path: &std::path::Path, content: &str) {
    let modified = std::fs::metadata(path).unwrap().modified().unwrap();
    let fresh = path.with_extension("new");
    std::fs::write(&fresh, content).unwrap();
    common::set_mode(&fresh, 0o755);
    std::fs::rename(&fresh, path).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
}

#[test]
fn a_replaced_binary_is_asked_again_even_with_the_same_time() {
    let dir = tempfile::tempdir().unwrap();
    let (env, path) = installed_at(dir.path());
    install(&path, None);
    let probe = MemoryKinds::new();

    let older = kinds("postgres\nmongodb\n");
    assert_eq!(
        wait(probe.support(&env, &older)),
        KindsSupport::Listed { documentdb: false }
    );

    // Another length, same time.
    replace_keeping_time(&path, "a newer server");
    let newer = kinds("mongodb\ndocumentdb\n");
    assert_eq!(
        wait(probe.support(&env, &newer)),
        KindsSupport::Listed { documentdb: true }
    );

    // Same length and time, another file (inode) on Unix.
    if cfg!(unix) {
        replace_keeping_time(&path, "a server again");
        let again = kinds("mongodb\n");
        assert_eq!(
            wait(probe.support(&env, &again)),
            KindsSupport::Listed { documentdb: false }
        );
    }
}
