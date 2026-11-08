//! The note routing keys of Harness Memory's `config.toml` take a third value, `remote+local`
//! (read the team database and this computer, save here), for `repository_notes` and
//! `global_notes` (and `scripts`, see `memory_remote_scripts_routing.rs`). It is saved only while the installed server lists every feature it needs
//! in `harness-memory features`: an older server would refuse the file.

use crate::common;
use crate::memory_remote_support;

use common::{set_mode, write_file};
use harness_business::BusinessError;
use harness_business::ecosystem::{
    FeaturesSupport, MemoryRemoteChange, MemorySupport, ScriptsSupport, memory_remote_get, memory_remote_set,
};
use harness_business::model::{MemoryNotesRouting, MemoryRemote};
use memory_remote_support::{EVERY_FEATURE, Home, postgres_change, routing_features, with_features};
use pretty_assertions::assert_eq;

fn support(features: FeaturesSupport) -> MemorySupport {
    with_features(ScriptsSupport::Unsupported, features)
}

fn routed(repository_notes: MemoryNotesRouting, global_notes: MemoryNotesRouting) -> MemoryRemoteChange {
    MemoryRemoteChange {
        repository_notes,
        global_notes,
        ..postgres_change()
    }
}

/// Helpers of these tests on the shared [`Home`].
trait RoutingFile {
    fn text(&self) -> String;

    /// Writes the file by hand (private, as the server wants it).
    fn write(&self, text: &str);

    fn read(&self) -> MemoryRemote;
}

impl RoutingFile for Home {
    fn text(&self) -> String {
        std::fs::read_to_string(self.config()).unwrap()
    }

    fn write(&self, text: &str) {
        write_file(&self.config(), text);
        set_mode(&self.config(), 0o600);
    }

    fn read(&self) -> MemoryRemote {
        memory_remote_get(&self.env, support(EVERY_FEATURE))
    }
}

#[test]
fn team_notes_with_local_saves_are_written_and_read_back_per_scope() {
    let home = Home::new();
    let change = routed(MemoryNotesRouting::RemoteLocal, MemoryNotesRouting::Remote);

    let saved = memory_remote_set(&home.env, change, support(EVERY_FEATURE)).expect("saved");
    assert_eq!(saved.repository_notes, MemoryNotesRouting::RemoteLocal);
    assert_eq!(saved.global_notes, MemoryNotesRouting::Remote);
    assert!(saved.team_and_local_supported);
    assert_eq!(saved.problem, None);

    let text = home.text();
    assert!(text.contains("repository_notes = \"remote+local\"\n"), "{text}");
    assert!(text.contains("global_notes = \"remote\"\n"), "{text}");

    let change = routed(MemoryNotesRouting::Local, MemoryNotesRouting::RemoteLocal);
    memory_remote_set(&home.env, change, support(EVERY_FEATURE)).expect("saved");
    assert_eq!(
        (home.read().repository_notes, home.read().global_notes),
        (MemoryNotesRouting::Local, MemoryNotesRouting::RemoteLocal)
    );

    let wire = serde_json::to_value(home.read()).unwrap();
    assert_eq!(wire["globalNotes"], "remoteLocal");
    assert_eq!(wire["teamAndLocalSupported"], true);
}

#[test]
fn a_server_without_every_feature_never_gets_remote_local_written() {
    let home = Home::new();
    memory_remote_set(&home.env, postgres_change(), support(EVERY_FEATURE)).expect("saved");
    let before = home.text();

    let partial = routing_features([true, true, false], [true, true, true]);

    for (features, wording) in [
        (partial, "Update Harness Memory"),
        (FeaturesSupport::Unsupported, "Update Harness Memory"),
        (FeaturesSupport::NotInstalled, "Install Harness Memory"),
        (FeaturesSupport::Unknown, "could not say"),
    ] {
        for change in [
            routed(MemoryNotesRouting::RemoteLocal, MemoryNotesRouting::Remote),
            routed(MemoryNotesRouting::Local, MemoryNotesRouting::RemoteLocal),
        ] {
            let refused = memory_remote_set(&home.env, change, support(features));

            assert!(
                matches!(&refused, Err(BusinessError::Unsupported(reason)) if reason.contains(wording)),
                "{features:?}: {refused:?}"
            );
            assert_eq!(home.text(), before, "nothing is written");
        }

        let remote = memory_remote_get(&home.env, support(features));
        assert!(!remote.team_and_local_supported, "{features:?}");

        let two_valued = routed(MemoryNotesRouting::Local, MemoryNotesRouting::Remote);
        memory_remote_set(&home.env, two_valued, support(features)).expect("remote and local stay possible");
        memory_remote_set(&home.env, postgres_change(), support(features)).expect("saved");
    }
}

