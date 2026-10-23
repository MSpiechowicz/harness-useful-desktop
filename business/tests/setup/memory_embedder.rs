//! What the app understands of `harness-memory embedder` and `settings`: the lines the embedder
//! prints folded into one state, the short commands against the stand-in of `fake_agent` (see
//! `src/bin/fake_agent/memory_embedder.rs`), and the `--lang` filter of the notes list, which is
//! asked only of a server that lists `notes-lang`.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    EmbedderEvent, EmbedderTracker, FeaturesSupport, ListWindow, MemoryFeatures, NotesQuery, Repos, embedder_status,
    feature_server, get_settings, list_notes, parse_event, remove_embedder_model, set_embedder_enabled, set_settings,
};
use harness_business::model::{
    EmbedderPhase, MemoryEmbedderState, MemoryScriptsAvailability, NoteLangFilter, NoteSummary,
};
use memory_stand_in::{CURRENT, Setup};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::memory_stand_in;

fn tracker_after(lines: &[&str]) -> EmbedderTracker {
    let mut tracker = EmbedderTracker::new();
    tracker.starting();

    for line in lines {
        tracker.apply(parse_event(line).unwrap_or_else(|| panic!("an event: {line}")));
    }

    tracker
}

fn shown(tracker: &EmbedderTracker) -> (EmbedderPhase, Option<f64>) {
    (tracker.state().phase, tracker.state().progress)
}

#[test]
fn every_line_the_embedder_prints_is_an_event_and_anything_else_is_nothing() {
    let cases = [
        (
            r#"{"event":"download","received":50,"total":200}"#,
            EmbedderEvent::Download {
                received: 50,
                total: 200,
            },
        ),
        (r#"{"event":"verifying"}"#, EmbedderEvent::Verifying),
        (r#"{"event":"loading"}"#, EmbedderEvent::Loading),
        (
            r#"{"event":"ready","model":"m1","dim":384}"#,
            EmbedderEvent::Ready { model: "m1".to_owned() },
        ),
        (
            r#"{"event":"backfill","store":"local:ab","done":3,"total":9}"#,
            EmbedderEvent::Backfill {
                store: "local:ab".to_owned(),
                done: 3,
                total: 9,
            },
        ),
        (
            r#"{"event":"model-update","from":"m1","to":"m2"}"#,
            EmbedderEvent::ModelUpdate,
        ),
        (
            r#"{"event":"reembed","store":"local:ab","done":1,"total":2}"#,
            EmbedderEvent::Reembed {
                store: "local:ab".to_owned(),
                done: 1,
                total: 2,
            },
        ),
        (
            r#"{"event":"switched","model":"m2"}"#,
            EmbedderEvent::Switched { model: "m2".to_owned() },
        ),
        (r#"{"event":"cleanup-done"}"#, EmbedderEvent::CleanupDone),
        (r#"{"event":"idle"}"#, EmbedderEvent::Idle),
        (
            r#"{"event":"error","message":"no disk"}"#,
            EmbedderEvent::Error {
                message: "no disk".to_owned(),
            },
        ),
        (r#"{"event":"busy"}"#, EmbedderEvent::Busy),
    ];

    for (line, event) in cases {
        assert_eq!(parse_event(line), Some(event), "{line}");
    }

    for line in [
        "",
        "not json",
        r#"{"event":"from-the-future"}"#,
        r#"{"no":"event"}"#,
        r#"[1]"#,
    ] {
        assert_eq!(parse_event(line), None, "{line:?}");
    }
}

#[test]
fn the_download_the_load_and_the_backfill_add_up_to_one_state() {
    let mut tracker = EmbedderTracker::new();
    assert_eq!(tracker.state(), &MemoryEmbedderState::default());
    assert!(tracker.starting());
    assert_eq!(shown(&tracker), (EmbedderPhase::Starting, None));

    let step = |tracker: &mut EmbedderTracker, line: &str| {
        let changed = tracker.apply(parse_event(line).unwrap());
        (changed, shown(tracker))
    };

    assert_eq!(
        step(&mut tracker, r#"{"event":"download","received":1,"total":4}"#),
        (true, (EmbedderPhase::Downloading, Some(0.25)))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"download","received":1,"total":4}"#),
        (false, (EmbedderPhase::Downloading, Some(0.25))),
        "the same state is not a change"
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"download","received":2,"total":0}"#),
        (true, (EmbedderPhase::Downloading, None)),
        "no total, no fraction"
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"verifying"}"#),
        (true, (EmbedderPhase::Verifying, None))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"loading"}"#),
        (true, (EmbedderPhase::Loading, None))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"ready","model":"m1","dim":384}"#),
        (true, (EmbedderPhase::Ready, None))
    );
    assert_eq!(tracker.state().model.as_deref(), Some("m1"));

    // Two stores count together, and the embedder is ready again once both are through.
    assert_eq!(
        step(&mut tracker, r#"{"event":"backfill","store":"a","done":0,"total":6}"#),
        (true, (EmbedderPhase::Backfilling, Some(0.0)))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"backfill","store":"b","done":2,"total":2}"#),
        (true, (EmbedderPhase::Backfilling, Some(0.25)))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"backfill","store":"a","done":6,"total":6}"#),
        (true, (EmbedderPhase::Ready, None))
    );
}

