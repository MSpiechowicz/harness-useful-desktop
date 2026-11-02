//! The streamed import: `harness-knowledge docs import --ndjson` gets its manifest on stdin, every
//! event it prints goes to the sink as it arrives, an import without its closing event ends as a
//! failure with the server's last line, one import runs at a time, and a cancel asks the child to
//! stop (SIGTERM, or ending it on Windows) and kills it when it does not. The child is an in-memory
//! peer: nothing is forked.

use crate::knowledge_stand_in;

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use harness_business::BusinessError;
use harness_business::ecosystem::{CANCEL_GRACE, ImportManifest, KnowledgeImporter, parse_import_event, run_import};
use harness_business::env::Env;
use harness_business::model::{
    KnowledgeEmbedding, KnowledgeFileStatus, KnowledgeImportCounts, KnowledgeImportEvent, KnowledgeImportOptions,
    KnowledgeImportResult, KnowledgeLink,
};
use harness_business::process::ExitInfo;
use harness_business::testing::{DuplexSpawner, FakePeer};
use knowledge_stand_in::{REPO, installed, line, play_import, server, small_import};
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedReceiver;

const EXITED_0: ExitInfo = ExitInfo {
    code: Some(0),
    signal: None,
};
const EXITED_1: ExitInfo = ExitInfo {
    code: Some(1),
    signal: None,
};

fn manifest() -> ImportManifest {
    ImportManifest::new(
        vec!["/docs/a.md".to_owned(), "/docs/b.pdf".to_owned()],
        vec!["/docs/handbook".to_owned()],
        KnowledgeImportOptions {
            links: vec![KnowledgeLink {
                repo: REPO.to_owned(),
                globs: vec!["src/**".to_owned()],
            }],
            replaces: None,
            replacing: Vec::new(),
            place: None,
        },
    )
    .unwrap()
}

/// An installed server, a spawner that hands its child to the test, and the importer.
struct Scene {
    _dir: tempfile::TempDir,
    env: Env,
    path: PathBuf,
    manifest: ImportManifest,
    spawner: DuplexSpawner,
    peers: UnboundedReceiver<FakePeer>,
    importer: KnowledgeImporter,
}

impl Scene {
    fn new() -> Self {
        Self::with_importer(KnowledgeImporter::new())
    }

    fn with_importer(importer: KnowledgeImporter) -> Self {
        let (dir, env) = installed();
        let (spawner, peers) = DuplexSpawner::new();

        Self {
            path: server(&env),
            _dir: dir,
            env,
            manifest: manifest(),
            spawner,
            peers,
            importer,
        }
    }

    /// Runs an import beside `child`, which plays the process it spawned, and answers the import's
    /// result with every event it forwarded and what `child` returned. `on_event` sees each event
    /// first, with the importer (to cancel it).
    async fn import<T, F: Future<Output = T>>(
        &mut self,
        child: impl FnOnce(FakePeer) -> F,
        mut on_event: impl FnMut(&KnowledgeImporter, &KnowledgeImportEvent),
    ) -> (
        Result<KnowledgeImportResult, BusinessError>,
        Vec<KnowledgeImportEvent>,
        T,
    ) {
        let run = self.importer.begin().unwrap();
        let importer = &self.importer;
        let mut events = Vec::new();

        let imported = run_import(&self.env, &self.spawner, &self.path, &self.manifest, &run, |event| {
            on_event(importer, &event);
            events.push(event);
        });
        let played = async { child(self.peers.recv().await.unwrap()).await };
        let (result, played) = tokio::join!(imported, played);

        (result, events, played)
    }
}

#[tokio::test]
async fn the_manifest_reaches_stdin_and_every_event_is_forwarded_in_order() {
    let mut scene = Scene::new();

    let (result, events, (spec, written)) = scene
        .import(
            |peer| async move { play_import(peer, &small_import(), "", EXITED_0).await },
            |_, _| {},
        )
        .await;
    let result = result.unwrap();

    assert_eq!(spec.program, scene.path);
    assert_eq!(spec.args, ["docs", "import", "--ndjson"]);
    assert_eq!(spec.cwd.as_deref(), Some(scene.env.home.as_path()));
    assert_eq!(
        written,
        json!({
            "paths": ["/docs/a.md", "/docs/b.pdf"],
            "folders": ["/docs/handbook"],
            "links": [{ "repo": REPO, "globs": ["src/**"] }]
        })
    );

    assert_eq!(events.len(), 6);
    assert_eq!(events[0], KnowledgeImportEvent::Plan { total: 2 });
    assert_eq!(
        events[1],
        KnowledgeImportEvent::File {
            path: "/docs/a.md".to_owned(),
            status: KnowledgeFileStatus::Added,
            reason: None,
            doc_id: Some(7.into()),
            chunks: Some(3),
            embedding: Some(KnowledgeEmbedding::Done),
            warnings: vec![],
        }
    );
    assert!(matches!(
        &events[3],
        KnowledgeImportEvent::File { status: KnowledgeFileStatus::Failed, reason: Some(reason), .. } if reason == "no text layer"
    ));
    assert_eq!(
        result.counts,
        Some(KnowledgeImportCounts {
            added: 1,
            updated: 0,
            unchanged: 0,
            skipped: 0,
            failed: 1
        })
    );
    assert!(!result.cancelled);
}

