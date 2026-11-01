//! The documents of Harness Knowledge through `harness-knowledge docs …`, `repos` and `stats`: the
//! arguments a page, a filter and a sort become (the server counts pages from 1), the answers read
//! into the contract types, what is refused before anything runs, a missing capability (exit 3) as
//! the page's availability, and an older server (exit 2) as unsupported.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeFeatures, ListWindow, delete_docs, get_doc, list_docs, list_knowledge_repos, read_stats, rename_doc,
    set_links,
};
use harness_business::model::{
    KnowledgeAvailability, KnowledgeLink, KnowledgeListFilter, KnowledgeOrigin, KnowledgeProblem, KnowledgeScope,
    KnowledgeSort, SortOrder,
};
use knowledge_stand_in::{ALL_FEATURES, REPO, cli, document, installed, server};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{Cli, answers, exits, wait};

fn window(offset: u32, limit: u32, query: Option<&str>, order: Option<SortOrder>) -> ListWindow {
    ListWindow::new(Some(offset), Some(limit), query.map(str::to_owned), order).unwrap()
}

fn page_cli() -> Cli {
    cli(ALL_FEATURES).starting(
        &["docs", "list"],
        answers(json!({ "items": [document(7, "guide"), document(8, "api")], "total": 120 })),
    )
}

fn last_args(cli: &Cli) -> Vec<String> {
    cli.commands().pop().unwrap()
}

#[test]
fn a_page_becomes_the_servers_page_number_with_the_sort_and_filters() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server = page_cli();
    let filter = KnowledgeListFilter {
        scope: Some(KnowledgeScope::Repo { key: REPO.to_owned() }),
        format: Some("markdown".to_owned()),
        sort: Some(KnowledgeSort::Title),
        order: None,
    };

    let page = wait(list_docs(
        &env,
        &server,
        &probe,
        &filter,
        &window(100, 50, Some(" tls "), None),
    ))
    .unwrap();

    assert_eq!(page.availability, KnowledgeAvailability::Available);
    assert_eq!(page.total, 120);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].origin, KnowledgeOrigin::User);
    assert_eq!(page.items[0].links[0].globs, ["src/**"]);
    assert_eq!(
        last_args(&server),
        [
            "docs",
            "list",
            "--json",
            "--repo",
            REPO,
            "--format",
            "markdown",
            "--query",
            "tls",
            "--sort",
            "title",
            "--page",
            "3",
            "--per-page",
            "50"
        ]
    );
}

#[test]
fn newest_and_largest_go_first_unless_the_order_says_otherwise() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server = page_cli();
    let global = KnowledgeListFilter {
        scope: Some(KnowledgeScope::Global),
        ..KnowledgeListFilter::default()
    };

    wait(list_docs(&env, &server, &probe, &global, &window(0, 50, None, None))).unwrap();
    assert_eq!(
        last_args(&server),
        [
            "docs",
            "list",
            "--json",
            "--global",
            "--sort",
            "updated",
            "--desc",
            "--page",
            "1",
            "--per-page",
            "50"
        ]
    );

    wait(list_docs(
        &env,
        &server,
        &probe,
        &KnowledgeListFilter::default(),
        &window(0, 10, None, Some(SortOrder::Asc)),
    ))
    .unwrap();
    assert!(!last_args(&server).contains(&"--desc".to_owned()));
}

#[test]
fn what_the_server_would_refuse_is_refused_before_anything_runs() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server = page_cli();
    let invalid = |filter: KnowledgeListFilter, window: ListWindow| {
        let error = wait(list_docs(&env, &server, &probe, &filter, &window)).expect_err("refused");
        assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    };

    invalid(KnowledgeListFilter::default(), window(25, 50, None, None));
    invalid(
        KnowledgeListFilter::default(),
        window(0, 50, Some(&"x".repeat(201)), None),
    );
    invalid(
        KnowledgeListFilter {
            format: Some("Mark Down".to_owned()),
            ..KnowledgeListFilter::default()
        },
        window(0, 50, None, None),
    );
    invalid(
        KnowledgeListFilter {
            scope: Some(KnowledgeScope::Repo { key: "*".to_owned() }),
            ..KnowledgeListFilter::default()
        },
        window(0, 50, None, None),
    );
    assert!(server.commands().is_empty());
}

