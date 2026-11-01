//! The review queue of documents agents proposed (`harness-knowledge held list|get|approve|reject`)
//! and the counts the live view follows: the exact arguments, the answers read strictly, the
//! refusals as outcomes, a server without the queue, and a bad id or revision refused before
//! anything runs.

use crate::knowledge_stand_in;
use crate::knowledge_stand_in::queue_page;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeFeatures, ListWindow, decide_knowledge_held, get_knowledge_held, knowledge_pulse,
};
use harness_business::model::{
    KnowledgeApproval, KnowledgeAvailability, KnowledgeDecided, KnowledgeDecision, KnowledgeEmbedding, SortOrder,
};
use knowledge_stand_in::{ALL_FEATURES, HELD_ID, HELD_ID_2, REPO, REVISION, cli, held_item, installed, server};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{Cli, answers, exits, wait};

fn queue() -> Cli {
    cli(ALL_FEATURES)
        .starting(
            &["held", "list"],
            answers(json!({ "held": [
                held_item(HELD_ID, "Retry policy", "preview", "Retry three times."),
                held_item(HELD_ID_2, "Runbook", "preview", "Page the on-call.")
            ] })),
        )
        .starting(
            &["held", "get"],
            answers(json!({ "held": held_item(HELD_ID, "Retry policy", "content", "All of it.") })),
        )
        .starting(
            &["held", "approve"],
            answers(json!({ "approved": {
                "heldId": HELD_ID, "docId": 12, "status": "added", "title": "Retry policy",
                "chunks": 3, "embedding": "pending"
            } })),
        )
        .starting(&["held", "reject"], answers(json!({ "rejected": HELD_ID })))
}

fn refusal(code: &str, message: &str) -> team_support::Cli {
    cli(ALL_FEATURES).on(
        |args| args.starts_with(&["held".to_owned()]),
        exits(
            1,
            &json!({ "error": code, "message": message }).to_string(),
            &format!("harness-knowledge: {message}\n"),
        ),
    )
}

#[test]
fn the_queue_is_one_call_and_the_window_filters_orders_and_pages_it_here() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = queue();

    let window = ListWindow::new(None, None, None, None).unwrap();
    let page = queue_page(&env, &server_cli, &probe, &window);

    assert_eq!(page.availability, KnowledgeAvailability::Available);
    assert_eq!(page.total, 2);
    assert_eq!(page.items[0].id, HELD_ID, "the server's newest first");
    assert_eq!(page.items[0].text, "Retry three times.", "a list carries the preview");
    assert_eq!(page.items[0].revision, REVISION);
    assert_eq!(page.items[0].links[0].repo, REPO);
    assert_eq!(page.items[0].client, "claude-code");
    assert_eq!(page.items[0].session.as_deref(), Some("chat-1"));
    assert_eq!(page.items[0].source_path, None);
    assert_eq!(server_cli.commands().pop().unwrap(), ["held", "list", "--json"]);

    let oldest_first = ListWindow::new(None, None, None, Some(SortOrder::Asc)).unwrap();
    let page = queue_page(&env, &server_cli, &probe, &oldest_first);
    assert_eq!(page.items[0].id, HELD_ID_2);

    let searched = ListWindow::new(None, None, Some("RUNBOOK".to_owned()), None).unwrap();
    let page = queue_page(&env, &server_cli, &probe, &searched);
    assert_eq!((page.total, page.items.len()), (1, 1));
    assert_eq!(page.items[0].id, HELD_ID_2);

    let second = ListWindow::new(Some(1), Some(1), None, None).unwrap();
    let page = queue_page(&env, &server_cli, &probe, &second);
    assert_eq!(page.total, 2, "the total counts every page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, HELD_ID_2);
}

