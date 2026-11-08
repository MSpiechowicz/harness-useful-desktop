//! Where a held note or script goes when the user saves it from the approval card: `--place` of
//! `held save` on a server that lists `held-save-place`, what the app does when it cannot take one,
//! and what the server says about a held `note_update`. Against the stand-in of `fake_agent`
//! (`src/bin/fake_agent/memory_user.rs`).

use crate::memory_remote_support;
use crate::memory_stand_in;

use harness_business::BusinessError;
use harness_business::ecosystem::{HeldSave, MemoryFeatures, guarded_server, memory_remote_get, save_held};
use harness_business::model::{HeldSaved, MemorySavePlace};
use memory_remote_support::{Home, held_place_features, support_of};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

const HELD: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";
const HELD_UPDATE: &str = "9d4e6f5b-2e1b-4c0d-9f8a-8b7c6d5e4f30";

const PLACING: Vars = &[("FAKE_MEMORY_FEATURES", "save-guards,held-save-place")];
const GUARDS: Vars = &[("FAKE_MEMORY_FEATURES", "save-guards")];

/// A save of `held_id` to `place`, through a server of `vars`.
async fn save(setup: &Setup, held_id: &str, place: Option<MemorySavePlace>) -> Result<HeldSaved, BusinessError> {
    let features = MemoryFeatures::new();
    let server = guarded_server(&setup.env, setup.runner(), &features)
        .await
        .expect("guarded");

    let held = HeldSave {
        held_id,
        revision: None,
        edited: None,
        place,
    };

    save_held(&setup.env, setup.runner(), &features, &server, held).await
}

/// The commands run that were a `held save`.
fn held_saves(setup: &Setup) -> Vec<Vec<String>> {
    (0..setup.runs().len())
        .map(|run| setup.run_args(run))
        .filter(|args| args.starts_with(&["held".to_owned(), "save".to_owned()]))
        .collect()
}

#[tokio::test]
async fn a_held_note_is_saved_to_the_place_the_user_picked() {
    let setup = Setup::installed(CURRENT, PLACING);

    for (place, name) in [(MemorySavePlace::Local, "local"), (MemorySavePlace::Remote, "remote")] {
        let saved = save(&setup, HELD, Some(place)).await.expect("saved");

        assert_eq!(saved, HeldSaved::Note { id: 12, queued: false });
        assert_eq!(
            held_saves(&setup).last().unwrap().as_slice(),
            ["held", "save", HELD, "--json", "--place", name]
        );
    }

    save(&setup, HELD, None).await.expect("saved without one");
    assert_eq!(
        held_saves(&setup).last().unwrap().as_slice(),
        ["held", "save", HELD, "--json"],
        "no place asked, none passed"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_without_held_save_place_is_not_asked_to_take_one() {
    let setup = Setup::installed(CURRENT, GUARDS);

    let refused = save(&setup, HELD, Some(MemorySavePlace::Local)).await;
    assert!(matches!(refused, Err(BusinessError::Unsupported(_))), "{refused:?}");
    assert_eq!(held_saves(&setup).len(), 0, "no held save ran");

    save(&setup, HELD, None)
        .await
        .expect("a save without a place still works");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_held_note_update_takes_no_place() {
    let setup = Setup::installed(CURRENT, PLACING);

    let refused = save(&setup, HELD_UPDATE, Some(MemorySavePlace::Remote)).await;
    assert!(
        matches!(refused, Err(BusinessError::InvalidInput(ref text)) if text.contains("note_update")),
        "{refused:?}"
    );

    let saved = save(&setup, HELD_UPDATE, None).await.expect("saved");
    assert_eq!(saved, HeldSaved::Note { id: 4, queued: false });

    setup.scripts.shutdown().await;
}

#[test]
fn the_held_save_place_line_reaches_the_page_as_its_own_flag() {
    let home = Home::new();
    let flag = |features| memory_remote_get(&home.env, support_of(features)).held_save_place_supported;

    assert!(flag(held_place_features(true)));
    assert!(!flag(held_place_features(false)));
    assert_eq!(
        serde_json::to_value(memory_remote_get(&home.env, support_of(held_place_features(true)))).unwrap()["heldSavePlaceSupported"],
        true
    );
}