#[test]
fn a_database_that_lacks_a_capability_is_the_pages_availability() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let missing = cli(ALL_FEATURES)
        .starting(
            &["docs", "list"],
            exits(
                3,
                "{\"error\":\"missing\",\"code\":\"no-pgvector\",\"message\":\"extension vector is not available\"}\n",
                "harness-knowledge: extension vector is not available\n",
            ),
        )
        .starting(
            &["repos"],
            exits(
                3,
                "{\"error\":\"missing\",\"code\":\"no-mongot\",\"message\":\"no mongot\"}\n",
                "",
            ),
        );

    let page = wait(list_docs(
        &env,
        &missing,
        &probe,
        &KnowledgeListFilter::default(),
        &window(0, 50, None, None),
    ))
    .unwrap();
    assert_eq!(
        page.availability,
        KnowledgeAvailability::Missing {
            problem: KnowledgeProblem::NoPgvector,
            message: "extension vector is not available".to_owned()
        }
    );
    assert!(page.items.is_empty());

    let repos = wait(list_knowledge_repos(&env, &missing, &probe));
    assert!(matches!(
        repos.availability,
        KnowledgeAvailability::Missing {
            problem: KnowledgeProblem::NoMongot,
            ..
        }
    ));
}

#[test]
fn a_failing_server_is_its_line_and_an_older_one_is_outdated() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let failing = cli(ALL_FEATURES).starting(
        &["docs", "list"],
        exits(1, "", "harness-knowledge: the database is locked\n"),
    );
    let page = wait(list_docs(
        &env,
        &failing,
        &probe,
        &KnowledgeListFilter::default(),
        &window(0, 50, None, None),
    ))
    .unwrap();
    assert_eq!(
        page.availability,
        KnowledgeAvailability::Failed {
            message: "harness-knowledge: the database is locked".to_owned()
        }
    );

    // `docs get` is a usage error (exit 2) on a server that predates it.
    let old = cli(ALL_FEATURES);
    let error = wait(get_doc(&env, &old, &server(&env), &7.into(), None)).expect_err("old");
    assert!(matches!(error, BusinessError::Unsupported(message) if message.contains("predates `docs get`")));
}

#[test]
fn a_document_comes_with_its_outline_and_a_chunk_when_asked() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(
        &["docs", "get"],
        answers(json!({
            "document": document(7, "guide"),
            "sections": [{"chunkId": 31, "ordinal": 0, "headingPath": "Guide > Setup", "tokens": 210, "charStart": 0, "charEnd": 900}],
            "chunk": {"id": 31, "docId": 7, "ordinal": 0, "headingPath": "Guide", "text": "Install it.", "tokens": 3, "charStart": 0, "charEnd": 11}
        })),
    );

    let detail = wait(get_doc(&env, &server_cli, &server(&env), &7.into(), Some(&31.into()))).unwrap();

    assert_eq!(detail.document.title, "guide");
    assert_eq!(detail.sections[0].heading_path, "Guide > Setup");
    assert_eq!(detail.chunk.unwrap().text, "Install it.");
    assert_eq!(
        last_args(&server_cli),
        ["docs", "get", "--json", "7", "--sections", "--chunk", "31"]
    );
    assert!(wait(get_doc(&env, &server_cli, &server(&env), &0.into(), None)).is_err());
}

#[test]
fn links_travel_on_stdin_and_are_checked_first() {
    let (_dir, env) = installed();
    let links = vec![KnowledgeLink {
        repo: REPO.to_owned(),
        globs: vec!["src/api/**".to_owned()],
    }];
    let server_cli = cli(ALL_FEATURES).starting(
        &["docs", "links", "set"],
        answers(json!({ "id": 7, "links": [{ "repo": REPO, "globs": ["src/api/**"] }] })),
    );

    let stored = wait(set_links(&env, &server_cli, &server(&env), &7.into(), &links)).unwrap();

    assert_eq!(stored, links);
    assert_eq!(last_args(&server_cli), ["docs", "links", "set", "--json", "7"]);
    let stdin = server_cli.runs.lock().unwrap().last().unwrap().stdin.clone().unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stdin).unwrap(),
        json!([{ "repo": REPO, "globs": ["src/api/**"] }])
    );

    let bad = [KnowledgeLink {
        repo: "not-a-key".to_owned(),
        globs: vec![],
    }];
    assert!(matches!(
        wait(set_links(&env, &server_cli, &server(&env), &7.into(), &bad)),
        Err(BusinessError::InvalidInput(_))
    ));
    assert_eq!(server_cli.commands().len(), 1, "the refused links ran nothing");
}

