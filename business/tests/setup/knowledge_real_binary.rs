//! A smoke test of the commands the app runs against a real `harness-knowledge`, which the
//! stand-ins of the other modules only imitate: `features`, the streamed import of a small folder,
//! `docs list|get`, `stats`, `held`, `search` and `check --json`, all through the app's own code and real
//! processes.
//!
//! It runs only when `HARNESS_KNOWLEDGE_TEST_BIN` names a built `harness-knowledge`, and skips
//! otherwise (also off Unix, where the server is reached through a launcher script). The server
//! runs from a launcher that gives it a temporary home, data folder and database, so it never
//! touches the user's own, and has no embedder (no model is installed there), so a search is
//! keyword-only and the imported chunks wait for a reindex.

#![cfg(unix)]

use crate::common::set_mode;
use crate::knowledge_stand_in::{installed_at, server};
use crate::team_support::wait;

use std::io::Write;
use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::ecosystem::{
    ImportManifest, KnowledgeFeatures, KnowledgeImporter, KnowledgeRemoteChange, ListWindow, decide_knowledge_held,
    docs_server, get_doc, get_knowledge_held, knowledge_doc_matches, knowledge_held_server, knowledge_pulse,
    knowledge_recall, knowledge_remote_get, knowledge_remote_set, knowledge_remote_test, list_docs,
    list_knowledge_held, read_check, read_stats, run_import, search_knowledge,
};
use harness_business::env::Env;
use harness_business::model::{
    KnowledgeApproval, KnowledgeAvailability, KnowledgeDecided, KnowledgeDecision, KnowledgeEmbedding,
    KnowledgeFileStatus, KnowledgeImportEvent, KnowledgeImportOptions, KnowledgeListFilter, KnowledgeOrigin,
    KnowledgeSearchScope, MemoryNotesRouting, MemoryRemoteKind,
};
use harness_business::process::{ExecSpec, ProcessRunner, TokioRunner, TokioSpawner};
use pretty_assertions::assert_eq;

/// The built server to run, when the smoke is asked for.
fn real_binary() -> Option<PathBuf> {
    let binary = std::env::var_os("HARNESS_KNOWLEDGE_TEST_BIN").map(PathBuf::from);

    if binary.is_none() {
        eprintln!("skipped: HARNESS_KNOWLEDGE_TEST_BIN does not name a built harness-knowledge");
    }

    binary
}

/// A temporary home whose installed `harness-knowledge` is a launcher for `real`: it runs it with
/// that home, a data folder and database of its own, and no team database.
fn scene(real: &Path) -> (tempfile::TempDir, Env) {
    let home = tempfile::tempdir().unwrap();
    let (env, path) = installed_at(home.path());
    let root = home.path();

    let shebang = "#!/bin/sh\n"; // guard-allow: installer-shell

    let launcher = format!(
        "{shebang}export HOME='{home}'\nexport XDG_DATA_HOME='{home}/data'\nexport XDG_CONFIG_HOME='{home}/config'\n\
         export HARNESS_KNOWLEDGE_DIR='{home}/db'\n\
         unset HARNESS_KNOWLEDGE_CONFIG HARNESS_KNOWLEDGE_DATABASE_URL HARNESS_KNOWLEDGE_DATABASE_TOKEN HARNESS_KNOWLEDGE_SESSION\n\
         exec '{real}' \"$@\"\n",
        home = root.display(),
        real = real.display()
    );

    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, launcher).unwrap();
    set_mode(&path, 0o755);

    (home, env)
}

/// A folder of two small documents.
fn handbook(home: &Path) -> PathBuf {
    let folder = home.join("handbook");
    std::fs::create_dir_all(&folder).unwrap();

    std::fs::write(
        folder.join("gateway.md"),
        "# Gateway\n\nThe gateway routes every request to the service that owns the path.\n\n## Timeouts\n\n\
         A request that waits more than thirty seconds is cancelled.\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("runbook.txt"),
        "Restart the scheduler when the queue stops draining.\n",
    )
    .unwrap();

    folder
}

