//! Drafts in the lists and counts of the app (`docs list --status`, `stats`, the live pulse). A server before
//! drafts still parses. The upgrade is in `knowledge_drafts_upgrade`.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::ecosystem::{KnowledgeFeatures, ListWindow, get_doc, knowledge_pulse, list_docs};
use harness_business::model::{
    KnowledgeDocStatus, KnowledgeListFilter, KnowledgePulse, KnowledgeStats, KnowledgeStatusFilter,
};
use knowledge_stand_in::review::{doc_id, docs_answering};
use knowledge_stand_in::{
    ALL_FEATURES, BEFORE_DRAFTS, cli, drafts_stats, installed, pending_summary, reviewed_document, server,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{Cli, answers, wait};

fn page_of(items: Vec<Value>) -> Value {
    let total = items.len();

    json!({ "items": items, "total": total })
}

fn list(cli: &Cli, filter: &KnowledgeListFilter) -> harness_business::model::KnowledgePage {
    let (_dir, env) = installed();
    let window = ListWindow::new(None, None, None, None).unwrap();

    wait(list_docs(&env, cli, &KnowledgeFeatures::new(), filter, &window)).unwrap()
}

#[test]
fn a_list_asks_for_a_state_and_reads_the_state_revision_and_pending_version_of_each_row() {
    let mut live = reviewed_document(5, "Billing", "live", 2);
    live["pending"] = pending_summary(21, "waiting", 4);
    let mut draft = reviewed_document(6, "Draft guide", "changesRequested", 7);
    draft["lastChange"] = json!({ "by": "Grace", "at": "2026-10-04T10:00:00Z", "comment": "Add the table" });
    let server_cli = docs_answering(ALL_FEATURES, &["docs", "list"], answers(page_of(vec![live, draft])));

    for (filter, word) in [
        (KnowledgeStatusFilter::Live, "live"),
        (KnowledgeStatusFilter::Draft, "draft"),
        (KnowledgeStatusFilter::Waiting, "waiting"),
        (KnowledgeStatusFilter::ChangesRequested, "changesRequested"),
        (KnowledgeStatusFilter::Pending, "pending"),
    ] {
        list(
            &server_cli,
            &KnowledgeListFilter {
                status: Some(filter),
                ..Default::default()
            },
        );

        let command = server_cli.commands().pop().unwrap();
        let at = command
            .iter()
            .position(|arg| arg == "--status")
            .expect("a status was asked");
        assert_eq!(command[at + 1], word);
    }

    let page = list(&server_cli, &KnowledgeListFilter::default());
    assert!(
        !server_cli.commands().pop().unwrap().contains(&"--status".to_owned()),
        "every row unless asked"
    );

    let [billing, guide] = &page.items[..] else {
        panic!("two rows");
    };
    assert_eq!((billing.status, billing.revision), (KnowledgeDocStatus::Live, 2));
    let pending = billing.pending.as_ref().expect("a pending version");
    assert_eq!(
        (pending.id.clone(), pending.status, pending.revision),
        (21.into(), KnowledgeDocStatus::Waiting, 4)
    );
    assert_eq!(
        pending.updated_by.as_ref().map(|author| author.name.as_str()),
        Some("Grace Hopper")
    );
    assert_eq!(
        (guide.status, guide.revision),
        (KnowledgeDocStatus::ChangesRequested, 7)
    );
    assert_eq!(
        guide.last_change.as_ref().and_then(|change| change.comment.as_deref()),
        Some("Add the table")
    );
}

#[test]
fn a_pending_row_names_its_live_document_and_the_older_name_of_the_last_request_is_read() {
    let (_dir, env) = installed();
    let mut pending = reviewed_document(21, "Billing", "waiting", 4);
    pending["pendingOf"] = json!(5);
    pending["lastSentBack"] = json!({ "by": null, "at": "2026-10-03T10:00:00Z", "comment": "Again" });
    let server_cli = docs_answering(
        ALL_FEATURES,
        &["docs", "get"],
        answers(json!({ "document": pending, "sections": [] })),
    );

    let detail = wait(get_doc(&env, &server_cli, &server(&env), &doc_id(), None)).unwrap();

    assert_eq!(detail.document.pending_of, Some(5.into()));
    assert_eq!(detail.document.status, KnowledgeDocStatus::Waiting);
    assert_eq!(
        detail.document.last_change.map(|change| change.comment),
        Some(Some("Again".to_owned()))
    );
}

fn names(authors: Option<&Vec<String>>) -> Option<Vec<&str>> {
    authors.map(|names| names.iter().map(String::as_str).collect())
}

#[test]
fn the_authors_of_a_version_are_read_cleaned_and_absent_from_a_server_that_does_not_name_them() {
    let (_dir, env) = installed();
    let mut waiting = reviewed_document(21, "Billing", "waiting", 4);
    waiting["authors"] = json!(["  Ada Lovelace ", "Grace\u{202e} Hopper", "", 7, null, "\u{200b}"]);
    let mut live = reviewed_document(5, "Billing", "live", 2);
    live["pending"] = pending_summary(21, "waiting", 4);
    live["pending"]["authors"] = json!(["Ada Lovelace", "Bob"]);
    let mut odd = reviewed_document(6, "Odd", "draft", 1);
    odd["authors"] = json!("Ada");

    for (document, expected_authors, expected_pending) in [
        (waiting, Some(vec!["Ada Lovelace", "Grace Hopper"]), None),
        (live, None, Some(vec!["Ada Lovelace", "Bob"])),
        (reviewed_document(7, "Old server", "draft", 1), None, None),
        (odd, None, None),
    ] {
        let server_cli = docs_answering(
            ALL_FEATURES,
            &["docs", "get"],
            answers(json!({ "document": document, "sections": [] })),
        );
        let detail = wait(get_doc(&env, &server_cli, &server(&env), &doc_id(), None))
            .unwrap()
            .document;

        assert_eq!(names(detail.authors.as_ref()), expected_authors);
        assert_eq!(
            names(detail.pending.as_ref().and_then(|pending| pending.authors.as_ref())),
            expected_pending
        );
    }
}

#[test]
fn a_server_before_drafts_still_parses_every_row_as_live_and_is_not_asked_for_a_state() {
    let server_cli = docs_answering(
        BEFORE_DRAFTS,
        &["docs", "list"],
        answers(page_of(vec![knowledge_stand_in::document(5, "Billing")])),
    );

    let page = list(
        &server_cli,
        &KnowledgeListFilter {
            status: Some(KnowledgeStatusFilter::Waiting),
            ..Default::default()
        },
    );

    assert_eq!(page.items.len(), 1);
    let document = &page.items[0];
    assert_eq!((document.status, document.revision), (KnowledgeDocStatus::Live, 0));
    assert!(document.pending.is_none() && document.pending_of.is_none() && document.last_change.is_none());
    assert!(
        !server_cli.commands().pop().unwrap().contains(&"--status".to_owned()),
        "an older server refuses it"
    );
}

#[test]
fn the_counts_of_drafts_and_waiting_documents_are_read_from_stats() {
    let stats: KnowledgeStats = serde_json::from_value(drafts_stats()).unwrap();

    assert_eq!(
        (
            stats.drafts,
            stats.waiting,
            stats.waiting_for_you,
            stats.pending,
            stats.approval
        ),
        (Some(2), Some(3), Some(2), Some(1), Some(true))
    );

    let old = json!({ "documents": 1, "chunks": 1, "embeddedChunks": 1, "bytes": 1, "globalDocuments": 0, "repositories": 0,
                      "generation": 1, "schema": 1, "model": null });
    let stats: KnowledgeStats = serde_json::from_value(old).unwrap();
    assert_eq!(
        (
            stats.drafts,
            stats.waiting,
            stats.waiting_for_you,
            stats.pending,
            stats.approval
        ),
        (None, None, None, None, None)
    );
}

/// The pulse a server answering `stats` with `stats` gives.
fn pulse_answering(env: &harness_business::env::Env, stats: Value) -> KnowledgePulse {
    let server_cli = cli(ALL_FEATURES).starting(&["stats"], answers(stats));

    wait(knowledge_pulse(env, &server_cli, &KnowledgeFeatures::new())).expect("answers")
}

#[test]
fn the_pulse_carries_the_waiting_count_and_approval_and_moves_when_only_a_state_changes() {
    let (_dir, env) = installed();
    let pulse_of = |stats: Value| pulse_answering(&env, stats);

    let pulse = pulse_of(drafts_stats());
    assert_eq!(
        (pulse.waiting, pulse.waiting_all, pulse.approval, pulse.held),
        (Some(2), Some(3), Some(true), Some(1)),
        "the badge counts what waits for this person, the total stays available"
    );

    // A server that counts waiting documents but not who could approve them badges nobody.
    let mut unnamed = drafts_stats();
    unnamed.as_object_mut().unwrap().remove("waitingForYou");
    let pulse = pulse_of(unnamed);
    assert_eq!((pulse.waiting, pulse.waiting_all), (Some(0), Some(3)));

    let mut older = drafts_stats();
    older.as_object_mut().unwrap().retain(|key, _| {
        !matches!(
            key.as_str(),
            "waiting" | "waitingForYou" | "approval" | "drafts" | "pending"
        )
    });
    let pulse = pulse_of(older);
    assert_eq!(
        (pulse.waiting, pulse.waiting_all, pulse.approval),
        (None, None, None),
        "a server without drafts says nothing"
    );

    // With both places in use the waiting documents of every place add up, and a state change in
    // either moves the generation the view follows.
    let split = |local_waiting: u32, local_drafts: u32| {
        let mut stats = drafts_stats();
        let stats_object = stats.as_object_mut().unwrap();
        stats_object.remove("waiting");
        stats_object.insert(
            "places".to_owned(),
            json!({
                "remote": { "documents": 10, "generation": 31, "waiting": 3, "drafts": 2, "pending": 1 },
                "local": { "documents": 2, "generation": 5, "waiting": local_waiting, "drafts": local_drafts, "pending": 0 }
            }),
        );

        pulse_of(stats)
    };

    let before = split(1, 0);
    assert_eq!(
        (before.waiting_all, before.waiting, before.documents),
        (Some(4), Some(2), 12)
    );
    assert_eq!(split(1, 0), before);
    assert_ne!(
        split(0, 0).generation,
        before.generation,
        "a document that left the waiting state"
    );
    assert_ne!(split(1, 1).generation, before.generation, "a new draft");
}

#[test]
fn the_pulse_carries_the_name_of_the_person_the_server_reads() {
    let (_dir, env) = installed();
    let pulse_of = |stats: Value| pulse_answering(&env, stats);

    let stats: KnowledgeStats = serde_json::from_value(drafts_stats()).unwrap();
    assert_eq!(
        stats.me.as_ref().and_then(|me| me.name.as_deref()),
        Some("Ada Lovelace")
    );
    assert_eq!(pulse_of(drafts_stats()).me.as_deref(), Some("Ada Lovelace"));

    // A name another program could make misleading is cleaned like an author's.
    let mut hostile = drafts_stats();
    hostile["me"] = json!({ "name": "\u{202E}Ada\u{200B} Lovelace\n" });
    assert_eq!(pulse_of(hostile).me.as_deref(), Some("Ada Lovelace"));

    // No name set, a blank one, and a server that predates it say none.
    for me in [json!({ "name": null }), json!({ "name": "  " }), json!(null)] {
        let mut stats = drafts_stats();
        stats["me"] = me;
        assert_eq!(pulse_of(stats).me, None);
    }

    let mut older = drafts_stats();
    older.as_object_mut().unwrap().remove("me");
    assert_eq!(pulse_of(older).me, None);
}