#[test]
fn a_proposal_the_app_cannot_read_fails_the_page_instead_of_being_skipped() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let window = ListWindow::new(None, None, None, None).unwrap();

    let mut partial = held_item(HELD_ID_2, "Runbook", "preview", "x");
    partial.as_object_mut().unwrap().remove("revision");
    let server_cli = cli(ALL_FEATURES).starting(
        &["held", "list"],
        answers(json!({ "held": [held_item(HELD_ID, "Retry policy", "preview", "y"), partial] })),
    );

    let page = queue_page(&env, &server_cli, &probe, &window);
    assert!(matches!(page.availability, KnowledgeAvailability::Failed { .. }));
    assert!(page.items.is_empty());

    let other_shape = cli(ALL_FEATURES).starting(&["held", "list"], answers(json!({ "items": [], "total": 0 })));
    let page = queue_page(&env, &other_shape, &KnowledgeFeatures::new(), &window);
    assert!(matches!(page.availability, KnowledgeAvailability::Failed { .. }));
}

#[test]
fn a_server_without_the_queue_is_outdated_and_runs_nothing_else() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli("docs\nimport-ndjson\n");
    let window = ListWindow::new(None, None, None, None).unwrap();

    let page = queue_page(&env, &server_cli, &probe, &window);

    assert_eq!(page.availability, KnowledgeAvailability::Outdated);
    assert_eq!(server_cli.commands(), [["features"]]);
}

#[test]
fn a_proposal_is_read_whole() {
    let (_dir, env) = installed();
    let server_cli = queue();

    let item = wait(get_knowledge_held(&env, &server_cli, &server(&env), HELD_ID)).unwrap();

    assert_eq!(item.text, "All of it.", "a get carries the whole text");
    assert_eq!(item.revision, REVISION);
    assert_eq!(server_cli.commands().pop().unwrap(), ["held", "get", "--json", HELD_ID]);

    let gone = refusal("notFound", "no such held proposal, or another decision took it");
    let error = wait(get_knowledge_held(&env, &gone, &server(&env), HELD_ID)).expect_err("gone");
    assert!(matches!(error, BusinessError::NotFound(message) if message.contains("another decision")));
}

#[test]
fn an_approval_passes_the_revision_shown_and_answers_the_document_it_made() {
    let (_dir, env) = installed();
    let server_cli = queue();
    let path = server(&env);

    let approved = wait(decide_knowledge_held(
        &env,
        &server_cli,
        &path,
        HELD_ID,
        KnowledgeDecision::Approve,
        Some(REVISION),
        None,
    ))
    .unwrap();

    assert_eq!(
        approved,
        KnowledgeDecided::Approved {
            doc_id: 12.into(),
            status: KnowledgeApproval::Added,
            title: "Retry policy".to_owned(),
            chunks: Some(3),
            embedding: Some(KnowledgeEmbedding::Pending),
        }
    );
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["held", "approve", "--json", HELD_ID, "--expect", REVISION]
    );

    let rejected = wait(decide_knowledge_held(
        &env,
        &server_cli,
        &path,
        HELD_ID,
        KnowledgeDecision::Reject,
        None,
        None,
    ))
    .unwrap();

    assert_eq!(rejected, KnowledgeDecided::Rejected);
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["held", "reject", "--json", HELD_ID]
    );
}

#[test]
fn an_unchanged_approval_has_no_chunks_to_report() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(
        &["held", "approve"],
        answers(json!({ "approved": {
            "heldId": HELD_ID, "docId": 5, "status": "unchanged", "title": "Retry policy",
            "chunks": null, "embedding": null
        } })),
    );

    let approved = wait(decide_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        HELD_ID,
        KnowledgeDecision::Approve,
        None,
        None,
    ))
    .unwrap();

    assert_eq!(
        approved,
        KnowledgeDecided::Approved {
            doc_id: 5.into(),
            status: KnowledgeApproval::Unchanged,
            title: "Retry policy".to_owned(),
            chunks: None,
            embedding: None,
        }
    );
}

