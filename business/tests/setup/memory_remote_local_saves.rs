//! The `remote_local_saves` key of Harness Memory's `config.toml` (where agents save new notes and
//! scripts of a scope routed to "Remote + this computer"): read as remote when absent, written only
//! as `local`, kept by every other save, and left alone by a server that predates it.

use crate::common;
use crate::memory_remote_support;

use common::{set_mode, write_file};
use harness_business::ecosystem::{FeaturesSupport, MemoryRemoteChange, memory_remote_get, memory_remote_set};
use harness_business::model::{MemoryNotesRouting, MemoryRemote, MemorySavePlace};
use memory_remote_support::{Home, local_saves_features, postgres_change, support_of};
use pretty_assertions::assert_eq;

fn saving(wanted: Option<MemorySavePlace>) -> MemoryRemoteChange {
    MemoryRemoteChange {
        remote_local_saves: wanted,
        ..postgres_change()
    }
}

fn read(home: &Home, features: FeaturesSupport) -> MemoryRemote {
    memory_remote_get(&home.env, support_of(features))
}

#[test]
fn saves_go_to_the_team_without_the_key_and_the_support_reaches_the_page() {
    let home = Home::new();

    let saved = memory_remote_set(&home.env, saving(None), support_of(local_saves_features(true))).expect("saved");
    assert_eq!(saved.remote_local_saves, MemorySavePlace::Remote);
    assert!(saved.remote_local_saves_supported);
    assert_eq!(saved.problem, None);
    assert!(
        !home.config_text().contains("remote_local_saves"),
        "remote is the server's default: {}",
        home.config_text()
    );

    let older = read(&home, local_saves_features(false));
    assert_eq!(older.remote_local_saves, MemorySavePlace::Remote);
    assert!(!older.remote_local_saves_supported);

    let wire = serde_json::to_value(read(&home, local_saves_features(true))).unwrap();
    assert_eq!(wire["remoteLocalSaves"], "remote");
    assert_eq!(wire["remoteLocalSavesSupported"], true);
}

#[test]
fn local_is_written_read_back_and_removed_again_by_remote() {
    let home = Home::new();
    let listed = support_of(local_saves_features(true));

    let local = memory_remote_set(&home.env, saving(Some(MemorySavePlace::Local)), listed).expect("saved");
    assert_eq!(local.remote_local_saves, MemorySavePlace::Local);
    assert!(
        home.config_text().contains("remote_local_saves = \"local\"\n"),
        "{}",
        home.config_text()
    );
    assert_eq!(
        read(&home, local_saves_features(true)).remote_local_saves,
        MemorySavePlace::Local
    );
    assert_eq!(serde_json::to_value(&local).unwrap()["remoteLocalSaves"], "local");

    let remote = memory_remote_set(&home.env, saving(Some(MemorySavePlace::Remote)), listed).expect("saved");
    assert_eq!(remote.remote_local_saves, MemorySavePlace::Remote);
    assert!(
        !home.config_text().contains("remote_local_saves"),
        "{}",
        home.config_text()
    );
}

#[test]
fn saving_other_settings_keeps_local() {
    let home = Home::new();
    let listed = support_of(local_saves_features(true));
    memory_remote_set(&home.env, saving(Some(MemorySavePlace::Local)), listed).expect("saved");

    let other = MemoryRemoteChange {
        repository_notes: MemoryNotesRouting::Local,
        trust_team_scripts: true,
        ..saving(None)
    };
    let saved = memory_remote_set(&home.env, other, listed).expect("saved");

    assert_eq!(
        saved.remote_local_saves,
        MemorySavePlace::Local,
        "an unset choice keeps the saved one"
    );
    assert!(
        home.config_text().contains("remote_local_saves = \"local\"\n"),
        "{}",
        home.config_text()
    );
    assert_eq!(saved.repository_notes, MemoryNotesRouting::Local);
}

#[test]
fn a_file_with_the_key_written_by_hand_is_read_and_kept() {
    let home = Home::new();
    write_file(
        &home.config(),
        "database_url = \"postgres://team@db.example.com/memory\"\nremote_local_saves = \"local\"\n",
    );
    set_mode(&home.config(), 0o600);

    let remote = read(&home, local_saves_features(true));
    assert_eq!(remote.remote_local_saves, MemorySavePlace::Local);
    assert_eq!(remote.problem, None, "the key is one the server knows");

    memory_remote_set(&home.env, saving(None), support_of(local_saves_features(true))).expect("saved");
    assert!(
        home.config_text().contains("remote_local_saves = \"local\"\n"),
        "{}",
        home.config_text()
    );
}

#[test]
fn a_value_the_server_would_refuse_is_a_problem_and_reads_as_remote() {
    let home = Home::new();
    write_file(
        &home.config(),
        "database_url = \"postgres://team@db.example.com/memory\"\nremote_local_saves = \"both\"\n",
    );
    set_mode(&home.config(), 0o600);

    let remote = read(&home, local_saves_features(true));

    assert_eq!(remote.remote_local_saves, MemorySavePlace::Remote);
    assert!(
        remote
            .problem
            .is_some_and(|problem| problem.contains("remote_local_saves"))
    );
}

#[test]
fn a_server_that_predates_the_key_never_gets_it_written() {
    let home = Home::new();
    memory_remote_set(&home.env, saving(None), support_of(local_saves_features(true))).expect("saved");

    for features in [local_saves_features(false), FeaturesSupport::Unsupported] {
        let saved =
            memory_remote_set(&home.env, saving(Some(MemorySavePlace::Local)), support_of(features)).expect("saved");

        assert_eq!(
            saved.remote_local_saves,
            MemorySavePlace::Remote,
            "{features:?}: the write is skipped, so it stays remote"
        );
        assert!(!saved.remote_local_saves_supported, "{features:?}");
        assert!(
            !home.config_text().contains("remote_local_saves"),
            "{features:?}: {}",
            home.config_text()
        );
    }
}

#[test]
fn a_server_that_could_not_be_asked_keeps_the_saved_value_as_it_is() {
    let home = Home::new();
    memory_remote_set(
        &home.env,
        saving(Some(MemorySavePlace::Local)),
        support_of(local_saves_features(true)),
    )
    .expect("saved");

    for features in [FeaturesSupport::Unknown, FeaturesSupport::NotInstalled] {
        memory_remote_set(&home.env, saving(Some(MemorySavePlace::Remote)), support_of(features)).expect("saved");

        assert!(
            home.config_text().contains("remote_local_saves = \"local\"\n"),
            "{features:?}: {}",
            home.config_text()
        );
    }
}
