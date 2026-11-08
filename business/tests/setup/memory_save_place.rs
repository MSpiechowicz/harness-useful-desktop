//! Where a new note or script goes while its scope is routed to "Remote + this computer": `--place`
//! of `notes save` and `scripts save` on a server that lists `save-place`, and what the app does
//! when it cannot take one. Against the stand-in of `fake_agent`
//! (`src/bin/fake_agent/memory_user.rs`, `FAKE_MEMORY_REMOTE_DOWN`).

use harness_business::BusinessError;
use harness_business::ecosystem::{MemoryFeatures, SaveRoute, save_note};
use harness_business::model::{
    MemorySavePlace, NoteDraft, NoteKind, NoteSaveOutcome, NoteSaved, ScriptSaveOutcome, ScriptSaved, ScriptScope,
};
use memory_stand_in::{CURRENT, Setup, Vars, last_args, script};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::memory_stand_in;

const PLACING: Vars = &[("FAKE_MEMORY_FEATURES", "save-guards,save-place")];
const PLACING_REMOTE_DOWN: Vars = &[
    ("FAKE_MEMORY_FEATURES", "save-guards,save-place"),
    ("FAKE_MEMORY_REMOTE_DOWN", "1"),
];
const GUARDS: Vars = &[("FAKE_MEMORY_FEATURES", "save-guards")];
const GUARDS_REVISIONS_PLACE: Vars = &[("FAKE_MEMORY_FEATURES", "save-guards,note-expect-revision,save-place")];
const PLACE_ONLY: Vars = &[("FAKE_MEMORY_FEATURES", "save-place")];

fn draft(scope: ScriptScope) -> NoteDraft {
    NoteDraft {
        title: "Retry the build".to_owned(),
        body: "Written in the app.".to_owned(),
        kind: NoteKind::Fix,
        tags: Vec::new(),
        entities: Vec::new(),
        scope,
    }
}

/// A save of a note on `route`, an edit of note `replace` when given.
async fn note(
    setup: &Setup,
    route: SaveRoute,
    scope: ScriptScope,
    replace: Option<i64>,
    place: Option<MemorySavePlace>,
) -> Result<NoteSaved, BusinessError> {
    save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        route,
        Some(&setup.project()),
        &draft(scope),
        replace,
        None,
        place,
    )
    .await
}

/// A save of script `fresh-one` on `route`, a replace when `replace`.
async fn script_save(
    setup: &Setup,
    route: SaveRoute,
    replace: bool,
    place: Option<MemorySavePlace>,
) -> Result<ScriptSaved, BusinessError> {
    setup
        .scripts
        .save(
            &setup.env,
            setup.runner(),
            route,
            Some(&setup.project()),
            &script(if replace { "open-prs" } else { "fresh-one" }),
            replace,
            None,
            place,
        )
        .await
}

fn save_args(setup: &Setup, kind: &str) -> Vec<String> {
    let mut args: Vec<String> = [kind, "save", "--json", "--client", "harness-desktop", "--project"]
        .map(String::from)
        .into();
    args.push(setup.project().to_string_lossy().into_owned());

    args
}