#[test]
fn the_servers_refusals_are_outcomes_with_its_sentence() {
    let (_dir, env) = installed();
    let path = server(&env);

    for (code, expected) in [("notFound", "NotFound"), ("changed", "Changed"), ("refused", "Refused")] {
        let message = format!("the {code} sentence");
        let server_cli = refusal(code, &message);

        let outcome = wait(decide_knowledge_held(
            &env,
            &server_cli,
            &path,
            HELD_ID,
            KnowledgeDecision::Approve,
            Some(REVISION),
            None,
        ))
        .unwrap();

        let (name, text) = match outcome {
            KnowledgeDecided::NotFound { message } => ("NotFound", message),
            KnowledgeDecided::Changed { message } => ("Changed", message),
            KnowledgeDecided::Refused { message } => ("Refused", message),
            other => panic!("not a refusal: {other:?}"),
        };

        assert_eq!((name, text), (expected, message));
    }
}

#[test]
fn a_failure_that_is_no_refusal_is_an_error_and_an_old_server_is_unsupported() {
    let (_dir, env) = installed();
    let path = server(&env);

    let failing = cli(ALL_FEATURES).starting(
        &["held", "approve"],
        exits(1, "", "harness-knowledge: the data folder is unknown\n"),
    );
    let error = wait(decide_knowledge_held(
        &env,
        &failing,
        &path,
        HELD_ID,
        KnowledgeDecision::Approve,
        None,
        None,
    ))
    .expect_err("failed");
    assert!(matches!(error, BusinessError::Process(message) if message.contains("data folder")));

    let old = cli(ALL_FEATURES);
    let error = wait(decide_knowledge_held(
        &env,
        &old,
        &path,
        HELD_ID,
        KnowledgeDecision::Reject,
        None,
        None,
    ))
    .expect_err("usage");
    assert!(matches!(error, BusinessError::Unsupported(_)));
}

#[test]
fn an_id_or_revision_that_is_not_one_runs_nothing() {
    let (_dir, env) = installed();
    let server_cli = queue();
    let path = server(&env);

    for id in [
        "",
        "h-1",
        "../etc",
        "A0B1C2D3-E4F5-4A6B-8C7D-9E0F1A2B3C4D",
        &"x".repeat(129),
    ] {
        assert!(matches!(
            wait(get_knowledge_held(&env, &server_cli, &path, id)),
            Err(BusinessError::InvalidInput(_))
        ));
    }

    for revision in ["", "abc", "--json", &"G".repeat(64)] {
        assert!(matches!(
            wait(decide_knowledge_held(
                &env,
                &server_cli,
                &path,
                HELD_ID,
                KnowledgeDecision::Approve,
                Some(revision),
                None,
            )),
            Err(BusinessError::InvalidInput(_))
        ));
    }

    assert!(server_cli.commands().is_empty());
}

#[test]
fn the_counts_follow_the_generation_and_the_queue_when_there_is_one() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let stats = json!({ "documents": 13, "generation": 17, "chunks": 1, "embeddedChunks": 1, "bytes": 1,
                        "globalDocuments": 0, "repositories": 0, "schema": 1, "model": null });
    let with_queue = queue().starting(&["stats"], answers(stats.clone()));

    let pulse = wait(knowledge_pulse(&env, &with_queue, &probe)).expect("answers");
    assert_eq!((pulse.documents, pulse.generation, pulse.held), (13, 17, Some(2)));

    let without_queue = cli("docs\n").starting(&["stats"], answers(stats));
    let pulse = wait(knowledge_pulse(&env, &without_queue, &KnowledgeFeatures::new())).expect("answers");
    assert_eq!(pulse.held, None);

    // A server that counts the proposals itself is asked once, and never for the queue.
    let counting = queue().starting(
        &["stats"],
        answers(
            json!({ "documents": 13, "generation": 17, "chunks": 1, "embeddedChunks": 1, "bytes": 1,
                        "globalDocuments": 0, "repositories": 0, "schema": 1, "model": null, "held": 5 }),
        ),
    );
    let pulse = wait(knowledge_pulse(&env, &counting, &probe)).expect("answers");
    assert_eq!(pulse.held, Some(5));
    assert!(
        counting.commands().iter().all(|command| command[0] == "stats"),
        "{:?}",
        counting.commands()
    );

    let broken = cli(ALL_FEATURES).starting(&["stats"], exits(1, "", "harness-knowledge: locked\n"));
    assert!(wait(knowledge_pulse(&env, &broken, &KnowledgeFeatures::new())).is_none());
}

