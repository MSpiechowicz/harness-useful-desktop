//! `harness-memory hooks …` (team hooks): the support probe (`hooks config --json`, cached until
//! the binary or the config file changes, a failure not cached), and `hooks list|put|delete` with
//! their arguments, paging, stdin and failure answers. Run against a rule-based stand-in.

mod common;
mod memory_cli_support;
mod team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::memory_config_path;
use harness_business::ecosystem::memory_hooks::{delete_team_hook, list_team_hooks, project_key, put_team_hook};
use harness_business::ecosystem::{HooksConfig, HooksSupport, MemoryHooks};
use harness_business::env::Env;
use memory_cli_support::{install, installed_at};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{Cli, REPO, answers, exits, id, number_after, row, row_json, server, wait, with_project};

/// A home with an installed `harness-memory` (never run: the stand-in answers).
fn installed() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let (env, path) = installed_at(dir.path());
    install(&path, None);

    (dir, env)
}

fn config(hooks: &str, trust: bool) -> Cli {
    Cli::default().starting(
        &["hooks", "config", "--json"],
        answers(json!({"hooks": hooks, "trustTeamHooks": trust})),
    )
}

#[test]
fn the_probe_reads_the_config_and_is_cached_until_the_config_file_changes() {
    let (_dir, env) = installed();
    let probe = MemoryHooks::new();
    let cli = config("remote", true);

    let shared = HooksSupport::Supported(HooksConfig {
        remote: true,
        trust: true,
    });
    assert_eq!(wait(probe.support(&env, &cli)), shared);
    assert_eq!(wait(probe.support(&env, &cli)), shared);
    assert_eq!(cli.commands(), [["hooks", "config", "--json"]]);
    assert_eq!(shared.sharing().map(|config| config.trust), Some(true));

    // A saved setting changes the config file: the server is asked again.
    let file = memory_config_path(&env).unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "hooks = \"local\"\n").unwrap();

    let local = config("local", false);
    let answer = wait(probe.support(&env, &local));
    assert_eq!(
        answer,
        HooksSupport::Supported(HooksConfig {
            remote: false,
            trust: false
        })
    );
    assert!(answer.keeps_hooks() && answer.sharing().is_none());

    let off = wait(probe.sharing_server(&env, &local));
    assert!(matches!(off, Err(BusinessError::Unsupported(_))), "{off:?}");
}

#[test]
fn an_older_server_has_no_team_hooks() {
    let (_dir, env) = installed();
    let probe = MemoryHooks::new();
    let cli = Cli::default().starting(&["hooks"], exits(2, "", "usage: harness-memory [mcp | check | …]"));

    assert_eq!(wait(probe.support(&env, &cli)), HooksSupport::Unsupported);
    assert_eq!(wait(probe.support(&env, &cli)), HooksSupport::Unsupported);
    assert_eq!(cli.commands().len(), 1, "the answer is cached");

    let unknown = Cli::default().starting(&["hooks"], exits(1, "", "error: unrecognized subcommand 'hooks'"));
    assert_eq!(
        wait(MemoryHooks::new().support(&env, &unknown)),
        HooksSupport::Unsupported
    );
}

#[test]
fn a_failed_probe_is_not_cached() {
    let (_dir, env) = installed();
    let probe = MemoryHooks::new();
    let broken = Cli::default().starting(&["hooks"], exits(1, "", "harness-memory: the config file is invalid"));

    assert_eq!(wait(probe.support(&env, &broken)), HooksSupport::Unknown);
    assert_eq!(wait(probe.support(&env, &broken)), HooksSupport::Unknown);
    assert_eq!(broken.commands().len(), 2);

    let garbled = Cli::default().starting(&["hooks"], exits(0, "not json", ""));
    assert_eq!(wait(probe.support(&env, &garbled)), HooksSupport::Unknown);

    assert_eq!(
        wait(probe.support(&env, &config("remote", false)))
            .sharing()
            .map(|c| c.trust),
        Some(false)
    );
}

#[test]
fn without_harness_memory_there_are_no_team_hooks() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path());

    assert_eq!(
        wait(MemoryHooks::new().support(&env, &Cli::default())),
        HooksSupport::NotInstalled
    );
}

