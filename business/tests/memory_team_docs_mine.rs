//! `harness-memory routines delete --mine --json` ("Remove all my team routines"): the command line
//! it runs, the ids it reads back, and its failures. Run against a rule-based stand-in.

mod team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::memory_team_docs::{TeamDocsRemoved, delete_mine_team_docs};
use harness_business::env::Env;
use harness_business::routines::RoutineKind;
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{Cli, answers, exits, id, server, wait};

fn env() -> Env {
    Env::for_tests(std::path::Path::new("/home/u"))
}

#[test]
fn it_names_no_author_and_reads_the_deleted_ids() {
    let cli = Cli::default().starting(
        &["routines", "delete"],
        answers(json!({"deleted": 3, "ids": [id(1), "--not-an-id", id(2)]})),
    );

    let removed = wait(delete_mine_team_docs::<RoutineKind>(&env(), &cli, &server())).expect("removed");

    assert_eq!(
        removed,
        TeamDocsRemoved {
            deleted: 3,
            ids: vec![id(1), id(2)],
        },
        "an id this app cannot read is left out"
    );
    assert_eq!(cli.commands(), [["routines", "delete", "--mine", "--json"]]);
}

#[test]
fn its_failures_become_the_matching_errors() {
    let local = json!({"error": "local", "message": "routines are kept on this machine"}).to_string();
    let cli = Cli::default().starting(&["routines"], exits(1, &local, "harness-memory: refused"));
    let off = wait(delete_mine_team_docs::<RoutineKind>(&env(), &cli, &server()));
    assert!(matches!(off, Err(BusinessError::Unsupported(_))), "{off:?}");

    let older = Cli::default().starting(&["routines"], exits(2, "", "usage: harness-memory …"));
    let older = wait(delete_mine_team_docs::<RoutineKind>(&env(), &older, &server()));
    assert!(matches!(older, Err(BusinessError::Unsupported(_))), "{older:?}");

    let garbled = Cli::default().starting(&["routines"], answers(json!({"ids": []})));
    let garbled = wait(delete_mine_team_docs::<RoutineKind>(&env(), &garbled, &server()));
    assert!(matches!(garbled, Err(BusinessError::Protocol(_))), "{garbled:?}");
}