fn split_stats(local_generation: i64, local_documents: u32, remote_offline: bool) -> serde_json::Value {
    json!({ "documents": 4, "generation": 9, "chunks": 1, "embeddedChunks": 1, "bytes": 1,
            "globalDocuments": 0, "repositories": 0, "schema": 1, "model": null, "held": 0,
            "offline": remote_offline,
            "places": {
                "remote": { "documents": if remote_offline { 0 } else { 4 }, "generation": 9, "offline": remote_offline },
                "local": { "documents": local_documents, "generation": local_generation }
            } })
}

#[test]
fn with_both_places_the_pulse_sums_them_and_moves_when_only_the_local_one_changes() {
    let (_dir, env) = installed();
    let pulse_of = |stats| {
        let server_cli = cli(ALL_FEATURES).starting(&["stats"], answers(stats));

        wait(knowledge_pulse(&env, &server_cli, &KnowledgeFeatures::new())).expect("answers")
    };

    let before = pulse_of(split_stats(3, 2, false));
    assert_eq!(before.documents, 6, "the remote documents and the local ones");

    // Only this computer's store changed: the top-level numbers (the remote database's) did not.
    let imported = pulse_of(split_stats(4, 3, false));
    assert_eq!(imported.documents, 7);
    assert_ne!(
        imported.generation, before.generation,
        "a local import must reach the view"
    );
    assert_eq!(
        pulse_of(split_stats(3, 2, false)),
        before,
        "the same stores give the same pulse"
    );

    // A remote database that cannot be reached counts for nothing: the local documents alone.
    let offline = pulse_of(split_stats(3, 2, true));
    assert_eq!((offline.documents, offline.offline), (2, true));
}

#[test]
fn an_approval_place_is_dropped_for_a_server_that_cannot_route() {
    use harness_business::ecosystem::approval_place;
    use harness_business::model::KnowledgePlace;

    let (_dir, env) = installed();
    let routing = cli(ALL_FEATURES);
    let plain = cli("docs\nheld\n");

    let kept = wait(approval_place(
        &env,
        &routing,
        &KnowledgeFeatures::new(),
        Some(KnowledgePlace::Local),
    ));
    assert_eq!(kept, Some(KnowledgePlace::Local));

    let dropped = wait(approval_place(
        &env,
        &plain,
        &KnowledgeFeatures::new(),
        Some(KnowledgePlace::Local),
    ));
    assert_eq!(dropped, None);
}

#[test]
fn an_approval_names_the_place_when_given_and_a_local_doc_id_stays_a_string() {
    use harness_business::model::{KnowledgeId, KnowledgePlace};

    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(
        &["held", "approve"],
        answers(json!({
            "approved": { "heldId": HELD_ID, "docId": "local:12", "status": "added", "title": "Retry policy", "chunks": 3, "embedding": "done" }
        })),
    );

    let approved = wait(decide_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        HELD_ID,
        KnowledgeDecision::Approve,
        Some(REVISION),
        Some(KnowledgePlace::Local),
    ))
    .unwrap();

    assert!(
        matches!(approved, KnowledgeDecided::Approved { doc_id, .. } if doc_id == KnowledgeId::Local("local:12".to_owned()))
    );
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "held", "approve", "--json", HELD_ID, "--expect", REVISION, "--place", "local"
        ]
    );
}
