//! Refresh rounds of the team copies (`fetch_team`, `apply_round`): what the server lists is
//! written, and what it no longer lists is removed scope by scope (global by the global query, a
//! repository by a root with its key), with the approvals of what leaves. Unmounted, failing,
//! refused and non-project roots keep or lose only their own scope. `harness-memory` is a
//! rule-based stand-in.

mod team_support;

use std::path::Path;

use harness_business::env::Env;
use harness_business::hooks::Approvals;
use harness_business::hooks::team::{
    TeamFetch, TeamRow, apply_round, approve_team_hook, clear_team, fetch_team, forget_team_hook, team_views,
};
use harness_business::model::HookStatus;
use pretty_assertions::assert_eq;
use team_support::{
    Cli, NONE, REPO, Team, answers, exits, id, listed, number_after, row, server, team, wait, with_project,
};

fn round(team: &Team, cli: &Cli, roots: &[String]) -> TeamFetch {
    let env = Env::for_tests(team.temp.path());

    wait(fetch_team(&env, cli, &server(), roots))
}

fn apply(team: &Team, fetch: &TeamFetch) -> Vec<String> {
    apply_round(&team.store, &team.folder, fetch).unwrap().removed
}

/// A server whose `hooks list` answers `global`, plus `repo` (under [`REPO`]) for a project.
fn serving(global: &[TeamRow], repo: &[TeamRow]) -> Cli {
    let project: Vec<TeamRow> = global.iter().chain(repo).cloned().collect();

    Cli::default()
        .on(
            |args| args.iter().any(|arg| arg == "--project"),
            listed(&project, Some((REPO, true))),
        )
        .starting(&["hooks", "list"], listed(global, None))
}

/// A server whose project lists fail (the team database is away for that query).
fn failing_projects(global: &[TeamRow]) -> Cli {
    Cli::default()
        .on(
            |args| args.iter().any(|arg| arg == "--project"),
            exits(1, "", "harness-memory: the team database did not answer"),
        )
        .starting(&["hooks", "list"], listed(global, None))
}

fn global_rule() -> TeamRow {
    row(&id(1), "Global rule", None)
}

fn repo_rule() -> TeamRow {
    row(&id(2), "Repo rule", Some(REPO))
}

/// A second folder of the same repository.
fn second_root(team: &Team) -> String {
    let dir = team.temp.path().join("demo-worktree");
    std::fs::create_dir_all(&dir).unwrap();

    dunce::canonicalize(dir).unwrap().to_string_lossy().into_owned()
}

fn approved(team: &Team, id: &str) -> bool {
    Approvals::read(team.store.config_dir()).unwrap().get(id).is_some()
}

#[test]
fn a_refresh_writes_what_the_server_lists_and_removes_what_it_no_longer_lists() {
    let team = team();
    let roots = [team.project.clone()];

    let fetch = round(&team, &serving(&[global_rule()], &[repo_rule()]), &roots);
    assert!(fetch.complete && fetch.global_listed, "{:?}", fetch.problem);
    assert_eq!(apply_round(&team.store, &team.folder, &fetch).unwrap().written, 2);
    assert_eq!(team.folder.projects()[&team.project].key, REPO);
    assert_eq!(team.folder.row(&id(2)).unwrap(), repo_rule());

    // Nothing changed: nothing is written again.
    let again = round(&team, &serving(&[global_rule()], &[repo_rule()]), &roots);
    assert!(!apply_round(&team.store, &team.folder, &again).unwrap().changed());

    // The repository hook left the team.
    let fetch = round(&team, &serving(&[global_rule()], &[]), &roots);
    assert_eq!(apply(&team, &fetch), [id(2)]);
    assert_eq!(team.folder.ids().len(), 1);
}