/// A server that answers `hooks list` pages of `rows` by `--offset` and `--limit`, without
/// `nextOffset` (an older one), leaving out the first row of each page as if it skipped it.
fn older_server(rows: Vec<harness_business::hooks::team::TeamRow>) -> Cli {
    Cli::default().answering(move |args| {
        let offset = number_after(args, "--offset")?;
        let limit = number_after(args, "--limit")?;
        let page: Vec<_> = rows.iter().skip(offset + 1).take(limit - 1).map(row_json).collect();

        Some(answers(json!({
            "total": rows.len(),
            "project": {"key": REPO, "shareable": true},
            "hooks": page,
        })))
    })
}

#[test]
fn an_older_list_advances_by_the_page_size_not_by_the_rows_returned() {
    let env = Env::for_tests(std::path::Path::new("/home/u"));
    let rows: Vec<_> = (1..=40).map(|n| row(&id(n), &format!("Rule {n}"), None)).collect();
    let cli = older_server(rows.clone());

    let project = std::path::Path::new("/home/u/demo");
    let list = wait(list_team_hooks(&env, &cli, &server(), Some(project))).unwrap();

    let expected: Vec<_> = [&rows[1..16], &rows[17..32], &rows[33..40]].concat();
    assert_eq!(list.rows, expected);
    assert!(list.complete);
    assert_eq!(list.project.unwrap().key, REPO);

    let commands = cli.commands();
    assert_eq!(
        commands[0],
        [
            "hooks",
            "list",
            "--json",
            "--offset",
            "0",
            "--limit",
            "16",
            "--project",
            "/home/u/demo"
        ]
    );
    let offsets: Vec<_> = commands
        .iter()
        .map(|args| number_after(args, "--offset").unwrap())
        .collect();
    assert_eq!(offsets, [0, 16, 32]);
    assert!(with_project(&commands[1], "/home/u/demo"));

    let key = wait(project_key(&env, &cli, &server(), project)).unwrap();
    assert_eq!((key.key.as_str(), key.shareable), (REPO, true));
}

#[test]
fn a_put_sends_the_body_on_stdin_and_names_the_version_it_updates() {
    let env = Env::for_tests(std::path::Path::new("/home/u"));
    let stored = row(&id(1), "Team rule", Some(REPO));
    let cli = Cli::default().starting(&["hooks", "put"], answers(row_json(&stored)));
    let sha = "a".repeat(64);

    let answer = wait(put_team_hook(
        &env,
        &cli,
        &server(),
        stored.body.as_bytes(),
        Some(std::path::Path::new("/home/u/demo")),
        Some(&sha),
    ))
    .unwrap();
    assert_eq!(answer, stored);

    let runs = cli.runs.lock().unwrap();
    assert_eq!(
        team_support::texts(&runs[0].args),
        [
            "hooks",
            "put",
            "--json",
            "--project",
            "/home/u/demo",
            "--expect-sha",
            &sha
        ]
    );
    assert_eq!(runs[0].stdin.as_deref(), Some(stored.body.as_bytes()));
}

#[test]
fn failure_answers_become_the_matching_errors() {
    let env = Env::for_tests(std::path::Path::new("/home/u"));
    let failing = |code: &str| {
        let stdout = json!({"error": code, "message": format!("{code} said the server")}).to_string();
        Cli::default().starting(&["hooks"], exits(1, &stdout, "harness-memory: refused"))
    };
    let put = |cli: &Cli| wait(put_team_hook(&env, cli, &server(), b"{}", None, None));

    assert!(
        matches!(put(&failing("conflict")), Err(BusinessError::Conflict(message)) if message == "conflict said the server")
    );
    assert!(matches!(put(&failing("refused")), Err(BusinessError::Forbidden(_))));
    assert!(matches!(put(&failing("invalid")), Err(BusinessError::InvalidInput(_))));
    assert!(matches!(put(&failing("limit")), Err(BusinessError::InvalidInput(_))));
    assert!(matches!(put(&failing("local")), Err(BusinessError::Unsupported(_))));
    assert!(matches!(put(&failing("notFound")), Err(BusinessError::NotFound(_))));

    let older = Cli::default().starting(&["hooks"], exits(2, "", "usage: harness-memory …"));
    assert!(matches!(put(&older), Err(BusinessError::Unsupported(_))));

    let plain = Cli::default().starting(&["hooks"], exits(1, "", "harness-memory: the database is down"));
    assert!(matches!(put(&plain), Err(BusinessError::Process(message)) if message.contains("database is down")));
}