#[tokio::test]
async fn a_new_note_names_its_place_only_when_it_was_given_one() {
    let setup = Setup::installed(CURRENT, PLACING);

    let plain = note(&setup, SaveRoute::CliPlace, ScriptScope::Global, None, None)
        .await
        .expect("saved");
    assert_eq!(
        (plain.outcome, plain.id, plain.place),
        (NoteSaveOutcome::Created, Some(77), None)
    );
    assert_eq!(last_args(&setup), save_args(&setup, "notes"));

    for (place, word) in [(MemorySavePlace::Remote, "remote"), (MemorySavePlace::Local, "local")] {
        let saved = note(&setup, SaveRoute::CliPlace, ScriptScope::Global, None, Some(place))
            .await
            .expect("saved");

        assert_eq!((saved.id, saved.place), (Some(77), Some(place)));
        assert_eq!(serde_json::to_value(saved).unwrap()["place"], json!(word));

        let mut expected = save_args(&setup, "notes");
        expected.extend(["--place".to_owned(), word.to_owned()]);
        assert_eq!(last_args(&setup), expected);
    }

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_place_for_an_edit_is_refused_and_nothing_runs() {
    let setup = Setup::installed(CURRENT, PLACING);

    let refused = note(
        &setup,
        SaveRoute::CliPlace,
        ScriptScope::Repo,
        Some(5),
        Some(MemorySavePlace::Local),
    )
    .await;

    assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");

    let replace = script_save(&setup, SaveRoute::CliPlace, true, Some(MemorySavePlace::Remote)).await;
    assert!(matches!(replace, Err(BusinessError::InvalidInput(_))), "{replace:?}");
    assert_eq!(setup.runs().len(), 0, "no command ran");

    // The same edit without a place saves as before.
    let edited = note(&setup, SaveRoute::CliPlace, ScriptScope::Repo, Some(5), None)
        .await
        .expect("edited");
    assert_eq!(
        (edited.outcome, edited.id, edited.place),
        (NoteSaveOutcome::Updated, Some(5), None)
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_save_place_cannot_take_one() {
    let setup = Setup::installed(CURRENT, GUARDS);

    for route in [SaveRoute::Cli, SaveRoute::CliExpectRevision, SaveRoute::Mcp] {
        let refused = note(&setup, route, ScriptScope::Global, None, Some(MemorySavePlace::Remote)).await;
        assert!(
            matches!(refused, Err(BusinessError::Unsupported(_))),
            "{route:?}: {refused:?}"
        );

        let refused = script_save(&setup, route, false, Some(MemorySavePlace::Local)).await;
        assert!(
            matches!(refused, Err(BusinessError::Unsupported(_))),
            "{route:?}: {refused:?}"
        );
    }

    assert_eq!(setup.runs().len(), 0, "no command ran");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn the_features_the_server_lists_pick_the_route_that_places() {
    async fn route_of(vars: Vars) -> SaveRoute {
        let setup = Setup::installed(CURRENT, vars);
        let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;

        SaveRoute::of(features)
    }

    assert_eq!(route_of(PLACING).await, SaveRoute::CliPlace);
    assert_eq!(
        route_of(GUARDS_REVISIONS_PLACE).await,
        SaveRoute::CliExpectRevisionPlace
    );
    assert_eq!(
        route_of(PLACE_ONLY).await,
        SaveRoute::Mcp,
        "no user commands to carry it"
    );
    assert!(SaveRoute::CliExpectRevisionPlace.places() && SaveRoute::CliExpectRevisionPlace.expects_revision());
    assert!(!SaveRoute::CliExpectRevision.places());
}

#[tokio::test]
async fn a_remote_that_cannot_be_reached_fails_the_save_with_a_redacted_message() {
    let setup = Setup::installed(CURRENT, PLACING_REMOTE_DOWN);

    let failed = note(
        &setup,
        SaveRoute::CliPlace,
        ScriptScope::Global,
        None,
        Some(MemorySavePlace::Remote),
    )
    .await
    .expect_err("fails");

    let BusinessError::Process(message) = &failed else {
        panic!("not a process failure: {failed:?}");
    };
    assert!(message.contains("cannot be reached"), "{message}");
    assert!(!message.contains("hunter2"), "the password is redacted: {message}");

    let failed = script_save(&setup, SaveRoute::CliPlace, false, Some(MemorySavePlace::Remote))
        .await
        .expect_err("fails");
    assert!(
        matches!(&failed, BusinessError::Process(message) if message.contains("cannot be reached") && !message.contains("hunter2")),
        "{failed:?}"
    );

    // This computer does not need the remote.
    let local = note(
        &setup,
        SaveRoute::CliPlace,
        ScriptScope::Global,
        None,
        Some(MemorySavePlace::Local),
    )
    .await
    .expect("saved here");
    assert_eq!(local.place, Some(MemorySavePlace::Local));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_new_script_names_its_place_and_is_answered_where_it_went() {
    let setup = Setup::installed(CURRENT, PLACING);

    for (place, word) in [(MemorySavePlace::Remote, "remote"), (MemorySavePlace::Local, "local")] {
        let saved = script_save(&setup, SaveRoute::CliPlace, false, Some(place))
            .await
            .expect("saved");

        assert_eq!(
            (saved.outcome, saved.id, saved.place),
            (ScriptSaveOutcome::Replaced, 12, Some(place))
        );
        assert!(saved.script.is_none(), "the stand-in cannot read the new script again");
        assert_eq!(serde_json::to_value(&saved).unwrap()["place"], json!(word));

        let mut expected = save_args(&setup, "scripts");
        expected.extend(["--place".to_owned(), word.to_owned()]);
        assert_eq!(last_args(&setup), expected);
    }

    setup.scripts.shutdown().await;
}
