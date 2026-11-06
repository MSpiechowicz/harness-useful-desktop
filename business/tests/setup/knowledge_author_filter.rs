//! The author filter of `docs list` (`docs-author-filter`): `--author`, `--author-unknown` and
//! `--sort addedBy` on the command line, the authors facet in the page, a server without the
//! feature as the app asks it, a name the servers refuse that is never sent, and a facet that is
//! read defensively. Every new argument waits for its feature.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{KnowledgeFeatures, ListWindow, list_docs};
use harness_business::model::{
    AuthorCount, AuthorFacet, AuthorFilter, KnowledgeListFilter, KnowledgePage, KnowledgeSort, SortOrder,
};
use knowledge_stand_in::authors::{authored_docs, without_filter};
use knowledge_stand_in::{ALL_FEATURES, cli, document, installed};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{Cli, answers, wait};

fn ada() -> AuthorFilter {
    AuthorFilter::Name {
        name: "Ada Lovelace".to_owned(),
    }
}

fn by(author: Option<AuthorFilter>, sort: Option<KnowledgeSort>) -> KnowledgeListFilter {
    KnowledgeListFilter {
        author,
        sort,
        ..KnowledgeListFilter::default()
    }
}

fn list(server_cli: &Cli, filter: &KnowledgeListFilter, order: Option<SortOrder>) -> (KnowledgePage, Vec<String>) {
    let (_dir, env) = installed();
    let window = ListWindow::new(None, None, None, order).unwrap();

    let page = wait(list_docs(&env, server_cli, &KnowledgeFeatures::new(), filter, &window)).unwrap();
    let args = server_cli.commands().pop().unwrap();

    (page, args)
}

fn titles(page: &KnowledgePage) -> Vec<&str> {
    page.items.iter().map(|item| item.title.as_str()).collect()
}

fn value_of<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == option)
        .map(|pair| pair[1].as_str())
}

fn person(name: &str, count: u32) -> AuthorCount {
    AuthorCount {
        name: name.to_owned(),
        count,
    }
}

#[test]
fn a_server_with_the_feature_filters_sorts_and_lists_the_authors() {
    let server_cli = authored_docs(ALL_FEATURES);
    let facet = AuthorFacet {
        people: vec![person("Ada Lovelace", 2), person("Grace Hopper", 2)],
        unknown: 1,
    };

    let (page, args) = list(&server_cli, &by(Some(ada()), None), None);
    assert_eq!(value_of(&args, "--author"), Some("Ada Lovelace"));
    assert!(!args.contains(&"--author-unknown".to_owned()));
    assert_eq!(titles(&page), ["api", "deploy"]);
    assert_eq!(page.authors.as_ref(), Some(&facet), "the facet keeps every choice");

    let (page, args) = list(&server_cli, &by(Some(AuthorFilter::Unknown), None), None);
    assert!(args.contains(&"--author-unknown".to_owned()));
    assert!(!args.contains(&"--author".to_owned()));
    assert_eq!(titles(&page), ["faq"]);
    assert_eq!(page.authors.as_ref(), Some(&facet));

    let sorted = by(None, Some(KnowledgeSort::AddedBy));
    let (page, args) = list(&server_cli, &sorted, None);
    assert_eq!(value_of(&args, "--sort"), Some("addedBy"));
    assert!(!args.contains(&"--desc".to_owned()), "A to Z by default");
    assert_eq!(titles(&page), ["api", "deploy", "build", "guide", "faq"]);
    assert!(!args.contains(&"--author".to_owned()));

    let (_, args) = list(&server_cli, &sorted, Some(SortOrder::Desc));
    assert!(args.contains(&"--desc".to_owned()));

    let (page, args) = list(&server_cli, &KnowledgeListFilter::default(), None);
    assert!(!args.contains(&"--author".to_owned()) && !args.contains(&"--author-unknown".to_owned()));
    assert_eq!(page.items.len(), 5);
}

#[test]
fn a_server_without_the_feature_is_asked_for_no_author_and_its_page_has_no_facet() {
    let server_cli = authored_docs(&without_filter());

    for author in [Some(ada()), Some(AuthorFilter::Unknown), None] {
        let (page, args) = list(&server_cli, &by(author, Some(KnowledgeSort::AddedBy)), None);

        assert!(!args.contains(&"--author".to_owned()), "{args:?}");
        assert!(!args.contains(&"--author-unknown".to_owned()), "{args:?}");
        assert_eq!(value_of(&args, "--sort"), Some("updated"), "the default order");
        assert!(args.contains(&"--desc".to_owned()));
        assert_eq!(page.authors, None);
        assert_eq!(page.items.len(), 5, "the list still reads");
        assert!(
            page.items.iter().any(|item| item.added_by.is_some()),
            "the rows carry names"
        );
    }

    // The sorts the old server knows are kept.
    let (_, args) = list(&server_cli, &by(Some(ada()), Some(KnowledgeSort::Title)), None);
    assert_eq!(value_of(&args, "--sort"), Some("title"));
}

