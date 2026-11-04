//! `harness-knowledge features`: what the installed server can do. The answer is cached per binary
//! like the memory server's (the shared line probe), an older server (a usage error, exit 2) is
//! cached as one without features, and a probe that failed is not cached. Every command waits
//! for its feature.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::ListWindow;
use harness_business::ecosystem::{KnowledgeFeatures, KnowledgeSupport, docs_server, list_docs};
use harness_business::model::{KnowledgeAvailability, KnowledgeFeatures as Features, KnowledgeListFilter};
use knowledge_stand_in::{ALL_FEATURES, cli, installed};
use pretty_assertions::assert_eq;
use team_support::{exits, wait};

const EVERY_FEATURE: Features = Features {
    docs: true,
    import: true,
    embedder: true,
    search: true,
    recall: true,
    held: true,
    mcp: true,
    routing: true,
    doc_matches: true,
    offline_fallback: true,
    docs_useful: true,
    docs_similar: true,
    docs_authors: true,
    docs_drafts: true,
    docs_approval: true,
};

#[test]
fn a_server_that_lists_every_feature_is_asked_once() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let cli = cli(&format!("{ALL_FEATURES}something-newer\n"));

    let answer = wait(probe.support(&env, &cli));

    assert_eq!(answer, KnowledgeSupport::Listed(EVERY_FEATURE));
    assert_eq!(answer.status().availability, KnowledgeAvailability::Available);
    assert_eq!(wait(probe.support(&env, &cli)), answer);
    assert_eq!(cli.commands(), [["features"]], "the answer is cached");
}

#[test]
fn each_feature_is_read_apart_and_tokens_are_trimmed_and_lowercased() {
    let (_dir, env) = installed();
    let answer = wait(KnowledgeFeatures::new().support(&env, &cli(" Docs \nHELD\nimport-ndjson\n")));

    assert!(answer.docs() && answer.held() && answer.import());
    assert!(!answer.embedder() && !answer.search() && !answer.recall());
    assert!(!answer.features().mcp);
}

#[test]
fn an_older_server_is_outdated_and_everything_waits_for_its_feature() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    // No rule answers `features`: a usage error, exit 2.
    let old = team_support::Cli::default();

    let answer = wait(probe.support(&env, &old));
    assert_eq!(answer, KnowledgeSupport::Unsupported);
    assert_eq!(answer.status().availability, KnowledgeAvailability::Outdated);
    assert_eq!(answer.status().features, Features::default());

    let error = wait(docs_server(&env, &old, &probe)).expect_err("no documents");
    assert!(matches!(error, BusinessError::Unsupported(message) if message.contains("harness-knowledge predates")));

    let page = wait(list_docs(
        &env,
        &old,
        &probe,
        &KnowledgeListFilter::default(),
        &ListWindow::new(None, None, None, None).unwrap(),
    ))
    .unwrap();
    assert_eq!(page.availability, KnowledgeAvailability::Outdated);
    assert_eq!(old.commands(), [["features"]], "only the probe ran, once");
}

#[test]
fn a_feature_the_server_lacks_gates_its_command_only() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let server = cli("docs\n");

    assert!(wait(probe.feature_server(&env, &server, KnowledgeSupport::docs, "documents")).is_ok());

    let held = wait(probe.feature_server(&env, &server, KnowledgeSupport::held, "the review queue"));
    assert!(matches!(held, Err(BusinessError::Unsupported(message)) if message.contains("the review queue")));
}

#[test]
fn without_the_server_nothing_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let env = harness_business::env::Env::for_tests(dir.path());
    let cli = cli(ALL_FEATURES);
    let probe = KnowledgeFeatures::new();

    assert_eq!(wait(probe.support(&env, &cli)), KnowledgeSupport::NotInstalled);
    assert_eq!(
        wait(probe.support(&env, &cli)).status().availability,
        KnowledgeAvailability::NotInstalled
    );
    assert!(matches!(
        wait(docs_server(&env, &cli, &probe)),
        Err(BusinessError::NotFound(_))
    ));
    assert!(cli.commands().is_empty());
}

#[test]
fn a_probe_that_failed_is_not_cached() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();
    let broken = team_support::Cli::default().starting(&["features"], exits(1, "", "harness-knowledge: boom"));

    assert_eq!(wait(probe.support(&env, &broken)), KnowledgeSupport::Unsupported);
    assert_eq!(wait(probe.support(&env, &broken)), KnowledgeSupport::Unsupported);
    assert_eq!(broken.commands().len(), 2, "a failure is asked again");

    let fixed = cli("docs\n");
    assert!(wait(probe.support(&env, &fixed)).docs());
}
