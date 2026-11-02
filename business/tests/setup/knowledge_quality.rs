//! What Harness Knowledge says of its documents beyond their text: the usefulness filter and sorts
//! of `docs list` (`docs-useful`), the authors and counts on the rows (`docs-authors`,
//! `docs-useful`), a row of an older server without them, and the remote place's write access in
//! `stats`. Every new argument waits for its feature.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::ecosystem::{KnowledgeFeatures, KnowledgeSupport, ListWindow, get_doc, list_docs, read_stats};
use harness_business::model::{KnowledgeListFilter, KnowledgePage, KnowledgeSort, KnowledgeUsefulFilter, SortOrder};
use knowledge_stand_in::{ALL_FEATURES, cli, document, document_with_quality, installed, server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{Cli, answers, wait};

/// What a server of 1.0 lists.
const BEFORE: &str = "docs\nimport-ndjson\nsearch\nrecall\nheld\nmcp\nrouting\ndoc-matches\noffline-fallback\n";

fn page_cli(features: &str, items: Vec<Value>) -> Cli {
    cli(features).starting(&["docs", "list"], answers(json!({ "items": items, "total": 2 })))
}

fn list_with(server_cli: &Cli, filter: &KnowledgeListFilter, order: Option<SortOrder>) -> Vec<String> {
    let (_dir, env) = installed();
    let window = ListWindow::new(None, None, None, order).unwrap();

    wait(list_docs(&env, server_cli, &KnowledgeFeatures::new(), filter, &window)).unwrap();

    server_cli.commands().pop().unwrap()
}

fn first_page(server_cli: &Cli) -> KnowledgePage {
    let (_dir, env) = installed();
    let window = ListWindow::new(None, None, None, None).unwrap();

    wait(list_docs(
        &env,
        server_cli,
        &KnowledgeFeatures::new(),
        &KnowledgeListFilter::default(),
        &window,
    ))
    .unwrap()
}

fn value_of<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == option)
        .map(|pair| pair[1].as_str())
}

#[test]
fn the_new_features_are_read_apart_and_an_older_server_lists_none() {
    let (_dir, env) = installed();

    let some = wait(KnowledgeFeatures::new().support(&env, &cli("docs\ndocs-similar\nDocs-Authors\n")));
    assert!(some.docs_similar() && some.docs_authors() && !some.docs_useful());

    let all = wait(KnowledgeFeatures::new().support(&env, &cli(ALL_FEATURES)));
    assert!(all.docs_useful() && all.docs_similar() && all.docs_authors());

    let before = wait(KnowledgeFeatures::new().support(&env, &cli(BEFORE)));
    assert!(matches!(before, KnowledgeSupport::Listed(_)));
    assert!(!before.docs_useful() && !before.docs_similar() && !before.docs_authors());
    assert!(before.docs(), "the old features stay");
}

#[test]
fn the_useful_filter_and_sorts_go_to_a_server_that_lists_them() {
    for (sort, name) in [
        (KnowledgeSort::Useful, "useful"),
        (KnowledgeSort::LastUseful, "lastUseful"),
    ] {
        let server_cli = page_cli(ALL_FEATURES, vec![document(7, "guide")]);
        let filter = KnowledgeListFilter {
            sort: Some(sort),
            useful: Some(KnowledgeUsefulFilter::Ever),
            ..KnowledgeListFilter::default()
        };

        let args = list_with(&server_cli, &filter, None);
        assert_eq!(value_of(&args, "--sort"), Some(name));
        assert_eq!(value_of(&args, "--useful"), Some("ever"));
        assert!(
            args.contains(&"--desc".to_owned()),
            "most useful and latest first by default"
        );

        let ascending = list_with(&server_cli, &filter, Some(SortOrder::Asc));
        assert!(!ascending.contains(&"--desc".to_owned()));
    }

    let server_cli = page_cli(ALL_FEATURES, vec![]);
    let never = KnowledgeListFilter {
        useful: Some(KnowledgeUsefulFilter::Never),
        ..KnowledgeListFilter::default()
    };
    let args = list_with(&server_cli, &never, None);

    assert_eq!(value_of(&args, "--useful"), Some("never"));
    assert_eq!(value_of(&args, "--sort"), Some("updated"));
    assert!(!list_with(&server_cli, &KnowledgeListFilter::default(), None).contains(&"--useful".to_owned()));
}

#[test]
fn a_server_without_docs_useful_is_never_asked_for_the_filter_or_the_sorts() {
    for sort in [KnowledgeSort::Useful, KnowledgeSort::LastUseful] {
        let server_cli = page_cli(BEFORE, vec![document(7, "guide")]);
        let filter = KnowledgeListFilter {
            sort: Some(sort),
            useful: Some(KnowledgeUsefulFilter::Never),
            ..KnowledgeListFilter::default()
        };

        let args = list_with(&server_cli, &filter, None);

        assert!(!args.contains(&"--useful".to_owned()), "{args:?}");
        assert_eq!(value_of(&args, "--sort"), Some("updated"), "the default order");
        assert!(args.contains(&"--desc".to_owned()));
    }

    // The sorts the old server knows are kept.
    let server_cli = page_cli(BEFORE, vec![]);
    let title = KnowledgeListFilter {
        sort: Some(KnowledgeSort::Title),
        ..KnowledgeListFilter::default()
    };

    assert_eq!(value_of(&list_with(&server_cli, &title, None), "--sort"), Some("title"));
}

