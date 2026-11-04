//! Drafts and team approval (`docs-drafts`, `docs-approval`): the transitions and the refusals the
//! server explains with a code, a server before drafts, and a review with line comments.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeFeatures, approval_server, drafts_server, review_doc, review_server, transition_doc,
};
use harness_business::model::{
    CommentSide, KnowledgeDocStatus, KnowledgeLineComment, KnowledgeOutcome, KnowledgeRefusalCode,
    KnowledgeReviewSubmit, KnowledgeTransition, KnowledgeVerdict,
};
use knowledge_stand_in::review::{ID, doc_id, docs_answering};
use knowledge_stand_in::{ALL_FEATURES, BEFORE_APPROVAL, BEFORE_DRAFTS, cli, installed, refusing, server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{Cli, answers, wait};

/// What `transition_doc` answers for `action` at revision 4 on a server that runs it.
fn transition(
    cli: &Cli,
    action: KnowledgeTransition,
    comment: Option<&str>,
) -> Result<KnowledgeOutcome<harness_business::model::KnowledgeTransitioned>, BusinessError> {
    let (_dir, env) = installed();

    wait(transition_doc(&env, cli, &server(&env), &doc_id(), action, 4, comment))
}

fn done(
    outcome: KnowledgeOutcome<harness_business::model::KnowledgeTransitioned>,
) -> harness_business::model::KnowledgeTransitioned {
    match outcome {
        KnowledgeOutcome::Done { value } => value,
        KnowledgeOutcome::Refused { code, message } => panic!("refused {code:?}: {message}"),
    }
}

#[test]
fn a_transition_names_the_revision_and_answers_the_state_it_made() {
    let reply = |status: &str, promoted: bool| {
        answers(json!({ "id": ID, "status": status, "revision": 5, "promoted": promoted }))
    };

    for (action, verb, status, promoted) in [
        (KnowledgeTransition::Ask, "ask", "waiting", false),
        (KnowledgeTransition::Approve, "approve", "live", true),
        (KnowledgeTransition::GoLive, "go-live", "live", false),
        (KnowledgeTransition::TakeBack, "take-back", "draft", false),
    ] {
        let server_cli = docs_answering(ALL_FEATURES, &["docs", verb], reply(status, promoted));
        let moved = done(transition(&server_cli, action, Some("ignored")).unwrap());

        assert_eq!(moved.id, doc_id());
        assert_eq!((moved.revision, moved.promoted), (5, promoted));
        assert_eq!(
            server_cli.commands().pop().unwrap(),
            ["docs", verb, "--json", "21", "--expect", "4"],
            "only a request for changes carries a comment"
        );
    }

    let server_cli = docs_answering(
        ALL_FEATURES,
        &["docs", "request-changes"],
        reply("changesRequested", false),
    );
    let moved = done(
        transition(
            &server_cli,
            KnowledgeTransition::RequestChanges,
            Some("  Add the table. "),
        )
        .unwrap(),
    );

    assert_eq!(moved.status, KnowledgeDocStatus::ChangesRequested);
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "docs",
            "request-changes",
            "--json",
            "21",
            "--expect",
            "4",
            "--comment",
            "Add the table."
        ]
    );
}

#[test]
fn only_asking_approving_and_requesting_changes_need_team_approval_on_the_server() {
    use harness_business::ecosystem::transition_needs_approval;

    for (action, needs) in [
        (KnowledgeTransition::Ask, true),
        (KnowledgeTransition::RequestChanges, true),
        (KnowledgeTransition::Approve, true),
        (KnowledgeTransition::GoLive, false),
        (KnowledgeTransition::TakeBack, false),
    ] {
        assert_eq!(transition_needs_approval(action), needs, "{action:?}");
    }
}

#[test]
fn requesting_changes_without_a_comment_is_refused_before_the_server_is_asked() {
    let server_cli = cli(ALL_FEATURES);

    for comment in [None, Some("  ")] {
        let outcome = transition(&server_cli, KnowledgeTransition::RequestChanges, comment).unwrap();

        assert!(
            matches!(
                outcome,
                KnowledgeOutcome::Refused {
                    code: KnowledgeRefusalCode::CommentRequired,
                    ..
                }
            ),
            "{outcome:?}"
        );
    }

    let long = "x".repeat(501);
    assert!(matches!(
        transition(&server_cli, KnowledgeTransition::RequestChanges, Some(&long)),
        Err(BusinessError::InvalidInput(_))
    ));
    assert!(
        server_cli.commands().is_empty(),
        "nothing ran: {:?}",
        server_cli.commands()
    );
}