#[test]
fn rename_delete_repos_and_stats_read_what_the_server_prints() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES)
        .starting(&["docs", "rename"], answers(json!({ "id": 7, "title": "Guide v2" })))
        .starting(&["docs", "delete"], answers(json!({ "deleted": 2 })))
        .starting(
            &["repos"],
            answers(json!({ "global": 4, "repos": [{ "key": REPO, "docs": 9 }] })),
        )
        .starting(
            &["stats"],
            answers(json!({
                "documents": 13, "chunks": 90, "embeddedChunks": 88, "bytes": 123456, "globalDocuments": 4,
                "repositories": 1, "generation": 17, "schema": 1, "model": "multilingual-e5-small"
            })),
        );
    let path = server(&env);

    assert_eq!(
        wait(rename_doc(&env, &server_cli, &path, &7.into(), "  Guide v2 ")).unwrap(),
        "Guide v2"
    );
    assert_eq!(
        last_args(&server_cli),
        ["docs", "rename", "--json", "7", "--title", "Guide v2"]
    );
    assert!(wait(rename_doc(&env, &server_cli, &path, &7.into(), "   ")).is_err());

    assert_eq!(
        wait(delete_docs(&env, &server_cli, &path, &[7.into(), 8.into()]))
            .unwrap()
            .deleted,
        2
    );
    assert_eq!(last_args(&server_cli), ["docs", "delete", "--json", "7", "8"]);
    assert!(wait(delete_docs(&env, &server_cli, &path, &[])).is_err());
    assert!(wait(delete_docs(&env, &server_cli, &path, &[0.into()])).is_err());

    let repos = wait(list_knowledge_repos(&env, &server_cli, &probe));
    assert_eq!(
        (repos.global, repos.repos[0].key.as_str(), repos.repos[0].docs),
        (4, REPO, 9)
    );

    let stats = wait(read_stats(&env, &server_cli, &path)).unwrap();
    assert_eq!((stats.documents, stats.embedded_chunks, stats.generation), (13, 88, 17));
    assert_eq!(stats.model.as_deref(), Some("multilingual-e5-small"));
}

#[test]
fn local_ids_travel_back_exactly_through_get_delete_and_matches() {
    use harness_business::ecosystem::knowledge_doc_matches;
    use harness_business::model::{KnowledgeId, KnowledgePlace};

    let (_dir, env) = installed();
    let local = KnowledgeId::Local("local:12".to_owned());
    let chunk = KnowledgeId::Local("local:340".to_owned());
    let mut guide = document(12, "guide");
    guide["id"] = json!("local:12");
    guide["place"] = json!("local");
    let server_cli = cli(ALL_FEATURES)
        .starting(
            &["docs", "list"],
            answers(json!({
                "items": [guide, document(8, "api")],
                "total": 2
            })),
        )
        .starting(
            &["docs", "get"],
            answers(json!({
                "document": document(7, "guide"),
                "sections": [{"chunkId": "local:340", "ordinal": 0, "headingPath": "Guide", "tokens": 2, "charStart": 0, "charEnd": 9}],
            })),
        )
        .starting(&["docs", "delete"], answers(json!({ "deleted": 1 })))
        .starting(
            &["docs", "matches"],
            answers(json!({
                "matches": [{ "chunkId": "local:340", "ordinal": 0, "section": "Guide > Setup", "snippet": "install the gateway" }],
                "total": 3
            })),
        );
    let path = server(&env);

    let detail = wait(get_doc(&env, &server_cli, &path, &local, Some(&chunk))).unwrap();
    assert_eq!(detail.sections[0].chunk_id, chunk);
    assert_eq!(
        last_args(&server_cli),
        [
            "docs",
            "get",
            "--json",
            "local:12",
            "--sections",
            "--chunk",
            "local:340"
        ]
    );

    wait(delete_docs(&env, &server_cli, &path, &[local.clone(), 8.into()])).unwrap();
    assert_eq!(last_args(&server_cli), ["docs", "delete", "--json", "local:12", "8"]);

    let found = wait(knowledge_doc_matches(
        &env,
        &server_cli,
        &path,
        &local,
        "  gateway ",
        Some(5),
    ))
    .unwrap();
    assert_eq!((found.total, found.matches[0].chunk_id.clone()), (3, chunk));
    assert_eq!(
        last_args(&server_cli),
        [
            "docs", "matches", "--json", "local:12", "--query", "gateway", "--limit", "5"
        ]
    );
    assert!(wait(knowledge_doc_matches(&env, &server_cli, &path, &local, "  ", None)).is_err());
    assert!(wait(knowledge_doc_matches(&env, &server_cli, &path, &local, "x", Some(51))).is_err());

    for bad in ["local:", "local:0", "local:1:2:3", "remote:1", "12"] {
        let id = KnowledgeId::Local(bad.to_owned());
        assert!(wait(get_doc(&env, &server_cli, &path, &id, None)).is_err(), "{bad}");
    }

    let probe = KnowledgeFeatures::new();
    let page = wait(list_docs(
        &env,
        &server_cli,
        &probe,
        &KnowledgeListFilter::default(),
        &window(0, 50, None, None),
    ))
    .unwrap();
    assert_eq!(page.items[0].id, local);
    assert_eq!(page.items[0].place, Some(KnowledgePlace::Local));
    assert_eq!(page.items[1].place, None);
}
