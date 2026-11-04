//! Deleting a comment of a review and reading the lines a comment quotes.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{delete_comment, doc_comments};
use harness_business::model::{KnowledgeId, KnowledgeOutcome, KnowledgeQuoteKind, KnowledgeRefusalCode};
use knowledge_stand_in::review::doc_id;
use knowledge_stand_in::{ALL_FEATURES, cli, installed, refusing, server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{answers, wait};

fn comment_json(id: i64, quote: Value) -> Value {
    json!({
        "id": id, "reviewId": 8, "section": "Install", "occurrence": 1, "side": "new", "from": 2, "to": 3,
        "body": "Why this flag?", "by": "Grace", "at": "2026-10-04T10:00:00Z", "parentId": null,
        "resolvedAt": null, "resolvedBy": null, "revision": 3, "outdated": false, "quote": quote
    })
}

#[test]
fn a_comment_is_deleted_with_the_ids_the_server_says_are_gone() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(
        &["docs", "comment", "delete"],
        answers(json!({ "deleted": [5, "local:6"] })),
    );

    let outcome = wait(delete_comment(&env, &server_cli, &server(&env), &doc_id(), &5.into())).unwrap();

    let KnowledgeOutcome::Done { value } = outcome else {
        panic!("refused");
    };
    assert_eq!(
        value.deleted,
        [KnowledgeId::Remote(5), KnowledgeId::Local("local:6".to_owned())]
    );
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["docs", "comment", "delete", "--json", "21", "5"]
    );
}

#[test]
fn a_refused_delete_is_an_answer_and_a_bad_comment_id_never_reaches_the_server() {
    let (_dir, env) = installed();

    for (code, expected) in [
        ("notAuthor", KnowledgeRefusalCode::NotAuthor),
        ("notFound", KnowledgeRefusalCode::NotFound),
        ("noName", KnowledgeRefusalCode::NoName),
        ("readOnly", KnowledgeRefusalCode::ReadOnly),
    ] {
        let server_cli = cli(ALL_FEATURES).starting(&["docs", "comment", "delete"], refusing(code, "no"));
        let outcome = wait(delete_comment(&env, &server_cli, &server(&env), &doc_id(), &5.into())).unwrap();

        assert!(
            matches!(outcome, KnowledgeOutcome::Refused { code, .. } if code == expected),
            "{code}"
        );
    }

    let server_cli = cli(ALL_FEATURES).starting(&["docs", "comment", "delete"], answers(json!({ "deleted": [] })));

    for bad in ["--json", "-h", "5", "local:0"] {
        let id = KnowledgeId::Local(bad.to_owned());

        assert!(
            matches!(
                wait(delete_comment(&env, &server_cli, &server(&env), &doc_id(), &id)),
                Err(BusinessError::InvalidInput(_))
            ),
            "{bad}"
        );
    }

    assert_eq!(server_cli.commands().len(), 0, "no bad id was sent");
}

#[test]
fn a_comment_carries_the_lines_it_quotes_and_an_older_server_has_none() {
    let (_dir, env) = installed();
    let quote = json!([
        { "line": 2, "kind": "added", "text": "Set the retry limit to 5." },
        { "line": 3, "kind": "same", "text": "" }
    ]);
    let mut older = comment_json(6, Value::Null);
    older.as_object_mut().unwrap().remove("quote");
    let server_cli = cli(ALL_FEATURES).starting(
        &["docs", "comments"],
        answers(json!({ "comments": [comment_json(5, quote), comment_json(7, Value::Null), older] })),
    );

    let comments = wait(doc_comments(&env, &server_cli, &server(&env), &doc_id())).unwrap();
    let lines = comments[0].quote.as_ref().expect("a quote");

    assert_eq!(
        lines
            .iter()
            .map(|line| (line.line, line.kind, line.text.as_str()))
            .collect::<Vec<_>>(),
        [
            (2, KnowledgeQuoteKind::Added, "Set the retry limit to 5."),
            (3, KnowledgeQuoteKind::Same, "")
        ]
    );
    assert_eq!(comments[1].quote, None, "an overall comment or a reply quotes nothing");
    assert_eq!(comments[2].quote, None, "a server before quotes prints none");
}
