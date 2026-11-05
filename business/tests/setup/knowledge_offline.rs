//! Harness Knowledge while its remote database cannot be reached (the `offline-fallback` feature):
//! the `offline` flag on lists, repositories, counts and the live pulse, the `--place` filter, and
//! a remote document that cannot be read as an `unreachable` error instead of a failure.

use crate::knowledge_stand_in::{ALL_FEATURES, cli, document, installed, server};
use crate::team_support::{answers, exits, wait};

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeFeatures, KnowledgeSupport, ListWindow, get_doc, knowledge_config_path, knowledge_doc_matches,
    knowledge_pulse, knowledge_remote_get, list_docs, list_knowledge_repos, read_stats, rename_doc,
};
use harness_business::env::Env;
use harness_business::model::{KnowledgeAvailability, KnowledgeId, KnowledgeListFilter, KnowledgePlace};
use pretty_assertions::assert_eq;
use serde_json::json;

const UNREACHABLE: &str = "harness-knowledge: remote knowledge is unreachable right now\n";

/// Saves a remote database and the routing of documents in the server's config file.
fn route(env: &Env, repository: &str, global: &str) {
    let path = knowledge_config_path(env).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!(
            "[database]\nurl = \"postgres://h/k\"\n[routing]\nrepository_documents = \"{repository}\"\nglobal_documents = \"{global}\"\n"
        ),
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
}

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
fn a_remote_list_asked_while_offline_is_an_empty_offline_page_when_both_places_are_in_use() {
    let (_dir, env) = installed();
    route(&env, "remote+local", "remote");
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
            "offlineReason": "  cannot reach\u{202e} the database  ",
            "places": { "remote": { "offline": true }, "local": { "documents": 2, "chunks": 9 } }
        })),
    );

    let stats = wait(read_stats(&env, &server_cli, &server(&env))).unwrap();
    let places = stats.places.expect("both places");
    assert!(stats.offline && places.remote.expect("remote").offline);
    assert!(!places.local.expect("local").offline);

    let pulse = wait(knowledge_pulse(&env, &server_cli, &probe)).expect("the counts");
    assert_eq!((pulse.documents, pulse.offline), (2, true));
    assert_eq!(
        pulse.offline_reason.as_deref(),
        Some("cannot reach the database"),
        "shown clean"
    );
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

#[test]
fn the_servers_unreachable_code_counts_even_when_its_line_is_long_or_not_the_known_one() {
    let (_dir, env) = installed();
    let path = server(&env);
    let remote = KnowledgeId::Remote(7);
    let coded = json!({ "error": "unreachable", "message": "the remote database did not answer" }).to_string();
    // A stderr line clipped before it says anything about being unreachable.
    let clipped = format!(
        "harness-knowledge: {} remote knowledge is unreachable right now\n",
        "x".repeat(600)
    );
    let long = format!(
        "harness-knowledge: {} remote knowledge is unreachable right now\n",
        "x".repeat(300)
    );
    let server_cli = cli(ALL_FEATURES)
        .starting(&["docs", "get"], exits(1, &format!("{coded}\n"), &clipped))
        .starting(
            &["docs", "rename"],
            exits(1, &format!("{coded}\n"), "harness-knowledge: nothing was renamed\n"),
        )
        .starting(&["docs", "links"], exits(1, "", &long));

    let error = wait(get_doc(&env, &server_cli, &path, &remote, None)).expect_err("offline");
    assert!(matches!(error, BusinessError::Unreachable(_)), "{error:?}");

    let error = wait(rename_doc(&env, &server_cli, &path, &remote, "New")).expect_err("offline");
    assert!(matches!(error, BusinessError::Unreachable(_)), "{error:?}");

    // Without the code the line is the fallback, whatever came before the words.
    let error = wait(harness_business::ecosystem::set_links(
        &env,
        &server_cli,
        &path,
        &remote,
        &[],
    ))
    .expect_err("offline");
    assert!(matches!(error, BusinessError::Unreachable(_)), "{error:?}");
}

#[test]
fn a_held_decision_while_offline_is_unreachable() {
    use harness_business::model::KnowledgeDecision;

    let (_dir, env) = installed();
    let coded = json!({ "error": "unreachable", "message": "the remote database did not answer" }).to_string();
    let server_cli = cli(ALL_FEATURES).starting(&["held", "approve"], exits(1, &format!("{coded}\n"), ""));

    let error = wait(harness_business::ecosystem::decide_knowledge_held(
        &env,
        &server_cli,
        &server(&env),
        "3f1c2a9e-7b4d-4e0a-9c55-1d2e3f4a5b6c",
        KnowledgeDecision::Approve,
        None,
        None,
    ))
    .expect_err("offline");

    assert!(matches!(error, BusinessError::Unreachable(_)), "{error:?}");
}

