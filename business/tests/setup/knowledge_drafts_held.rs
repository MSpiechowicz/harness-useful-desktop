//! The proposals of agents under drafts: who they are written for, a request for changes, a rejection
//! with a reason and an approval that is pending or a draft.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeFeatures, ListWindow, reject_knowledge_held, request_changes_knowledge_held,
};
use harness_business::model::KnowledgeDecided;
use knowledge_stand_in::{ALL_FEATURES, HELD_ID, REVISION, cli, held_item, installed, refusing, server};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{answers, wait};

#[test]
fn a_proposal_carries_who_it_is_for_its_state_and_the_comment_asked_for() {
    let (_dir, env) = installed();
    let window = ListWindow::new(None, None, None, None).unwrap();
    let mut asked = held_item(HELD_ID, "Retry policy", "preview", "Retry three times.");
    asked["addedBy"] = json!("  Ada Lovelace ");
    asked["status"] = json!("changesRequested");
    asked["comment"] = json!("Add the backoff table");
    let plain = held_item(knowledge_stand_in::HELD_ID_2, "Runbook", "preview", "Page the on-call.");
    let by_object = {
        let mut item = held_item("b0b1c2d3-e4f5-4a6b-8c7d-9e0f1a2b3c4d", "Other", "preview", "x");
        item["addedBy"] = json!({ "name": "Grace", "via": "codex" });
        item
    };
    let server_cli =
        cli(ALL_FEATURES).starting(&["held", "list"], answers(json!({ "held": [asked, plain, by_object] })));

    let page = knowledge_stand_in::queue_page(&env, &server_cli, &KnowledgeFeatures::new(), &window);

    let by: Vec<Option<&str>> = page.items.iter().map(|item| item.added_by.as_deref()).collect();
    assert_eq!(by, [Some("Ada Lovelace"), None, Some("Grace")]);
    let first = &page.items[0];
    assert_eq!(
        first.status,
        harness_business::model::KnowledgeHeldStatus::ChangesRequested
    );
    assert_eq!(first.comment.as_deref(), Some("Add the backoff table"));
    assert_eq!(
        page.items[1].status,
        harness_business::model::KnowledgeHeldStatus::Waiting,
        "waiting without a status"
    );
    assert_eq!(page.items[1].comment, None);
}

#[test]
fn a_request_for_changes_passes_the_revision_and_comment_and_a_refusal_is_an_outcome() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(
        &["held", "request-changes"],
        answers(json!({ "changesRequested": HELD_ID })),
    );

    let decided = wait(request_changes_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        HELD_ID,
        Some(REVISION),
        " Add the table ",
    ))
    .unwrap();

    assert_eq!(decided, KnowledgeDecided::ChangesRequested);
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "held",
            "request-changes",
            "--json",
            HELD_ID,
            "--expect",
            REVISION,
            "--comment",
            "Add the table"
        ]
    );

    let before = server_cli.commands().len();
    let empty = wait(request_changes_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        HELD_ID,
        None,
        "  ",
    ))
    .unwrap();
    assert!(matches!(empty, KnowledgeDecided::CommentRequired { .. }));
    assert_eq!(server_cli.commands().len(), before, "an empty comment is not sent");

    for (code, expected) in [
        ("commentRequired", "commentRequired"),
        ("changed", "changed"),
        ("notFound", "notFound"),
        ("wrongState", "refused"),
    ] {
        let refused = cli(ALL_FEATURES).on(|args| args.starts_with(&["held".to_owned()]), refusing(code, "no"));
        let outcome = wait(request_changes_knowledge_held(
            &env,
            &refused,
            &server(&env),
            HELD_ID,
            None,
            "Please",
        ))
        .unwrap();
        let json = serde_json::to_value(&outcome).unwrap();

        assert_eq!(json["type"], expected, "{code}");
    }
}

#[test]
fn a_rejection_may_tell_the_agent_why_and_an_approval_may_be_pending_or_a_draft() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES)
        .starting(&["held", "reject"], answers(json!({ "rejected": HELD_ID })))
        .starting(
            &["held", "approve"],
            answers(json!({ "approved": {
                "heldId": HELD_ID, "docId": 21, "status": "pending", "title": "Retry policy", "chunks": 3, "embedding": "done", "draft": true
            } })),
        );

    let rejected = wait(reject_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        HELD_ID,
        Some(REVISION),
        "Out of date",
    ))
    .unwrap();
    assert_eq!(rejected, KnowledgeDecided::Rejected);
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "held",
            "reject",
            "--json",
            HELD_ID,
            "--expect",
            REVISION,
            "--comment",
            "Out of date"
        ]
    );

    let approved = wait(harness_business::ecosystem::decide_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        HELD_ID,
        harness_business::model::KnowledgeDecision::Approve,
        None,
        None,
    ))
    .unwrap();
    assert!(matches!(
        approved,
        KnowledgeDecided::Approved {
            status: harness_business::model::KnowledgeApproval::Pending,
            draft: true,
            ..
        }
    ));

    assert!(matches!(
        wait(reject_knowledge_held(
            &env,
            &server_cli,
            &server(&env),
            HELD_ID,
            None,
            &"x".repeat(501)
        )),
        Err(BusinessError::InvalidInput(_))
    ));
}