#[test]
fn rows_carry_authors_and_counts_and_rows_of_an_older_server_carry_none() {
    let server_cli = page_cli(
        ALL_FEATURES,
        vec![document_with_quality(7, "guide"), document(8, "api")],
    );
    let page = first_page(&server_cli);

    let rated = &page.items[0];
    assert_eq!(rated.added_by.as_ref().map(|by| by.name.as_str()), Some("Ada Lovelace"));
    assert_eq!(
        rated.added_by.as_ref().and_then(|by| by.via.as_deref()),
        Some("claude-code")
    );
    assert_eq!(
        rated.updated_by.as_ref().map(|by| by.name.as_str()),
        Some("Grace Hopper")
    );
    assert_eq!(rated.updated_by.as_ref().and_then(|by| by.via.as_deref()), None);
    assert_eq!((rated.shown, rated.useful), (Some(6), Some(2)));
    assert_eq!(rated.last_useful_at.as_deref(), Some("2026-10-05T12:00:00Z"));

    let plain = &page.items[1];
    assert_eq!((plain.added_by.as_ref(), plain.updated_by.as_ref()), (None, None));
    assert_eq!(
        (plain.shown, plain.useful, plain.last_useful_at.as_ref()),
        (None, None, None)
    );

    // The wire the view reads leaves unknown values out.
    let wire = serde_json::to_value(plain).unwrap();
    assert_eq!(wire["addedBy"], Value::Null);
}

#[test]
fn an_author_is_cleaned_and_one_that_cannot_be_read_is_none() {
    let mut hostile = document_with_quality(7, "guide");
    hostile["addedBy"] =
        json!({ "name": format!("  Ev\u{202e}il\u{0007}{}  ", "x".repeat(200)), "via": "claude\u{200b}-code" });
    hostile["updatedBy"] = json!({ "name": "\u{202e}\u{0000}", "via": null });

    let mut odd = document_with_quality(8, "api");
    odd["addedBy"] = json!("Ada");
    odd["updatedBy"] = json!({ "via": "codex" });

    let server_cli = page_cli(ALL_FEATURES, vec![hostile, odd]);
    let page = first_page(&server_cli);

    let added = page.items[0].added_by.as_ref().expect("an author");
    assert_eq!(added.name.chars().count(), 80);
    assert!(
        added.name.starts_with("Evil") && !added.name.contains('\u{202e}'),
        "{}",
        added.name
    );
    assert_eq!(added.via.as_deref(), Some("claude-code"));
    assert_eq!(page.items[0].updated_by, None, "nothing readable is left of the name");

    assert_eq!(
        (page.items[1].added_by.as_ref(), page.items[1].updated_by.as_ref()),
        (None, None)
    );
    assert_eq!(page.items[1].useful, Some(2), "the rest of the row is read");
}

#[test]
fn a_document_get_carries_the_same_fields() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(
        &["docs", "get"],
        answers(json!({ "document": document_with_quality(7, "guide"), "sections": [] })),
    );

    let detail = wait(get_doc(&env, &server_cli, &server(&env), &7.into(), None)).unwrap();

    assert_eq!(detail.document.useful, Some(2));
    assert_eq!(
        detail.document.added_by.as_ref().map(|by| by.name.as_str()),
        Some("Ada Lovelace")
    );
}

#[test]
fn remote_writable_is_carried_and_unknown_when_the_server_does_not_say() {
    let (_dir, env) = installed();
    let base = json!({
        "documents": 13, "chunks": 90, "embeddedChunks": 88, "bytes": 1, "globalDocuments": 4,
        "repositories": 1, "generation": 17, "schema": 1, "model": null
    });

    let stats = |extra: Option<Value>| {
        let mut data = base.clone();

        if let Some(extra) = extra {
            data["remoteWritable"] = extra;
        }

        let server_cli = cli(ALL_FEATURES).starting(&["stats"], answers(data));

        wait(read_stats(&env, &server_cli, &server(&env))).unwrap()
    };

    assert_eq!(stats(Some(json!(true))).remote_writable, Some(true));
    assert_eq!(stats(Some(json!(false))).remote_writable, Some(false));
    assert_eq!(
        stats(Some(Value::Null)).remote_writable,
        None,
        "a store that cannot tell"
    );
    assert_eq!(stats(None).remote_writable, None, "an older server");

    let wire = serde_json::to_value(stats(Some(json!(false)))).unwrap();
    assert_eq!(wire["remoteWritable"], false);
}