#[test]
fn lines_that_are_no_event_are_skipped_and_a_done_line_is_read_whole() {
    for stray in [
        "",
        "not json",
        "{\"type\":\"newer\"}",
        "{\"type\":\"file\",\"path\":\"/a\",\"status\":\"odd\"}",
        "[1]",
    ] {
        assert_eq!(parse_import_event(stray), None, "{stray:?}");
    }

    let pending = parse_import_event(
        &json!({"type": "file", "path": "/a.md", "status": "updated", "chunks": 0, "embedding": "pending", "warnings": ["w"]}).to_string(),
    )
    .unwrap();
    assert!(matches!(
        pending,
        KnowledgeImportEvent::File { embedding: Some(KnowledgeEmbedding::Pending), chunks: Some(0), ref warnings, .. } if warnings == &["w"]
    ));

    let done = parse_import_event(
        &json!({"type": "done", "counts": {"added": 2, "unchanged": 5}, "cancelled": true, "counter": "approximate", "embeddingPending": 9}).to_string(),
    )
    .unwrap();
    assert_eq!(
        done,
        KnowledgeImportEvent::Done {
            counts: KnowledgeImportCounts {
                added: 2,
                updated: 0,
                unchanged: 5,
                skipped: 0,
                failed: 0
            },
            cancelled: true,
            counter: "approximate".to_owned(),
            embedding_pending: 9,
        }
    );
}

#[tokio::test]
async fn an_import_without_its_done_event_ends_with_the_servers_last_line() {
    let mut scene = Scene::new();
    let stderr = "warning: slow disk\nharness-knowledge: the manifest must be JSON like {\"paths\": [...]}\n";

    let (result, events, _) = scene
        .import(|peer| play_import(peer, &[], stderr, EXITED_1), |_, _| {})
        .await;

    let result = result.unwrap();
    assert_eq!(result.counts, None);
    assert!(!result.cancelled);
    assert_eq!(
        events,
        [KnowledgeImportEvent::Failed {
            message: "the manifest must be JSON like {\"paths\": [...]}".to_owned()
        }]
    );
}

#[tokio::test]
async fn one_import_runs_at_a_time_and_a_cancel_needs_one() {
    let importer = KnowledgeImporter::new();

    assert!(!importer.cancel());
    assert!(!importer.is_running());

    let first = importer.begin().unwrap();
    assert!(importer.is_running());
    assert!(
        matches!(importer.begin(), Err(BusinessError::InvalidInput(message)) if message.contains("already running"))
    );

    drop(first);
    assert!(!importer.is_running());
    assert!(importer.begin().is_ok());
}

#[tokio::test]
async fn a_cancel_asks_the_child_to_stop_and_keeps_what_was_done() {
    let mut scene = Scene::new();

    let child = |mut peer: FakePeer| async move {
        let manifest = knowledge_stand_in::read_manifest(&mut peer).await;
        assert!(manifest["paths"].is_array());

        for line in small_import().iter().take(3) {
            peer.stdout.write_all(line.as_bytes()).await.unwrap();
        }

        peer.terminated().await;
        assert!(!peer.is_killed(), "it is asked first");

        let done = json!({"type": "done", "counts": {"added": 1}, "cancelled": true, "counter": "tokenizer", "embeddingPending": 0});
        peer.stdout.write_all(line(&done).as_bytes()).await.unwrap();
        peer.stdout.shutdown().await.unwrap();
        peer.stderr.shutdown().await.unwrap();
        peer.exit(EXITED_0);
    };
    // The user cancels once the first file is done.
    let cancel_after_the_first = |importer: &KnowledgeImporter, event: &KnowledgeImportEvent| {
        if matches!(event, KnowledgeImportEvent::Progress { done: 1, .. }) {
            assert!(importer.cancel());
        }
    };

    let (result, events, ()) = scene.import(child, cancel_after_the_first).await;

    let result = result.unwrap();
    assert!(result.cancelled);
    assert_eq!(result.counts.map(|counts| counts.added), Some(1));
    assert!(matches!(
        events.last(),
        Some(KnowledgeImportEvent::Done { cancelled: true, .. })
    ));
}

