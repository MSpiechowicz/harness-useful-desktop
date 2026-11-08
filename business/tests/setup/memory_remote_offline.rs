//! The `offline_fallback` key of Harness Memory's `config.toml` (save on this computer while the
//! team database cannot be reached, on unless the file says `false`): read as on when absent,
//! written only as `false`, kept by every other save, and left alone by a server that predates it.

use crate::common;
use crate::memory_remote_support;

use common::{set_mode, write_file};
use harness_business::ecosystem::{
    FeaturesSupport, MemoryRemoteChange, MemorySupport, ScriptsSupport, memory_remote_get, memory_remote_set,
};
use harness_business::model::{MemoryNotesRouting, MemoryRemote};
use memory_remote_support::{Home, offline_features, postgres_change, with_features};
use pretty_assertions::assert_eq;

fn support(features: FeaturesSupport) -> MemorySupport {
    with_features(ScriptsSupport::Unsupported, features)
}

fn offline(wanted: Option<bool>) -> MemoryRemoteChange {
    MemoryRemoteChange {
        offline_fallback: wanted,
        ..postgres_change()
    }
}

fn text(home: &Home) -> String {
    std::fs::read_to_string(home.config()).unwrap()
}

fn read(home: &Home, features: FeaturesSupport) -> MemoryRemote {
    memory_remote_get(&home.env, support(features))
}

#[test]
fn the_fallback_is_on_without_the_key_and_reported_as_supported_by_the_server() {
    let home = Home::new();

    let saved = memory_remote_set(&home.env, offline(None), support(offline_features(true))).expect("saved");
    assert!(saved.offline_fallback);
    assert!(saved.offline_fallback_supported);
    assert_eq!(saved.problem, None);
    assert!(
        !text(&home).contains("offline_fallback"),
        "on is the server's default: {}",
        text(&home)
    );

    let older = read(&home, offline_features(false));
    assert!(
        older.offline_fallback,
        "still on for the page, which says it is unsupported"
    );
    assert!(!older.offline_fallback_supported);

    let wire = serde_json::to_value(read(&home, offline_features(true))).unwrap();
    assert_eq!(wire["offlineFallback"], true);
    assert_eq!(wire["offlineFallbackSupported"], true);
}

#[test]
fn turning_it_off_writes_false_and_turning_it_on_removes_the_key() {
    let home = Home::new();
    let listed = support(offline_features(true));

    let off = memory_remote_set(&home.env, offline(Some(false)), listed).expect("saved");
    assert!(!off.offline_fallback);
    assert!(text(&home).contains("offline_fallback = false\n"), "{}", text(&home));
    assert!(!read(&home, offline_features(true)).offline_fallback);

    let on = memory_remote_set(&home.env, offline(Some(true)), listed).expect("saved");
    assert!(on.offline_fallback);
    assert!(!text(&home).contains("offline_fallback"), "{}", text(&home));
}

#[test]
fn saving_other_settings_keeps_the_fallback_off() {
    let home = Home::new();
    let listed = support(offline_features(true));
    memory_remote_set(&home.env, offline(Some(false)), listed).expect("saved");

    let other = MemoryRemoteChange {
        repository_notes: MemoryNotesRouting::Local,
        trust_team_scripts: true,
        ..offline(None)
    };
    let saved = memory_remote_set(&home.env, other, listed).expect("saved");

    assert!(!saved.offline_fallback, "an unset choice keeps the saved one");
    assert!(text(&home).contains("offline_fallback = false\n"), "{}", text(&home));
    assert_eq!(saved.repository_notes, MemoryNotesRouting::Local);
}

#[test]
fn a_file_with_the_key_written_by_hand_is_read_and_kept() {
    let home = Home::new();
    write_file(
        &home.config(),
        "database_url = \"postgres://team@db.example.com/memory\"\noffline_fallback = false\n",
    );
    set_mode(&home.config(), 0o600);

    let remote = read(&home, offline_features(true));
    assert!(!remote.offline_fallback);
    assert_eq!(remote.problem, None, "the key is one the server knows");

    memory_remote_set(&home.env, offline(None), support(offline_features(true))).expect("saved");
    assert!(text(&home).contains("offline_fallback = false\n"), "{}", text(&home));
}

#[test]
fn a_server_that_predates_the_key_never_gets_it_written() {
    let home = Home::new();
    memory_remote_set(&home.env, offline(None), support(offline_features(true))).expect("saved");

    for features in [offline_features(false), FeaturesSupport::Unsupported] {
        let saved = memory_remote_set(&home.env, offline(Some(false)), support(features)).expect("saved");

        assert!(
            saved.offline_fallback,
            "{features:?}: the write is skipped, so it stays on"
        );
        assert!(!saved.offline_fallback_supported, "{features:?}");
        assert!(
            !text(&home).contains("offline_fallback"),
            "{features:?}: {}",
            text(&home)
        );
    }
}

#[test]
fn a_server_that_could_not_be_asked_keeps_the_saved_value_as_it_is() {
    let home = Home::new();
    memory_remote_set(&home.env, offline(Some(false)), support(offline_features(true))).expect("saved");

    for features in [FeaturesSupport::Unknown, FeaturesSupport::NotInstalled] {
        memory_remote_set(&home.env, offline(Some(true)), support(features)).expect("saved");

        assert!(
            text(&home).contains("offline_fallback = false\n"),
            "{features:?}: {}",
            text(&home)
        );
    }
}