#[test]
fn a_real_server_imports_lists_searches_and_checks() {
    let Some(real) = real_binary() else { return };
    let (home, env) = scene(&real);
    let runner = TokioRunner;
    let probe = KnowledgeFeatures::new();

    // features
    let features = wait(probe.support(&env, &runner)).features();
    assert!(features.docs && features.import && features.search && features.held && features.embedder);

    // the streamed import of a folder, through the real import path
    let path = server(&env);
    let manifest = ImportManifest::new(
        Vec::new(),
        vec![handbook(home.path()).to_string_lossy().into_owned()],
        KnowledgeImportOptions::default(),
    )
    .unwrap();
    let importer = KnowledgeImporter::new();
    let run = importer.begin().unwrap();
    let mut events = Vec::new();

    let result = wait(run_import(&env, &TokioSpawner, &path, &manifest, &run, |event| {
        events.push(event)
    }))
    .unwrap();
    drop(run);

    let counts = result.counts.expect("the import ended with its done event");
    assert_eq!((counts.added, counts.failed, result.cancelled), (2, 0, false));
    assert!(matches!(events.first(), Some(KnowledgeImportEvent::Plan { total: 2 })));
    assert!(matches!(
        events.last(),
        Some(KnowledgeImportEvent::Done {
            embedding_pending, ..
        }) if *embedding_pending > 0
    ));

    let files: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            KnowledgeImportEvent::File {
                status,
                embedding,
                doc_id,
                ..
            } => Some((*status, *embedding, doc_id.is_some())),
            _ => None,
        })
        .collect();
    assert_eq!(
        files,
        vec![(KnowledgeFileStatus::Added, Some(KnowledgeEmbedding::Pending), true); 2],
        "no embedder, so the chunks wait for a reindex"
    );

    // docs list, docs get and stats
    let window = ListWindow::new(None, None, None, None).unwrap();
    let page = wait(list_docs(
        &env,
        &runner,
        &probe,
        &KnowledgeListFilter::default(),
        &window,
    ))
    .unwrap();
    assert_eq!(page.availability, KnowledgeAvailability::Available);
    assert_eq!(page.total, 2);

    let gateway = page
        .items
        .iter()
        .find(|doc| doc.title == "Gateway")
        .expect("the gateway guide");
    assert!(gateway.links.is_empty(), "an import without links is global");

    let docs = wait(docs_server(&env, &runner, &probe)).unwrap();
    let detail = wait(get_doc(&env, &runner, &docs, &gateway.id, None)).unwrap();
    assert_eq!(detail.document.id, gateway.id);
    assert!(!detail.sections.is_empty());

    let chunk = detail.sections[0].chunk_id.clone();
    let detail = wait(get_doc(&env, &runner, &docs, &gateway.id, Some(&chunk))).unwrap();
    assert!(detail.chunk.expect("the chunk asked for").text.contains("gateway"));

    // the matches inside one document
    assert!(features.doc_matches && features.routing);
    let found = wait(knowledge_doc_matches(
        &env,
        &runner,
        &docs,
        &gateway.id,
        "thirty seconds",
        None,
    ))
    .unwrap();
    assert_eq!((found.total, found.matches.len()), (1, 1));
    assert_eq!(found.matches[0].section, "Timeouts");
    assert!(found.matches[0].snippet.contains("thirty"));
    let none = wait(knowledge_doc_matches(
        &env,
        &runner,
        &docs,
        &gateway.id,
        "zzzunmatchable",
        None,
    ))
    .unwrap();
    assert_eq!((none.total, none.matches.len()), (0, 0));

    let stats = wait(read_stats(&env, &runner, &docs)).unwrap();
    assert_eq!((stats.documents, stats.embedded_chunks), (2, 0));
    assert!(stats.chunks >= 2);

    // held list is empty, and the counts follow
    let held = wait(list_knowledge_held(&env, &runner, &probe, &window));
    assert_eq!(held.availability, KnowledgeAvailability::Available);
    assert_eq!((held.total, held.items.len()), (0, 0));

    let pulse = wait(knowledge_pulse(&env, &runner, &probe)).expect("the counts");
    assert_eq!((pulse.documents, pulse.held), (2, Some(0)));
    assert_eq!(pulse.generation, stats.generation);

    // search without an embedder: keyword results only
    let hits = wait(search_knowledge(
        &env,
        &runner,
        &probe,
        "gateway routes request",
        None,
        KnowledgeSearchScope::Repo,
        None,
    ))
    .unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].doc_id, gateway.id);
    assert_eq!(hits[0].title, "Gateway");
    assert!(hits[0].chunk_id.is_valid() && hits[0].snippet.to_lowercase().contains("gateway"));

    let none = wait(search_knowledge(
        &env,
        &runner,
        &probe,
        "zzzunmatchable",
        None,
        KnowledgeSearchScope::All,
        Some(5),
    ))
    .unwrap();
    assert!(none.is_empty());

    // check: the local database is ready (the app reads this output when it tests a team database)
    let output = wait(runner.run(&ExecSpec::new(&path).args(["check", "--json"]).cwd(&env.home))).unwrap();
    let check = read_check(&output.stdout, &output.stderr, output.status, &[]);
    assert!(check.ok, "{check:?}");
    assert_eq!(check.backend.as_deref(), Some("sqlite"));
    assert!(check.problem.is_none() && check.capabilities.expect("capabilities").keyword);
}