#[test]
fn a_name_the_servers_refuse_is_refused_before_anything_runs() {
    let too_long = "a".repeat(81);

    for features in [ALL_FEATURES.to_owned(), without_filter()] {
        for name in [
            "",
            "   ",
            " Ada",
            "Ada ",
            "Ada\nLovelace",
            "Ada\u{202e}",
            too_long.as_str(),
        ] {
            let server_cli = authored_docs(&features);
            let (_dir, env) = installed();
            let window = ListWindow::new(None, None, None, None).unwrap();
            let filter = by(Some(AuthorFilter::Name { name: name.to_owned() }), None);

            let error = wait(list_docs(
                &env,
                &server_cli,
                &KnowledgeFeatures::new(),
                &filter,
                &window,
            ))
            .expect_err("refused, not read as an older server");

            assert!(matches!(error, BusinessError::InvalidInput(_)), "{name:?}: {error:?}");
            assert!(server_cli.commands().is_empty(), "{name:?} reached the server");
        }
    }
}

fn facet_cli(authors: Value) -> Cli {
    cli(ALL_FEATURES).starting(
        &["docs", "list"],
        answers(json!({ "items": [document(7, "guide")], "total": 1, "authors": authors })),
    )
}

#[test]
fn a_facet_is_read_defensively() {
    let messy = json!({
        "people": [
            { "name": "Ada Lovelace", "count": 2 },
            { "name": "Eve\u{202e}", "count": 1 },
            { "name": " Padded", "count": 1 },
            { "name": "Tab\there", "count": 1 },
            { "name": "", "count": 1 },
            { "name": "No count" },
            { "name": "Negative", "count": -1 },
            { "name": "Text count", "count": "3" },
            { "count": 4 },
            { "name": 7, "count": 4 },
            "Grace",
            null,
            { "name": "é".repeat(81), "count": 1 },
            { "name": "Grace Hopper", "count": 5 },
        ],
        "unknown": "many",
    });

    let (page, _) = list(&facet_cli(messy), &KnowledgeListFilter::default(), None);
    let facet = page.authors.expect("a facet");

    assert_eq!(
        facet.people,
        [person("Ada Lovelace", 2), person("Grace Hopper", 5)],
        "only the names the filter could give back, in the server's order"
    );
    assert_eq!(facet.unknown, 0, "a count that is not a number is none");

    let (page, _) = list(
        &facet_cli(json!({ "people": "none", "unknown": 3 })),
        &KnowledgeListFilter::default(),
        None,
    );
    assert_eq!(
        page.authors,
        Some(AuthorFacet {
            people: Vec::new(),
            unknown: 3
        })
    );

    for authors in [json!(null), json!([]), json!("Ada"), json!(7)] {
        let (page, _) = list(&facet_cli(authors.clone()), &KnowledgeListFilter::default(), None);
        assert_eq!(page.authors, None, "{authors}");
    }

    let (page, _) = list(
        &cli(ALL_FEATURES).starting(&["docs", "list"], answers(json!({ "items": [], "total": 0 }))),
        &KnowledgeListFilter::default(),
        None,
    );
    assert_eq!(page.authors, None, "an older server sends none");
}

#[test]
fn a_facet_is_bounded() {
    let people: Vec<Value> = (0..500)
        .map(|n| json!({ "name": format!("Person {n}"), "count": 1 }))
        .collect();

    let (page, _) = list(
        &facet_cli(json!({ "people": people, "unknown": 4_000_000_000_000_u64 })),
        &KnowledgeListFilter::default(),
        None,
    );
    let facet = page.authors.expect("a facet");

    assert_eq!(facet.people.len(), 201);
    assert_eq!(facet.people[0].name, "Person 0");
    assert_eq!(facet.unknown, u32::MAX, "a count past u32 is clamped");
}

#[test]
fn the_filter_has_the_shape_of_a_scope_on_the_wire_and_an_absent_one_reads() {
    assert_eq!(
        serde_json::to_value(ada()).unwrap(),
        json!({ "kind": "name", "name": "Ada Lovelace" })
    );
    assert_eq!(
        serde_json::to_value(AuthorFilter::Unknown).unwrap(),
        json!({ "kind": "unknown" })
    );
    assert_eq!(serde_json::to_value(KnowledgeSort::AddedBy).unwrap(), json!("addedBy"));

    let with: KnowledgeListFilter = serde_json::from_value(json!({
        "author": { "kind": "unknown" }, "sort": "addedBy"
    }))
    .unwrap();
    assert_eq!(
        (with.author, with.sort),
        (Some(AuthorFilter::Unknown), Some(KnowledgeSort::AddedBy))
    );

    for body in [json!({}), json!({ "author": null })] {
        let without: KnowledgeListFilter = serde_json::from_value(body).unwrap();
        assert_eq!(without.author, None);
    }
}

#[test]
fn the_feature_line_is_read_apart_from_the_authors_line() {
    let (_dir, env) = installed();

    let filter_only = wait(KnowledgeFeatures::new().support(&env, &cli("docs\ndocs-author-filter\n")));
    assert!(filter_only.docs_author_filter() && !filter_only.docs_authors());
    assert!(filter_only.status().features.docs_author_filter);

    let authors_only = wait(KnowledgeFeatures::new().support(&env, &cli("docs\ndocs-authors\n")));
    assert!(authors_only.docs_authors() && !authors_only.docs_author_filter());
}