#[test]
fn the_place_filter_is_only_sent_to_a_server_that_falls_back() {
    let (_dir, env) = installed();
    let old = cli("docs\nrouting\n").starting(&["docs", "list"], answers(json!({ "items": [], "total": 0 })));
    let current = cli(ALL_FEATURES).starting(&["docs", "list"], answers(json!({ "items": [], "total": 0 })));

    for server_cli in [&old, &current] {
        wait(list_docs(
            &env,
            server_cli,
            &KnowledgeFeatures::new(),
            &place_filter(KnowledgePlace::Local),
            &window(),
        ))
        .unwrap();
    }

    let asked = |server_cli: &crate::team_support::Cli| {
        server_cli
            .commands()
            .into_iter()
            .find(|command| command[0] == "docs")
            .expect("a list")
    };
    assert!(!asked(&old).contains(&"--place".to_owned()), "{:?}", asked(&old));
    assert!(asked(&current).contains(&"--place".to_owned()), "{:?}", asked(&current));
}

#[test]
fn a_list_while_the_remote_is_the_only_place_and_unreachable_is_unreachable_not_an_empty_library() {
    let (_dir, env) = installed();
    route(&env, "remote", "remote");
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES).starting(&["docs", "list"], exits(1, "", UNREACHABLE));

    for filter in [KnowledgeListFilter::default(), place_filter(KnowledgePlace::Remote)] {
        let page = wait(list_docs(&env, &server_cli, &probe, &filter, &window())).unwrap();

        assert_eq!(
            (page.availability, page.items.len(), page.offline),
            (KnowledgeAvailability::Unreachable, 0, false)
        );
    }
}

#[test]
fn the_pulse_marks_offline_when_the_counts_fail_unreachable_and_stays_silent_for_other_failures() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let unreachable = cli(ALL_FEATURES).starting(&["stats"], exits(1, "", UNREACHABLE));
    let broken = cli(ALL_FEATURES).starting(&["stats"], exits(1, "", "harness-knowledge: disk full\n"));

    let pulse = wait(knowledge_pulse(&env, &unreachable, &probe)).expect("an offline pulse");
    assert_eq!((pulse.documents, pulse.held, pulse.offline), (0, None, true));

    assert_eq!(wait(knowledge_pulse(&env, &broken, &probe)), None);
}

#[test]
fn the_combined_generation_is_exact_as_a_javascript_number() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let place = |generation: u64| json!({ "documents": 3, "chunks": 9, "embeddedChunks": 9, "bytes": 1, "globalDocuments": 1, "repositories": 1, "generation": generation });

    for generation in [1_u64, 7, 1 << 40, 1 << 62] {
        let server_cli = cli(ALL_FEATURES).starting(
            &["stats"],
            answers(json!({
                "documents": 3, "chunks": 9, "embeddedChunks": 9, "bytes": 1, "globalDocuments": 1,
                "repositories": 1, "generation": 1, "schema": 1, "model": null, "held": 0,
                "places": { "remote": place(generation), "local": place(generation + 1) }
            })),
        );

        let pulse = wait(knowledge_pulse(&env, &server_cli, &probe)).expect("the counts");
        assert!((0..(1_i64 << 52)).contains(&pulse.generation), "{}", pulse.generation);
    }
}

#[test]
fn a_server_that_is_upgrading_its_database_is_a_passing_state_not_a_failure() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server_cli = cli(ALL_FEATURES).starting(
        &["docs", "list"],
        exits(
            1,
            "",
            "harness-knowledge: upgrading the knowledge database, try again in a moment\n",
        ),
    );

    let page = wait(list_docs(
        &env,
        &server_cli,
        &probe,
        &KnowledgeListFilter::default(),
        &window(),
    ))
    .unwrap();

    assert_eq!(page.availability, KnowledgeAvailability::Upgrading);
}

#[test]
fn a_line_that_only_mentions_the_unreachable_words_is_not_an_unreachable_remote() {
    let (_dir, env) = installed();
    let path = server(&env);
    let remote = KnowledgeId::Remote(7);
    let server_cli = cli(ALL_FEATURES)
        .starting(
            &["docs", "get"],
            exits(
                1,
                "",
                "harness-knowledge: remote knowledge is unreachable right now for 3 of 4 documents\n",
            ),
        )
        .starting(
            &["docs", "matches"],
            exits(1, "", "harness-knowledge: remote knowledge is unreachable right now.\n"),
        );

    let error = wait(get_doc(&env, &server_cli, &path, &remote, None)).expect_err("failed");
    assert!(matches!(error, BusinessError::Process(_)), "{error:?}");

    let error = wait(knowledge_doc_matches(
        &env,
        &server_cli,
        &path,
        &remote,
        "gateway",
        None,
    ))
    .expect_err("offline");
    assert!(
        matches!(error, BusinessError::Unreachable(_)),
        "a closing period still counts: {error:?}"
    );
}

#[test]
fn a_whitespace_only_routing_variable_is_a_bad_value_as_the_server_reads_it() {
    let (_dir, env) = installed();
    route(&env, "remote", "remote");
    let blank = env.clone().with_var("HARNESS_KNOWLEDGE_REPOSITORY_DOCUMENTS", "  ");
    let empty = env.with_var("HARNESS_KNOWLEDGE_REPOSITORY_DOCUMENTS", "");

    let remote = knowledge_remote_get(&blank, KnowledgeSupport::Unknown);
    assert!(
        remote.repository_documents_env && remote.problem.is_some(),
        "the server refuses to start"
    );

    let remote = knowledge_remote_get(&empty, KnowledgeSupport::Unknown);
    assert!(
        !remote.repository_documents_env && remote.problem.is_none(),
        "empty is unset"
    );
}