#[test]
fn a_real_server_refuses_an_unknown_proposal_as_not_found() {
    let Some(real) = real_binary() else { return };
    let (_home, env) = scene(&real);
    let runner = TokioRunner;
    let probe = KnowledgeFeatures::new();

    let held = wait(knowledge_held_server(&env, &runner, &probe)).unwrap();
    let unknown = "00000000-0000-4000-8000-000000000000";

    let error = wait(get_knowledge_held(&env, &runner, &held, unknown)).expect_err("no such proposal");
    assert!(matches!(error, BusinessError::NotFound(_)), "{error:?}");

    for decision in [KnowledgeDecision::Approve, KnowledgeDecision::Reject] {
        let outcome = wait(decide_knowledge_held(
            &env, &runner, &held, unknown, decision, None, None,
        ))
        .unwrap();

        assert!(matches!(outcome, KnowledgeDecided::NotFound { .. }), "{outcome:?}");
    }
}

/// Has an agent propose a document the way a harness does: `knowledge_add` over the server's MCP.
fn propose(server: &Path, home: &Path) {
    let calls = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "1"}}}),
        serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "knowledge_add",
            "arguments": {"title": "Retry policy", "content": "# Retry\n\nRetry three times.", "project_path": home}}}),
    ];
    let input: String = calls.iter().map(|call| format!("{call}\n")).collect();

    let mut child = std::process::Command::new(server)
        .arg("mcp")
        .current_dir(home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("Sent to review"));
}

#[test]
fn a_real_server_holds_a_proposal_and_approves_only_the_version_shown() {
    let Some(real) = real_binary() else { return };
    let (home, env) = scene(&real);
    let runner = TokioRunner;
    let probe = KnowledgeFeatures::new();
    let path = server(&env);
    let window = ListWindow::new(None, None, None, None).unwrap();

    propose(&path, home.path());

    let held = wait(list_knowledge_held(&env, &runner, &probe, &window));
    assert_eq!((held.availability, held.total), (KnowledgeAvailability::Available, 1));

    let item = &held.items[0];
    assert_eq!(
        (item.title.as_str(), item.format.as_str(), item.client.as_str()),
        ("Retry policy", "markdown", "smoke")
    );
    assert!(item.text.contains("Retry three times") && item.links.is_empty() && item.replaces.is_none());

    let whole = wait(get_knowledge_held(&env, &runner, &path, &item.id)).unwrap();
    assert_eq!(
        (whole.revision.as_str(), whole.text.as_str()),
        (item.revision.as_str(), "# Retry\n\nRetry three times.")
    );

    let stale = "0".repeat(64);
    let outcome = wait(decide_knowledge_held(
        &env,
        &runner,
        &path,
        &item.id,
        KnowledgeDecision::Approve,
        Some(&stale),
        None,
    ))
    .unwrap();
    assert!(matches!(outcome, KnowledgeDecided::Changed { .. }), "{outcome:?}");

    let outcome = wait(decide_knowledge_held(
        &env,
        &runner,
        &path,
        &item.id,
        KnowledgeDecision::Approve,
        Some(&item.revision),
        None,
    ))
    .unwrap();
    let KnowledgeDecided::Approved {
        doc_id, status, title, ..
    } = outcome
    else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!((status, title.as_str()), (KnowledgeApproval::Added, "Retry policy"));

    let outcome = wait(decide_knowledge_held(
        &env,
        &runner,
        &path,
        &item.id,
        KnowledgeDecision::Reject,
        None,
        None,
    ))
    .unwrap();
    assert!(matches!(outcome, KnowledgeDecided::NotFound { .. }), "{outcome:?}");

    let held = wait(list_knowledge_held(&env, &runner, &probe, &window));
    assert_eq!(held.total, 0);

    let docs = wait(list_docs(
        &env,
        &runner,
        &probe,
        &KnowledgeListFilter::default(),
        &window,
    ))
    .unwrap();
    assert_eq!(
        (docs.total, docs.items[0].id.clone(), docs.items[0].origin),
        (1, doc_id, KnowledgeOrigin::AgentApproved)
    );
}