#[test]
fn a_newer_model_keeps_the_embedder_updating_until_it_switched() {
    let mut tracker = tracker_after(&[
        r#"{"event":"ready","model":"m1","dim":384}"#,
        r#"{"event":"model-update","from":"m1","to":"m2"}"#,
        r#"{"event":"download","received":1,"total":2}"#,
    ]);
    assert_eq!(shown(&tracker), (EmbedderPhase::Updating, Some(0.5)));

    tracker.apply(parse_event(r#"{"event":"reembed","store":"a","done":1,"total":4}"#).unwrap());
    assert_eq!(shown(&tracker), (EmbedderPhase::Updating, Some(0.25)));
    assert_eq!(
        tracker.state().model.as_deref(),
        Some("m1"),
        "the old model serves meanwhile"
    );

    tracker.apply(parse_event(r#"{"event":"switched","model":"m2"}"#).unwrap());
    assert_eq!(shown(&tracker), (EmbedderPhase::Ready, None));
    assert_eq!(tracker.state().model.as_deref(), Some("m2"));

    // A failed move leaves the old model serving, and says why.
    tracker.apply(parse_event(r#"{"event":"model-update","from":"m2","to":"m3"}"#).unwrap());
    tracker.apply(parse_event(r#"{"event":"error","message":"checksum mismatch"}"#).unwrap());
    assert_eq!(shown(&tracker), (EmbedderPhase::Ready, None));
    assert_eq!(tracker.state().error.as_deref(), Some("checksum mismatch"));
}

#[test]
fn an_error_fails_the_embedder_and_busy_hands_it_to_another_instance() {
    let failed = tracker_after(&[
        r#"{"event":"loading"}"#,
        r#"{"event":"error","message":"model is corrupt"}"#,
    ]);
    assert_eq!(shown(&failed), (EmbedderPhase::Failed, None));
    assert_eq!(failed.state().error.as_deref(), Some("model is corrupt"));
    assert!(!failed.is_busy());

    let elsewhere = tracker_after(&[r#"{"event":"busy"}"#]);
    assert_eq!(shown(&elsewhere), (EmbedderPhase::Elsewhere, None));
    assert!(elsewhere.is_busy());

    let mut tracker = tracker_after(&[r#"{"event":"ready","model":"m1","dim":384}"#]);
    assert!(tracker.stopped());
    assert_eq!(
        tracker.state(),
        &MemoryEmbedderState::default(),
        "off forgets the model"
    );
}

#[test]
fn an_error_message_is_redacted_before_it_is_shown() {
    let secret = ["ghp_", &"a1B2".repeat(9)].concat();
    let line = json!({ "event": "error", "message": format!("cannot reach https://u:{secret}@host/x") }).to_string();

    let tracker = tracker_after(&[&line]);

    let message = tracker.state().error.clone().unwrap();
    assert!(!message.contains(&secret), "{message}");
}

#[tokio::test]
async fn the_short_commands_run_through_the_installed_server_that_lists_the_features() {
    let setup = Setup::installed(
        CURRENT,
        &[("FAKE_EMBEDDER_ENABLED", "true"), ("FAKE_ENGLISH_NOTES", "false")],
    );
    let features = MemoryFeatures::new();
    let server = feature_server(
        &setup.env,
        setup.runner(),
        &features,
        FeaturesSupport::embedder,
        "the embedder",
    )
    .await
    .expect("the embedder is listed");
    assert_eq!(server, setup.binary);

    assert!(
        embedder_status(&setup.env, setup.runner(), &server)
            .await
            .unwrap()
            .enabled
    );
    set_embedder_enabled(&setup.env, setup.runner(), &server, false)
        .await
        .unwrap();
    remove_embedder_model(&setup.env, setup.runner(), &server)
        .await
        .unwrap();

    assert!(
        !get_settings(&setup.env, setup.runner(), &server)
            .await
            .unwrap()
            .english_notes
    );
    assert!(
        set_settings(&setup.env, setup.runner(), &server, true)
            .await
            .unwrap()
            .english_notes
    );

    let commands: Vec<Vec<String>> = (0..setup.runs().len()).map(|run| setup.run_args(run)).collect();
    assert_eq!(
        commands[1..],
        [
            vec!["embedder", "status", "--json"],
            vec!["embedder", "disable"],
            vec!["embedder", "remove-model"],
            vec!["settings", "get", "--json"],
            vec!["settings", "set", "english_notes=true", "--json"],
        ]
    );
}

#[tokio::test]
async fn a_server_without_the_feature_or_without_an_install_is_unsupported_or_not_found() {
    let older = Setup::installed(CURRENT, &[("FAKE_MEMORY_FEATURES", "notes-hide")]);
    let features = MemoryFeatures::new();

    for (feature, what) in [
        (
            FeaturesSupport::embedder as fn(FeaturesSupport) -> bool,
            "vector search",
        ),
        (FeaturesSupport::settings, "settings"),
    ] {
        let refused = feature_server(&older.env, older.runner(), &features, feature, what).await;
        assert!(
            matches!(refused, Err(BusinessError::Unsupported(_))),
            "{what}: {refused:?}"
        );
    }

    let absent = Setup::without_server(CURRENT, &[]);
    let refused = feature_server(
        &absent.env,
        absent.runner(),
        &MemoryFeatures::new(),
        FeaturesSupport::settings,
        "settings",
    )
    .await;
    assert!(matches!(refused, Err(BusinessError::NotFound(_))), "{refused:?}");

    let server = &older.binary;
    let refused = get_settings(&older.env, older.runner(), server).await;
    assert!(
        matches!(refused, Err(BusinessError::Unsupported(_))),
        "the usage error of an older server: {refused:?}"
    );
}

#[tokio::test]
async fn a_refused_model_removal_comes_back_with_the_servers_line() {
    let setup = Setup::installed(CURRENT, &[("FAKE_EMBEDDER_REMOVE", "refuse")]);

    let refused = remove_embedder_model(&setup.env, setup.runner(), &setup.binary).await;

    assert!(
        matches!(&refused, Err(BusinessError::Process(message)) if message.contains("the embedder is running")),
        "{refused:?}"
    );
}

fn query(lang: Option<NoteLangFilter>) -> NotesQuery {
    NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang,
        sort: None,
        hidden: false,
    }
}

#[tokio::test]
async fn the_lang_filter_goes_to_the_server_only_when_asked() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();
    let list = |query: NotesQuery| {
        let (env, scripts, runner, project) = (&setup.env, &setup.scripts, setup.runner(), &project);

        async move { list_notes(env, scripts, runner, Repos::Project(Some(project)), &query).await }
    };

    let everything = list(query(None)).await;
    assert_eq!(everything.availability, MemoryScriptsAvailability::Available);
    assert!(!setup.run_args(0).contains(&"--lang".to_owned()));
    assert!(
        everything.notes.iter().all(|note: &NoteSummary| note.lang.is_none()),
        "a fixture has no language yet"
    );

    let english = list(query(Some(NoteLangFilter::En))).await;
    let args = setup.run_args(1);
    assert_eq!(english.total, 0, "no fixture is English");
    assert_eq!(args[args.iter().position(|arg| arg == "--lang").unwrap() + 1], "en");

    let unknown = list(query(Some(NoteLangFilter::Unknown))).await;
    assert_eq!(unknown.total, everything.total);
}

#[tokio::test]
async fn a_server_that_lists_no_lang_cannot_be_asked_for_one() {
    let setup = Setup::installed(CURRENT, &[("FAKE_MEMORY_FEATURES", "notes-hide")]);
    let project = setup.project();

    let refused = list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(Some(&project)),
        &query(Some(NoteLangFilter::Other)),
    )
    .await;
    assert_eq!(
        refused.availability,
        MemoryScriptsAvailability::Outdated,
        "the usage error of an older server"
    );

    let page = list_notes(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        Repos::Project(Some(&project)),
        &query(None),
    )
    .await;
    assert_eq!(
        page.availability,
        MemoryScriptsAvailability::Available,
        "without it the list keeps working"
    );
    assert!(page.notes.iter().all(|note| note.lang.is_none()));
}

#[test]
fn a_query_keeps_its_lang_only_for_a_server_that_lists_notes_lang() {
    let asked = query(Some(NoteLangFilter::Other));
    let listed = |notes_lang| FeaturesSupport::Listed {
        notes_remote_local: false,
        notes_hide: false,
        push_json: false,
        scripts_remote_local: false,
        scripts_hide: false,
        scripts_push_json: false,
        save_guards: false,
        embedder: false,
        settings: false,
        notes_lang,
    };

    assert_eq!(asked.for_server(listed(true)), asked);
    assert_eq!(asked.for_server(listed(false)), query(None));

    for other in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert_eq!(asked.for_server(other).lang, None, "{other:?}");
    }
}