#[tokio::test]
async fn a_child_that_ignores_the_request_is_killed_after_the_grace() {
    let mut scene = Scene::with_importer(KnowledgeImporter::with_grace(Duration::from_millis(50)));
    let child = |mut peer: FakePeer| async move {
        knowledge_stand_in::read_manifest(&mut peer).await;
        peer.stdout.write_all(small_import()[0].as_bytes()).await.unwrap();
        peer.terminated().await;

        // Its output stays open until the app kills it, as a hung process's would.
        while !peer.is_killed() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert!(CANCEL_GRACE >= Duration::from_secs(1), "the real grace is long");
        drop(peer.stdout);
        drop(peer.stderr);
    };
    let cancel_at_the_plan = |importer: &KnowledgeImporter, _: &KnowledgeImportEvent| {
        importer.cancel();
    };

    let (result, events, ()) = scene.import(child, cancel_at_the_plan).await;

    let result = result.unwrap();
    assert!(result.cancelled);
    assert_eq!(result.counts, None);
    assert!(matches!(events.last(), Some(KnowledgeImportEvent::Failed { .. })));
}

#[tokio::test]
async fn a_child_that_does_not_start_is_an_error_and_a_huge_manifest_is_refused() {
    let scene = Scene::new();
    let run = scene.importer.begin().unwrap();
    scene.spawner.fail_next_spawn("no such file");

    let error = run_import(&scene.env, &scene.spawner, &scene.path, &scene.manifest, &run, |_| {})
        .await
        .expect_err("not started");
    assert!(matches!(error, BusinessError::Process(message) if message == "Harness Knowledge could not be started"));

    let huge = ImportManifest::new(
        vec!["a".repeat(5 * 1024 * 1024)],
        vec![],
        KnowledgeImportOptions::default(),
    )
    .unwrap();
    assert!(matches!(huge.to_json(), Err(BusinessError::InvalidInput(_))));
}

#[test]
fn a_manifest_refuses_links_the_server_would_refuse_and_replaces_without_exactly_one_file() {
    let bad_link = KnowledgeImportOptions {
        links: vec![KnowledgeLink {
            repo: "not a key".to_owned(),
            globs: Vec::new(),
        }],
        replaces: None,
        replacing: Vec::new(),
        place: None,
    };
    let error = ImportManifest::new(vec!["/docs/a.md".to_owned()], vec![], bad_link).expect_err("bad link");
    assert!(matches!(error, BusinessError::InvalidInput(_)));

    let replaces = KnowledgeImportOptions {
        links: vec![],
        replaces: Some(7.into()),
        replacing: Vec::new(),
        place: None,
    };
    for (paths, folders) in [
        (vec!["/docs/a.md", "/docs/b.md"], vec![]),
        (vec!["/docs/a.md"], vec!["/docs/handbook"]),
        (vec![], vec!["/docs/handbook"]),
        (vec![], vec![]),
    ] {
        let paths = paths.into_iter().map(str::to_owned).collect();
        let folders = folders.into_iter().map(str::to_owned).collect();
        let error = ImportManifest::new(paths, folders, replaces.clone()).expect_err("not one file");
        assert!(matches!(error, BusinessError::InvalidInput(_)));
    }

    let one = ImportManifest::new(vec!["/docs/a.md".to_owned()], vec![], replaces).unwrap();
    assert_eq!(one.replaces, Some(7.into()));
}

#[test]
fn a_place_and_a_local_replaces_id_are_written_into_the_manifest_as_given() {
    use harness_business::model::KnowledgeId;
    use harness_business::model::KnowledgePlace;

    let options = KnowledgeImportOptions {
        links: vec![],
        replaces: Some(KnowledgeId::Local("local:12".to_owned())),
        replacing: Vec::new(),
        place: Some(KnowledgePlace::Local),
    };
    let one = ImportManifest::new(vec!["/docs/a.md".to_owned()], vec![], options).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&one.to_json().unwrap()).unwrap();

    assert_eq!(json["place"], "local");
    assert_eq!(json["replaces"], "local:12", "a local id stays a string");
    assert!(
        manifest()
            .to_json()
            .is_ok_and(|bytes| !String::from_utf8_lossy(&bytes).contains("place"))
    );

    let bad = KnowledgeImportOptions {
        links: vec![],
        replaces: Some(KnowledgeId::Local("local:x".to_owned())),
        replacing: Vec::new(),
        place: None,
    };
    assert!(ImportManifest::new(vec!["/docs/a.md".to_owned()], vec![], bad).is_err());
}