#[test]
fn a_real_server_recalls_pointers_for_a_prompt_and_nothing_for_an_unrelated_one() {
    let Some(real) = real_binary() else { return };
    let (home, env) = scene(&real);
    let runner = TokioRunner;
    let probe = KnowledgeFeatures::new();
    let path = server(&env);

    // An empty database recalls nothing, and the read does not create it.
    let prompt = "How does the gateway route every request to the service that owns the path?";
    assert_eq!(wait(knowledge_recall(&env, &runner, &probe, prompt, None)), None);

    // Without an embedder the recall reads the keyword list above a floor that a tiny corpus never
    // reaches, so the handbook sits among 60 documents about other things.
    let folder = handbook(home.path());
    for number in 0..60 {
        let text = format!("# Topic {number}\n\nNotes about subject{number} and matter{number} for the team.\n");
        std::fs::write(folder.join(format!("topic-{number}.md")), text).unwrap();
    }

    let manifest = ImportManifest::new(
        Vec::new(),
        vec![folder.to_string_lossy().into_owned()],
        KnowledgeImportOptions::default(),
    )
    .unwrap();
    let importer = KnowledgeImporter::new();
    let run = importer.begin().unwrap();
    let imported = wait(run_import(&env, &TokioSpawner, &path, &manifest, &run, |_| {})).unwrap();
    drop(run);
    assert_eq!(imported.counts.expect("the import ended").added, 62);

    let block = wait(knowledge_recall(&env, &runner, &probe, prompt, None)).expect("pointers for the gateway");
    assert!(block.starts_with("<harness-knowledge-recall>") && block.ends_with("</harness-knowledge-recall>"));
    assert!(block.contains("Gateway") && block.len() <= 1024, "{block}");

    let unrelated = "What should we order for the team lunch on Friday afternoon?";
    assert_eq!(wait(knowledge_recall(&env, &runner, &probe, unrelated, None)), None);
}

#[test]
fn a_real_server_accepts_the_routing_table_the_app_writes() {
    let Some(real) = real_binary() else { return };
    let (_home, env) = scene(&real);
    let runner = TokioRunner;
    let support = wait(KnowledgeFeatures::new().support(&env, &runner));
    assert!(support.routing());

    let saved = knowledge_remote_set(
        &env,
        KnowledgeRemoteChange {
            enabled: true,
            kind: Some(MemoryRemoteKind::Postgres),
            url: Some("postgres://ana@127.0.0.1:1/knowledge".to_owned()),
            token: None,
            repository_documents: Some(MemoryNotesRouting::RemoteLocal),
            global_documents: Some(MemoryNotesRouting::Local),
        },
        support,
    )
    .unwrap();
    assert_eq!(saved.repository_documents, MemoryNotesRouting::RemoteLocal);
    assert_eq!(
        knowledge_remote_get(&env, support).global_documents,
        MemoryNotesRouting::Local
    );

    // The server reads the file (it refuses an unknown table or value before it checks anything)
    // and says what each place in use can do.
    let check = wait(knowledge_remote_test(
        &env,
        &runner,
        MemoryRemoteKind::Postgres,
        None,
        None,
    ));
    assert!(
        check
            .places
            .as_ref()
            .is_some_and(|places| places.local.as_ref().is_some_and(|local| local.ok)),
        "{check:?}"
    );
}