#[test]
fn a_failing_root_keeps_only_its_repository_and_global_copies_still_go() {
    let team = team();
    let roots = [team.project.clone()];
    apply(&team, &round(&team, &serving(&[global_rule()], &[repo_rule()]), &roots));

    // The global hook left the team while the project's list fails.
    let fetch = round(&team, &failing_projects(&[]), &roots);
    assert!(!fetch.complete && fetch.global_listed);
    assert_eq!(fetch.failed, roots);

    assert_eq!(apply(&team, &fetch), [id(1)]);
    assert!(team.folder.row(&id(2)).is_ok());
    assert_eq!(team.folder.projects()[&team.project].key, REPO, "the mapping stays");
}

#[test]
fn another_root_of_the_same_repository_decides_for_it() {
    let team = team();
    let roots = [team.project.clone(), second_root(&team)];
    apply(&team, &round(&team, &serving(&[], &[repo_rule()]), &roots));

    // The first root's list fails, the second's answers without the hook: it goes.
    let first = team.project.clone();
    let cli = Cli::default()
        .on(
            move |args| with_project(args, &first),
            exits(1, "", "harness-memory: the team database did not answer"),
        )
        .on(
            |args| args.iter().any(|arg| arg == "--project"),
            listed(&[], Some((REPO, true))),
        )
        .starting(&["hooks", "list"], listed(&[], None));

    let fetch = round(&team, &cli, &roots);
    assert!(!fetch.complete);
    assert!(fetch.listed_keys.contains(REPO));

    assert_eq!(apply(&team, &fetch), [id(2)]);
}