#[test]
fn every_refusal_code_is_an_outcome_with_the_servers_sentence() {
    for (code, expected) in [
        ("notFound", KnowledgeRefusalCode::NotFound),
        ("changed", KnowledgeRefusalCode::Changed),
        ("wrongState", KnowledgeRefusalCode::WrongState),
        ("approvalOn", KnowledgeRefusalCode::ApprovalOn),
        ("approvalOff", KnowledgeRefusalCode::ApprovalOff),
        ("selfApproval", KnowledgeRefusalCode::SelfApproval),
        ("notAuthor", KnowledgeRefusalCode::NotAuthor),
        ("noName", KnowledgeRefusalCode::NoName),
        ("readOnly", KnowledgeRefusalCode::ReadOnly),
        ("commentRequired", KnowledgeRefusalCode::CommentRequired),
        ("sinceUnavailable", KnowledgeRefusalCode::SinceUnavailable),
        ("refused", KnowledgeRefusalCode::Refused),
    ] {
        let server_cli = docs_answering(ALL_FEATURES, &["docs", "approve"], refusing(code, "it is not so"));
        let outcome = transition(&server_cli, KnowledgeTransition::Approve, None).unwrap();

        assert_eq!(
            outcome,
            KnowledgeOutcome::Refused {
                code: expected,
                message: "it is not so".to_owned()
            },
            "{code}"
        );
    }
}

#[test]
fn an_unreachable_database_a_usage_error_and_an_unknown_refusal_are_errors() {
    let down = docs_answering(
        ALL_FEATURES,
        &["docs", "approve"],
        refusing("unreachable", "remote knowledge is unreachable right now"),
    );
    assert!(matches!(
        transition(&down, KnowledgeTransition::Approve, None),
        Err(BusinessError::Unreachable(message)) if message.contains("unreachable")
    ));

    let old = cli(ALL_FEATURES);
    assert!(matches!(
        transition(&old, KnowledgeTransition::Approve, None),
        Err(BusinessError::Unsupported(message)) if message.contains("predates `docs approve`")
    ));

    let odd = docs_answering(ALL_FEATURES, &["docs", "approve"], refusing("somethingNew", "later"));
    assert!(matches!(
        transition(&odd, KnowledgeTransition::Approve, None),
        Err(BusinessError::Process(_))
    ));
}

#[test]
fn approving_and_requesting_changes_in_a_review_need_team_approval_on_the_server() {
    // A server with drafts but without team approval.
    let drafts_only = BEFORE_APPROVAL;

    for (verdict, needs) in [
        (KnowledgeVerdict::Comment, false),
        (KnowledgeVerdict::Approve, true),
        (KnowledgeVerdict::RequestChanges, true),
    ] {
        let (_dir, env) = installed();
        let server_cli = cli(drafts_only);
        let reached = wait(review_server(&env, &server_cli, &KnowledgeFeatures::new(), verdict));

        if needs {
            assert!(
                matches!(&reached, Err(BusinessError::Unsupported(message)) if message.contains("team approval")),
                "{verdict:?}"
            );
        } else {
            assert!(reached.is_ok(), "{verdict:?}");
        }

        let with_approval = wait(review_server(
            &env,
            &cli(ALL_FEATURES),
            &KnowledgeFeatures::new(),
            verdict,
        ));
        assert!(with_approval.is_ok(), "{verdict:?}");
    }
}

#[test]
fn a_server_before_drafts_is_outdated_for_every_review_command() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let old = cli(BEFORE_DRAFTS);

    let error = wait(drafts_server(&env, &old, &probe)).expect_err("no drafts");
    assert!(matches!(error, BusinessError::Unsupported(message) if message.contains("drafts")));

    let error = wait(approval_server(&env, &old, &probe)).expect_err("no approval");
    assert!(matches!(error, BusinessError::Unsupported(message) if message.contains("team approval")));

    assert!(wait(drafts_server(&env, &cli(ALL_FEATURES), &KnowledgeFeatures::new())).is_ok());
    assert_eq!(
        old.commands(),
        [["features"]],
        "the answer is cached and nothing else ran"
    );
}

/// The comments file the review wrote, read while the server runs: its path and text.
type Seen = Arc<Mutex<Option<(String, String)>>>;

fn reviewing(reply: Value) -> (Cli, Seen) {
    let seen: Seen = Arc::default();
    let record = Arc::clone(&seen);

    let server_cli = cli(ALL_FEATURES).answering(move |args| {
        if args.starts_with(&["docs".to_owned(), "review".to_owned()]) {
            let file = args
                .iter()
                .position(|arg| arg == "--comments")
                .map(|at| args[at + 1].clone());
            *record.lock().unwrap() =
                file.map(|path| (path.clone(), std::fs::read_to_string(path).unwrap_or_default()));

            Some(answers(reply.clone()))
        } else {
            None
        }
    });

    (server_cli, seen)
}

fn line_comment(section: &str, side: CommentSide, from: u32, to: u32, body: &str) -> KnowledgeLineComment {
    KnowledgeLineComment {
        section: section.to_owned(),
        occurrence: 1,
        side,
        from,
        to,
        body: body.to_owned(),
    }
}

