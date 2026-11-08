//! A change the server keeps on this computer while the team database cannot be reached is
//! answered `"queued": true` (a note's `title` may be `null`), and the app passes that on: the
//! note edit as [`NoteSaved::queued`](harness_business::model::NoteSaved) and a held change as
//! [`HeldSaved::Note`] with `queued`. Against the stand-in of `fake_agent`
//! (`src/bin/fake_agent/memory_user.rs`, `FAKE_MEMORY_QUEUED`).

use harness_business::ecosystem::{HeldSave, MemoryFeatures, SaveRoute, guarded_server, save_held, save_note};
use harness_business::model::{HeldSaved, NoteDraft, NoteKind, NoteSaveOutcome, NoteSaved, ScriptScope};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::memory_stand_in;

const HELD: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";

/// A server whose team database cannot be reached.
const OFFLINE: Vars = &[("FAKE_MEMORY_QUEUED", "1")];
const OFFLINE_GUARDED: Vars = &[("FAKE_MEMORY_FEATURES", "save-guards"), ("FAKE_MEMORY_QUEUED", "1")];

/// An edit of note 5 through `notes update`.
async fn edit(setup: &Setup) -> NoteSaved {
    let draft = NoteDraft {
        title: "Retry the build".to_owned(),
        body: "Written in the app.".to_owned(),
        kind: NoteKind::Fix,
        tags: vec!["Build".to_owned()],
        entities: Vec::new(),
        scope: ScriptScope::Repo,
    };

    save_note(
        &setup.env,
        &setup.scripts,
        setup.runner(),
        SaveRoute::Cli,
        Some(&setup.project()),
        &draft,
        Some(5),
        None,
        None,
    )
    .await
    .expect("saved")
}

#[tokio::test]
async fn an_edit_kept_while_the_team_database_is_unreachable_is_saved_as_queued() {
    let setup = Setup::installed(CURRENT, OFFLINE);

    let saved = edit(&setup).await;
    assert_eq!(
        (saved.outcome, saved.id, saved.queued),
        (NoteSaveOutcome::Updated, Some(5), true)
    );
    assert_eq!(serde_json::to_value(saved).unwrap()["queued"], true);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn an_answer_without_queued_is_a_plain_save() {
    let setup = Setup::installed(CURRENT, &[]);

    let saved = edit(&setup).await;
    assert_eq!((saved.id, saved.queued), (Some(5), false));
    assert_eq!(serde_json::to_value(saved).unwrap()["queued"], false);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_held_change_kept_while_the_team_database_is_unreachable_is_saved_as_queued() {
    let setup = Setup::installed(CURRENT, OFFLINE_GUARDED);
    let server = guarded_server(&setup.env, setup.runner(), &MemoryFeatures::new())
        .await
        .expect("guarded");
    let save = HeldSave {
        held_id: HELD,
        revision: None,
        edited: None,
        place: None,
    };

    let saved = save_held(&setup.env, setup.runner(), &MemoryFeatures::new(), &server, save)
        .await
        .expect("saved");

    assert_eq!(saved, HeldSaved::Note { id: 12, queued: true });
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        json!({ "kind": "note", "id": 12, "queued": true })
    );

    setup.scripts.shutdown().await;
}