#[test]
fn the_filesystem_root_home_and_refused_roots_do_not_stall_a_round() {
    let team = team();
    team.copies(&[global_rule()]);

    let filesystem = Path::new(&team.project)
        .ancestors()
        .last()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let home = dunce::canonicalize(team.temp.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let refused = second_root(&team);

    let asked = refused.clone();
    let cli = Cli::default()
        .on(
            move |args| with_project(args, &asked),
            exits(1, r#"{"error":"refused","message":"not a project"}"#, ""),
        )
        .starting(&["hooks", "list"], listed(&[], None));

    let fetch = round(&team, &cli, &[filesystem.clone(), home.clone(), refused.clone()]);

    assert!(fetch.complete && fetch.problem.is_none(), "{:?}", fetch.problem);
    assert_eq!(fetch.refused, [filesystem, home, refused]);
    assert_eq!(
        cli.commands().len(),
        2,
        "only the global list and the folder the server refused"
    );
    assert_eq!(apply(&team, &fetch), [id(1)]);
}

#[test]
fn a_cut_short_or_endless_list_keeps_its_own_scope() {
    let team = team();
    let roots = [team.project.clone()];
    apply(&team, &round(&team, &serving(&[global_rule()], &[repo_rule()]), &roots));

    // The project's page was cut at the output cap: its repository keeps its copies.
    let cut = Cli::default()
        .on(
            |args| args.iter().any(|arg| arg == "--project"),
            exits(0, r#"{"total":1,"nextOffset":null,"hooks":[{"id":"000"#, ""),
        )
        .starting(&["hooks", "list"], listed(&[global_rule()], None));

    let fetch = round(&team, &cut, &roots);
    assert!(!fetch.complete && fetch.problem.is_some());
    assert!(apply(&team, &fetch).is_empty());

    // Every list is longer than the app reads: nothing is removed.
    let endless = Cli::default().answering(|args| {
        let offset = number_after(args, "--offset")?;

        Some(answers(serde_json::json!({
            "total": 10_000, "nextOffset": offset + 16, "project": {"key": REPO, "shareable": true}, "hooks": []
        })))
    });

    let fetch = round(&team, &endless, &roots);
    assert!(!fetch.complete && !fetch.global_listed && fetch.listed_keys.is_empty());
    assert!(apply(&team, &fetch).is_empty());
    assert_eq!(team.folder.ids().len(), 2);
}

#[test]
fn an_unmounted_root_keeps_its_repositorys_copies_and_approvals_until_it_is_back() {
    let team = team();
    let roots = [team.project.clone()];
    let both = serving(&[global_rule()], &[repo_rule()]);

    apply(&team, &round(&team, &both, &roots));
    approve_team_hook(&team.store, &team.folder, &id(2), &repo_rule().sha256).unwrap();
    std::fs::create_dir_all(team.store.hooks_dir()).unwrap();

    let status = || {
        let views = team_views(&team.store, &team.folder, NONE, false).views;
        views.iter().find(|view| view.id == id(2)).map(|view| view.status)
    };
    assert_eq!(status(), Some(HookStatus::Active));

    // The drive is unmounted: several rounds skip the root and keep its copies.
    let parked = team.temp.path().join("parked");
    std::fs::rename(&team.project, &parked).unwrap();

    for _ in 0..3 {
        let skipped = round(&team, &both, &roots);
        assert!(skipped.complete);
        assert_eq!(skipped.skipped, roots);
        assert!(apply(&team, &skipped).is_empty());
        assert_eq!(team.store.prune_approvals().unwrap(), 0);
    }

    assert_eq!(team.folder.projects()[&team.project].key, REPO);
    assert_eq!(status(), Some(HookStatus::Active));

    // Back again: in use at once, no new approval needed.
    std::fs::rename(&parked, &team.project).unwrap();
    assert!(
        !apply_round(&team.store, &team.folder, &round(&team, &both, &roots))
            .unwrap()
            .changed()
    );
    assert_eq!(status(), Some(HookStatus::Active));

    // Removed on the server while the root is mounted: the copy and its approval go.
    assert_eq!(
        apply(&team, &round(&team, &serving(&[global_rule()], &[]), &roots)),
        [id(2)]
    );
    assert!(team.folder.row(&id(1)).is_ok());
    assert!(!approved(&team, &id(2)));
}

#[test]
fn copies_of_a_repository_no_known_root_has_are_removed() {
    let team = team();
    team.copies(&[global_rule(), row(&id(3), "Old repo", Some("git:github.com/o/old"))]);

    let fetch = round(&team, &serving(&[global_rule()], &[]), &[]);

    assert_eq!(apply(&team, &fetch), [id(3)]);
}

#[test]
fn a_removed_team_copy_loses_its_approval_and_comes_back_pending() {
    let team = team();
    let shared = global_rule();
    team.copies(std::slice::from_ref(&shared));
    approve_team_hook(&team.store, &team.folder, &id(1), &shared.sha256).unwrap();
    let snapshot = team
        .store
        .config_dir()
        .join("hook-approved")
        .join(format!("{}.json", id(1)));
    assert!(approved(&team, &id(1)) && snapshot.exists());

    // Removed by a round: the approval and the approved copy go with it.
    assert_eq!(apply(&team, &round(&team, &serving(&[], &[]), &[])), [id(1)]);
    assert!(!approved(&team, &id(1)) && !snapshot.exists());

    // The same bytes published again wait for approval.
    apply(&team, &round(&team, &serving(std::slice::from_ref(&shared), &[]), &[]));
    let views = team_views(&team.store, &team.folder, NONE, false).views;
    assert_eq!(views[0].status, HookStatus::Pending);

    // Removed from the team, or sharing turned off: the same.
    approve_team_hook(&team.store, &team.folder, &id(1), &shared.sha256).unwrap();
    forget_team_hook(&team.store, &team.folder, &id(1)).unwrap();
    assert!(!approved(&team, &id(1)));

    team.copies(std::slice::from_ref(&shared));
    approve_team_hook(&team.store, &team.folder, &id(1), &shared.sha256).unwrap();
    assert!(clear_team(&team.store, &team.folder).unwrap());
    assert!(!approved(&team, &id(1)));
}

#[test]
fn a_local_hook_with_the_same_id_keeps_its_approval_when_the_team_copy_goes() {
    let team = team();
    let mine = team_support::local(&team.store, "Mine");
    team.copies(&[row(&mine.id, "Shared", None)]);

    forget_team_hook(&team.store, &team.folder, &mine.id).unwrap();

    assert!(approved(&team, &mine.id));
    assert_eq!(team.store.get(&mine.id, NONE).unwrap().status, HookStatus::Active);
}