fn submit(
    cli: &Cli,
    verdict: KnowledgeVerdict,
    body: Option<&str>,
    comments: Vec<KnowledgeLineComment>,
) -> Result<KnowledgeOutcome<harness_business::model::KnowledgeTransitioned>, BusinessError> {
    let (_dir, env) = installed();
    let review = KnowledgeReviewSubmit {
        verdict,
        body: body.map(str::to_owned),
        comments,
    };

    wait(review_doc(&env, cli, &server(&env), &doc_id(), 4, &review))
}

#[test]
fn a_review_sends_its_verdict_body_and_line_comments_in_a_file_that_is_removed_after() {
    let (server_cli, seen) =
        reviewing(json!({ "id": ID, "status": "changesRequested", "revision": 5, "promoted": false }));
    let comments = vec![
        line_comment("Guide > Install", CommentSide::New, 2, 3, "Why this flag?"),
        line_comment("Reference", CommentSide::Old, 7, 7, "Keep the old wording"),
        KnowledgeLineComment {
            occurrence: 2,
            ..line_comment("Install", CommentSide::New, 1, 1, " \n Padded body \n ")
        },
    ];

    let moved = done(
        submit(
            &server_cli,
            KnowledgeVerdict::RequestChanges,
            Some(" Please fix the install. "),
            comments,
        )
        .unwrap(),
    );

    assert_eq!(moved.status, KnowledgeDocStatus::ChangesRequested);

    let command = server_cli.commands().pop().unwrap();
    assert_eq!(
        command[..10],
        [
            "docs",
            "review",
            "--json",
            "21",
            "--expect",
            "4",
            "--verdict",
            "requestChanges",
            "--body",
            "Please fix the install."
        ]
    );
    assert_eq!(command[10], "--comments");

    let (path, text) = seen.lock().unwrap().clone().expect("a file was passed");
    assert_eq!(command[11], path);
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap(),
        json!([
            { "section": "Guide > Install", "occurrence": 1, "side": "new", "from": 2, "to": 3, "body": "Why this flag?" },
            { "section": "Reference", "occurrence": 1, "side": "old", "from": 7, "to": 7, "body": "Keep the old wording" },
            { "section": "Install", "occurrence": 2, "side": "new", "from": 1, "to": 1, "body": "Padded body" }
        ])
    );
    assert!(
        !Path::new(&path).exists(),
        "the file is removed once the server answered"
    );
}

#[test]
fn a_comment_only_review_without_line_comments_passes_no_file() {
    let (server_cli, seen) = reviewing(json!({ "id": ID, "status": "waiting", "revision": 4 }));

    let moved = done(
        submit(
            &server_cli,
            KnowledgeVerdict::Comment,
            Some("Looks fine so far."),
            Vec::new(),
        )
        .unwrap(),
    );

    assert_eq!(
        (moved.status, moved.promoted),
        (KnowledgeDocStatus::Waiting, false),
        "promoted is absent"
    );
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "docs",
            "review",
            "--json",
            "21",
            "--expect",
            "4",
            "--verdict",
            "comment",
            "--body",
            "Looks fine so far."
        ]
    );
    assert!(seen.lock().unwrap().is_none());
}

#[test]
fn a_review_the_server_would_refuse_is_refused_before_it_runs() {
    let (server_cli, _seen) = reviewing(json!({}));
    let fine = || line_comment("Install", CommentSide::New, 1, 1, "ok");

    let bad = [
        vec![line_comment("Install", CommentSide::New, 3, 2, "backwards")],
        vec![line_comment("Install", CommentSide::New, 0, 1, "from zero")],
        vec![line_comment("Install", CommentSide::New, 1, 1_000_001, "too far")],
        vec![line_comment("", CommentSide::New, 1, 1, "no section")],
        vec![KnowledgeLineComment {
            occurrence: 0,
            ..fine()
        }],
        vec![line_comment("Install", CommentSide::New, 1, 1, "  ")],
        vec![line_comment("Install", CommentSide::New, 1, 1, &"x".repeat(4001))],
        (0..201).map(|_| fine()).collect(),
    ];

    for comments in bad {
        assert!(
            matches!(
                submit(&server_cli, KnowledgeVerdict::Comment, None, comments),
                Err(BusinessError::InvalidInput(_))
            ),
            "refused"
        );
    }

    assert!(server_cli.commands().is_empty());
}

#[test]
fn a_review_refused_for_the_author_is_an_outcome() {
    let server_cli = docs_answering(
        ALL_FEATURES,
        &["docs", "review"],
        refusing("selfApproval", "you wrote it"),
    );

    let outcome = submit(&server_cli, KnowledgeVerdict::Approve, None, Vec::new()).unwrap();

    assert!(matches!(
        outcome,
        KnowledgeOutcome::Refused {
            code: KnowledgeRefusalCode::SelfApproval,
            ..
        }
    ));
}
