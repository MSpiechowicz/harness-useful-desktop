//! Harness Knowledge while its remote database cannot be reached (the `offline-fallback` feature):
//! the `offline` flag on lists, repositories, counts and the live pulse, the `--place` filter, and
//! a remote document that cannot be read as an `unreachable` error instead of a failure.

use crate::knowledge_stand_in::{ALL_FEATURES, cli, document, installed, server};
use crate::team_support::{answers, exits, wait};

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeFeatures, ListWindow, get_doc, knowledge_doc_matches, knowledge_pulse, list_docs, list_knowledge_repos,
    read_stats, rename_doc,
};
use harness_business::model::{KnowledgeAvailability, KnowledgeId, KnowledgeListFilter, KnowledgePlace};
use pretty_assertions::assert_eq;
use serde_json::json;

const UNREACHABLE: &str = "harness-knowledge: remote knowledge is unreachable right now\n";

fn window() -> ListWindow {
    ListWindow::new(Some(0), Some(50), None, None).unwrap()
}

fn place_filter(place: KnowledgePlace) -> KnowledgeListFilter {
    KnowledgeListFilter {
        place: Some(place),
        ..KnowledgeListFilter::default()
    }
}

#[test]
fn a_list_read_while_offline_says_so_and_a_normal_one_does_not() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let offline = cli(ALL_FEATURES)
        .starting(
            &["docs", "list"],
            answers(json!({ "items": [document(1, "local guide")], "total": 1, "offline": true })),
        )
        .starting(
            &["repos"],
            answers(json!({ "global": 1, "repos": [], "offline": true })),
        );
    let online = cli(ALL_FEATURES).starting(
        &["docs", "list"],
        answers(json!({ "items": [document(1, "guide")], "total": 1 })),
    );

    let page = wait(list_docs(
        &env,
        &offline,
        &probe,
        &KnowledgeListFilter::default(),
        &window(),
    ))
    .unwrap();
    assert_eq!(
        (page.availability, page.total, page.offline),
        (KnowledgeAvailability::Available, 1, true)
    );

    let page = wait(list_docs(
        &env,
        &online,
        &probe,
        &KnowledgeListFilter::default(),
        &window(),
    ))
    .unwrap();
    assert!(!page.offline);

    assert!(wait(list_knowledge_repos(&env, &offline, &probe)).offline);
}

#[test]
fn the_place_filter_is_the_servers_place_argument() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES).starting(&["docs", "list"], answers(json!({ "items": [], "total": 0 })));

    for (place, name) in [(KnowledgePlace::Remote, "remote"), (KnowledgePlace::Local, "local")] {
        wait(list_docs(&env, &server_cli, &probe, &place_filter(place), &window())).unwrap();

        let args = server_cli.commands().pop().unwrap();
        let at = args.iter().position(|arg| arg == "--place").expect("the place is sent");
        assert_eq!(args[at + 1], name);
    }

    wait(list_docs(
        &env,
        &server_cli,
        &probe,
        &KnowledgeListFilter::default(),
        &window(),
    ))
    .unwrap();
    assert!(!server_cli.commands().pop().unwrap().contains(&"--place".to_owned()));
}

#[test]
fn a_remote_list_asked_while_offline_is_an_empty_offline_page() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES).starting(&["docs", "list"], exits(1, "", UNREACHABLE));

    let page = wait(list_docs(
        &env,
        &server_cli,
        &probe,
        &place_filter(KnowledgePlace::Remote),
        &window(),
    ))
    .unwrap();

    assert_eq!(
        (page.availability, page.items.len(), page.total, page.offline),
        (KnowledgeAvailability::Available, 0, 0, true)
    );
}

#[test]
fn counts_and_the_pulse_carry_the_offline_flag() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES).starting(
        &["stats"],
        answers(json!({
            "documents": 2, "chunks": 9, "embeddedChunks": 9, "bytes": 1200, "globalDocuments": 1,
            "repositories": 1, "generation": 5, "schema": 1, "model": null, "held": 0, "offline": true,
            "places": { "remote": { "offline": true }, "local": { "documents": 2, "chunks": 9 } }
        })),
    );

    let stats = wait(read_stats(&env, &server_cli, &server(&env))).unwrap();
    let places = stats.places.expect("both places");
    assert!(stats.offline && places.remote.expect("remote").offline);
    assert!(!places.local.expect("local").offline);

    let pulse = wait(knowledge_pulse(&env, &server_cli, &probe)).expect("the counts");
    assert_eq!((pulse.documents, pulse.offline), (2, true));
}

#[test]
fn a_remote_document_that_cannot_be_read_is_unreachable_and_a_refused_write_keeps_its_sentence() {
    let (_dir, env) = installed();
    let path = server(&env);
    let remote = KnowledgeId::Remote(7);
    let refused = "harness-knowledge: the remote database is unreachable, nothing was renamed\n";
    let server_cli = cli(ALL_FEATURES)
        .starting(&["docs", "get"], exits(1, "", UNREACHABLE))
        .starting(&["docs", "matches"], exits(1, "", UNREACHABLE))
        .starting(&["docs", "rename"], exits(1, "", refused));

    let error = wait(get_doc(&env, &server_cli, &path, &remote, None)).expect_err("offline");
    assert!(matches!(error, BusinessError::Unreachable(_)), "{error:?}");

    let error = wait(knowledge_doc_matches(
        &env,
        &server_cli,
        &path,
        &remote,
        "gateway",
        None,
    ))
    .expect_err("offline");
    assert!(matches!(error, BusinessError::Unreachable(_)), "{error:?}");

    let error = wait(rename_doc(&env, &server_cli, &path, &remote, "New")).expect_err("refused");
    assert!(
        matches!(error, BusinessError::Process(message) if message.ends_with("nothing was renamed")),
        "a refused write is the server's sentence, not an unreachable read"
    );
}