#[test]
fn a_delete_names_the_id_and_version_and_checks_them_first() {
    let env = Env::for_tests(std::path::Path::new("/home/u"));
    let cli = Cli::default().starting(&["hooks", "delete"], answers(json!({"deleted": true})));
    let sha = "b".repeat(64);

    assert!(wait(delete_team_hook(&env, &cli, &server(), &id(3), Some(&sha))).unwrap());
    assert_eq!(
        cli.commands(),
        [vec![
            "hooks".to_owned(),
            "delete".to_owned(),
            id(3),
            "--expect-sha".to_owned(),
            sha.clone(),
            "--json".to_owned()
        ]]
    );

    let bad_id = wait(delete_team_hook(&env, &cli, &server(), "--all", Some(&sha)));
    assert!(matches!(bad_id, Err(BusinessError::InvalidInput(_))));

    let bad_sha = wait(delete_team_hook(&env, &cli, &server(), &id(3), Some("--yes")));
    assert!(matches!(bad_sha, Err(BusinessError::InvalidInput(_))));
    assert_eq!(cli.commands().len(), 1, "nothing ran for the bad arguments");
}

#[test]
fn a_list_follows_next_offset_past_a_page_of_skipped_rows() {
    let env = Env::for_tests(std::path::Path::new("/home/u"));
    let wanted = row(&id(7), "Team rule", None);
    let found = wanted.clone();

    // The first page held only a row the server skipped: no hooks, and the next page starts after it.
    let cli = Cli::default().answering(move |args| match number_after(args, "--offset")? {
        0 => Some(answers(
            json!({"total": 2, "nextOffset": 1, "project": null, "hooks": []}),
        )),
        1 => Some(answers(
            json!({"total": 2, "nextOffset": null, "project": null, "hooks": [row_json(&found)]}),
        )),
        _ => None,
    });

    let list = wait(list_team_hooks(&env, &cli, &server(), None)).unwrap();

    assert_eq!(list.rows, [wanted]);
    assert!(list.complete);
    assert_eq!(cli.commands().len(), 2);
}

#[test]
fn a_list_longer_than_the_pages_read_is_incomplete() {
    let env = Env::for_tests(std::path::Path::new("/home/u"));
    let cli = Cli::default().answering(|args| {
        let offset = number_after(args, "--offset")?;

        Some(answers(
            json!({"total": 10_000, "nextOffset": offset + 16, "project": null, "hooks": []}),
        ))
    });

    let list = wait(list_team_hooks(&env, &cli, &server(), None)).unwrap();

    assert!(!list.complete);
    assert_eq!(cli.commands().len(), 16);

    // A `nextOffset` that does not move forward is unreadable, not a loop.
    let stuck = Cli::default().starting(
        &["hooks", "list"],
        answers(json!({"total": 5, "nextOffset": 0, "project": null, "hooks": []})),
    );
    let error = wait(list_team_hooks(&env, &stuck, &server(), None));
    assert!(matches!(error, Err(BusinessError::Protocol(_))), "{error:?}");
}

#[test]
fn the_probe_cache_follows_the_config_file_harness_memory_config_names() {
    let (dir, env) = installed();
    let custom = dir.path().join("custom.toml");
    let env = env.with_var("HARNESS_MEMORY_CONFIG", custom.to_string_lossy());
    let probe = MemoryHooks::new();

    std::fs::write(&custom, "hooks = \"remote\"\n").unwrap();
    assert!(wait(probe.support(&env, &config("remote", true))).sharing().is_some());

    // The default file changes: the answer stays cached.
    let default = memory_config_path(&Env::for_tests(&env.home)).unwrap();
    std::fs::create_dir_all(default.parent().unwrap()).unwrap();
    std::fs::write(&default, "x").unwrap();
    let unasked = config("local", false);
    assert!(wait(probe.support(&env, &unasked)).sharing().is_some());
    assert!(unasked.commands().is_empty());

    // The named file changes: the server is asked again.
    std::fs::write(&custom, "hooks = \"local\"\n").unwrap();
    assert!(wait(probe.support(&env, &unasked)).sharing().is_none());
    assert_eq!(unasked.commands().len(), 1);
}