#[test]
fn the_note_keys_take_remote_local_and_hooks_do_not() {
    let home = Home::new();

    home.write("database_url = \"postgres://h/db\"\nglobal_notes = \"remote+local\"\n");
    let remote = home.read();
    assert_eq!(remote.global_notes, MemoryNotesRouting::RemoteLocal);
    assert_eq!(remote.repository_notes, MemoryNotesRouting::Remote, "unset is remote");
    assert_eq!(remote.problem, None);

    home.write("database_url = \"postgres://h/db\"\nrepository_notes = \"team\"\n");
    let remote = home.read();
    assert_eq!(remote.repository_notes, MemoryNotesRouting::Remote);
    assert_eq!(
        remote.problem.as_deref(),
        Some("repository_notes must be \"remote\", \"local\" or \"remote+local\"")
    );

    home.write("database_url = \"postgres://h/db\"\nhooks = \"remote+local\"\n");
    assert_eq!(
        home.read().problem.as_deref(),
        Some("hooks must be \"remote\" or \"local\""),
        "hooks and routines stay two-valued"
    );
}

#[test]
fn the_vector_search_lines_reach_the_page_as_their_own_flags() {
    let home = Home::new();
    let listing = |embedder, settings, notes_lang| FeaturesSupport::Listed {
        notes_remote_local: false,
        notes_hide: false,
        push_json: false,
        scripts_remote_local: false,
        scripts_hide: false,
        scripts_push_json: false,
        save_guards: false,
        save_opt_in: false,
        embedder,
        settings,
        notes_lang,
        notes_useful: false,
        notes_authors: false,
        notes_author_filter: false,
        offline_update_queue: false,
        note_expect_revision: false,
        sync_command: false,
        offline_fallback: false,
        push_remote_local: false,
        push_selection: false,
        pull_json: false,
        scripts_pull_json: false,
        save_place: false,
        remote_local_saves: false,
        held_save_place: false,
    };
    let flags = |features| {
        let remote = memory_remote_get(&home.env, support(features));

        (
            remote.embedder_supported,
            remote.settings_supported,
            remote.notes_lang_supported,
        )
    };

    assert_eq!(flags(listing(true, true, true)), (true, true, true));
    assert_eq!(flags(listing(false, true, false)), (false, true, false));
    assert_eq!(flags(listing(true, false, true)), (true, false, true));

    for features in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert_eq!(flags(features), (false, false, false), "{features:?}");
    }
}

#[test]
fn the_notes_authors_line_reaches_the_page_as_its_own_flag() {
    let home = Home::new();
    let listing = |notes_authors| FeaturesSupport::Listed {
        notes_remote_local: false,
        notes_hide: false,
        push_json: false,
        scripts_remote_local: false,
        scripts_hide: false,
        scripts_push_json: false,
        save_guards: false,
        save_opt_in: false,
        embedder: false,
        settings: false,
        notes_lang: false,
        notes_useful: false,
        notes_authors,
        notes_author_filter: false,
        offline_update_queue: false,
        note_expect_revision: false,
        sync_command: false,
        offline_fallback: false,
        push_remote_local: false,
        push_selection: false,
        pull_json: false,
        scripts_pull_json: false,
        save_place: false,
        remote_local_saves: false,
        held_save_place: false,
    };
    let flag = |features| memory_remote_get(&home.env, support(features)).notes_authors_supported;

    assert!(flag(listing(true)));
    assert!(!flag(listing(false)));

    for features in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert!(!flag(features), "{features:?}");
    }
}

#[test]
fn the_sync_buttons_need_push_selection_next_to_the_copy_lines() {
    let home = Home::new();
    let listing = |push_remote_local, push_selection| FeaturesSupport::Listed {
        notes_remote_local: false,
        notes_hide: false,
        push_json: false,
        scripts_remote_local: false,
        scripts_hide: false,
        scripts_push_json: false,
        save_guards: false,
        save_opt_in: false,
        embedder: false,
        settings: false,
        notes_lang: false,
        notes_useful: false,
        notes_authors: false,
        notes_author_filter: false,
        offline_update_queue: false,
        note_expect_revision: false,
        sync_command: false,
        offline_fallback: false,
        push_remote_local,
        push_selection,
        pull_json: true,
        scripts_pull_json: true,
        save_place: false,
        remote_local_saves: false,
        held_save_place: false,
    };
    let flags = |features| {
        let remote = memory_remote_get(&home.env, support(features));

        (remote.notes_sync_supported, remote.scripts_sync_supported)
    };

    assert_eq!(flags(listing(true, true)), (true, true));
    assert_eq!(flags(listing(true, false)), (false, false));
    assert_eq!(flags(listing(false, true)), (false, false));
}
