//! The user's own search of Harness Knowledge (`harness-knowledge search --json`): the exact
//! arguments for the project and scope, the array read strictly, and a query or limit the server
//! would refuse stopped before anything runs.

use std::path::Path;

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{KnowledgeFeatures, search_knowledge};
use harness_business::model::{KnowledgeHit, KnowledgeSearchScope};
use knowledge_stand_in::{ALL_FEATURES, cli, hit, installed};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{answers, exits, wait};

#[test]
fn a_search_asks_for_the_project_and_scope_and_reads_the_passages() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES).starting(
        &["search"],
        answers(json!([
            hit(
                7,
                31,
                "Gateway guide",
                "Guide > Routing",
                "How the gateway routes requests."
            ),
            hit(9, 40, "Runbook", "", "Restart the gateway.")
        ])),
    );

    let hits = wait(search_knowledge(
        &env,
        &server_cli,
        &probe,
        "  gateway routing ",
        Some(Path::new("/work/widget")),
        KnowledgeSearchScope::Repo,
        None,
    ))
    .unwrap();

    assert_eq!(
        hits[0],
        KnowledgeHit {
            doc_id: 7.into(),
            chunk_id: 31.into(),
            title: "Gateway guide".to_owned(),
            section: "Guide > Routing".to_owned(),
            snippet: "How the gateway routes requests.".to_owned(),
            score: 42,
        }
    );
    assert_eq!(hits[1].section, "");
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "search",
            "--json",
            "--query",
            "gateway routing",
            "--scope",
            "repo",
            "--limit",
            "10",
            "--project",
            "/work/widget"
        ]
    );

    wait(search_knowledge(
        &env,
        &server_cli,
        &probe,
        "gateway",
        None,
        KnowledgeSearchScope::All,
        Some(20),
    ))
    .unwrap();
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "search", "--json", "--query", "gateway", "--scope", "all", "--limit", "20"
        ]
    );
}

#[test]
fn no_match_is_an_empty_list_and_another_shape_is_a_protocol_error() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let search = |server_cli: &team_support::Cli| {
        wait(search_knowledge(
            &env,
            server_cli,
            &probe,
            "gateway",
            None,
            KnowledgeSearchScope::Repo,
            None,
        ))
    };

    assert!(
        search(&cli(ALL_FEATURES).starting(&["search"], answers(json!([]))))
            .unwrap()
            .is_empty()
    );

    for wrong in [
        json!({ "hits": [] }),
        json!([{ "docId": 1, "title": "No chunk" }]),
        json!("nothing"),
    ] {
        let server_cli = cli(ALL_FEATURES).starting(&["search"], answers(wrong));

        assert!(matches!(search(&server_cli), Err(BusinessError::Protocol(_))));
    }
}

#[test]
fn a_server_without_search_is_unsupported_and_a_failure_is_the_servers_line() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let old = cli("docs\nheld\n");

    let error = wait(search_knowledge(
        &env,
        &old,
        &probe,
        "gateway",
        None,
        KnowledgeSearchScope::Repo,
        None,
    ))
    .expect_err("no search");
    assert!(matches!(error, BusinessError::Unsupported(_)));
    assert_eq!(old.commands(), [["features"]]);

    let failing = cli(ALL_FEATURES).starting(
        &["search"],
        exits(1, "", "harness-knowledge: --project must be an existing folder\n"),
    );
    let error = wait(search_knowledge(
        &env,
        &failing,
        &KnowledgeFeatures::new(),
        "gateway",
        Some(Path::new("/gone")),
        KnowledgeSearchScope::Repo,
        None,
    ))
    .expect_err("failed");
    assert!(matches!(error, BusinessError::Process(message) if message.contains("existing folder")));
}

#[test]
fn a_query_or_limit_the_server_would_refuse_runs_nothing() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES);

    let long = "q".repeat(257);

    for (query, limit) in [
        ("   ", None),
        (long.as_str(), None),
        ("gateway", Some(0)),
        ("gateway", Some(21)),
    ] {
        let error = wait(search_knowledge(
            &env,
            &server_cli,
            &probe,
            query,
            None,
            KnowledgeSearchScope::Repo,
            limit,
        ))
        .expect_err("refused");

        assert!(matches!(error, BusinessError::InvalidInput(_)));
    }

    assert!(server_cli.commands().is_empty());
}
