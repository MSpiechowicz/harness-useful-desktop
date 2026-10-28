//! What the app understands of `harness-memory embedder` and `settings`: the short commands against
//! the stand-in of `fake_agent` (see `src/bin/fake_agent/memory_embedder.rs`), and the `--lang`
//! filter of the notes list, which is asked only of a server that lists `notes-lang`. The lines the
//! embedder prints folded into one state are `tracker`'s.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    FeaturesSupport, ListWindow, MemoryFeatures, NotesQuery, Repos, embedder_status, feature_server, get_settings,
    list_notes, remove_embedder_model, set_embedder_enabled, set_settings,
};
use harness_business::model::{MemoryScriptsAvailability, NoteLangFilter, NoteSummary};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

use crate::memory_stand_in;

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

    let status = embedder_status(&setup.env, setup.runner(), &server).await.unwrap();
    assert!(status.enabled);
    assert_eq!(status.model_present, Some(false));

    let answers: [(Vars, Option<bool>); 2] = [
        (&[("FAKE_EMBEDDER_MODEL", "present")], Some(true)),
        (&[("FAKE_EMBEDDER_MODEL", "unknown")], None),
    ];

    for (vars, expected) in answers {
        let other = Setup::installed(CURRENT, vars);
        let status = embedder_status(&other.env, other.runner(), &other.binary)
            .await
            .unwrap();

        assert_eq!(status.model_present, expected, "{vars:?}");
    }
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
        useful: None,
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
        notes_useful: false,
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

mod tracker;
