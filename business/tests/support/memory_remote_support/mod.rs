//! A temporary Linux home for the tests of Harness Memory's `config.toml` keys
//! (`memory_remote.rs`, `memory_remote_scripts.rs`, `memory_remote_team_docs.rs`), and the change
//! they start from.
#![allow(dead_code)]

use harness_business::ecosystem::{FeaturesSupport, MemoryRemoteChange, MemorySupport, ScriptsSupport};
use harness_business::env::{Env, OsKind};
use harness_business::model::{MemoryNotesRouting, MemoryRemoteKind};
use tempfile::TempDir;

/// The team database URL the tests save.
pub const URL: &str = "postgres://team@db.example.com/memory";

/// Saves the PostgreSQL team database [`URL`] with notes and scripts on the team and every team
/// document choice kept as saved.
pub fn postgres_change() -> MemoryRemoteChange {
    MemoryRemoteChange {
        enabled: true,
        kind: Some(MemoryRemoteKind::Postgres),
        url: Some(URL.to_owned()),
        token: None,
        repository_notes: MemoryNotesRouting::Remote,
        global_notes: MemoryNotesRouting::Remote,
        scripts: MemoryNotesRouting::Remote,
        trust_team_scripts: false,
        share_hooks: None,
        trust_team_hooks: None,
        share_routines: None,
        trust_team_routines: None,
        offline_fallback: None,
        remote_local_saves: None,
    }
}

/// The features a stand-in server lists, as far as the tests of the config file tell them apart:
/// the three notes features (`notes_remote_local`, `notes_hide`, `push_json`), the three scripts
/// features (`scripts_remote_local`, `scripts_hide`, `scripts_push_json`), `save-guards` and
/// `offline-fallback`, `remote-local-saves` and `held-save-place`. No other feature is listed.
#[derive(Debug, Clone, Copy)]
pub struct Listed {
    pub notes: [bool; 3],
    pub scripts: [bool; 3],
    pub save_guards: bool,
    pub offline_fallback: bool,
    pub remote_local_saves: bool,
    pub held_save_place: bool,
    pub notes_lang: bool,
}

/// A server that lists none of them.
pub const NOTHING: Listed = Listed {
    notes: [false; 3],
    scripts: [false; 3],
    save_guards: false,
    offline_fallback: false,
    remote_local_saves: false,
    held_save_place: false,
    notes_lang: false,
};

/// What `features` answers for a server that lists `listed`.
pub const fn features_listing(listed: Listed) -> FeaturesSupport {
    FeaturesSupport::Listed {
        notes_remote_local: listed.notes[0],
        notes_hide: listed.notes[1],
        push_json: listed.notes[2],
        scripts_remote_local: listed.scripts[0],
        scripts_hide: listed.scripts[1],
        scripts_push_json: listed.scripts[2],
        save_guards: listed.save_guards,
        save_opt_in: false,
        embedder: false,
        settings: false,
        notes_lang: listed.notes_lang,
        notes_useful: false,
        notes_authors: false,
        notes_author_filter: false,
        offline_update_queue: false,
        note_expect_revision: false,
        sync_command: false,
        offline_fallback: listed.offline_fallback,
        push_remote_local: false,
        push_selection: false,
        pull_json: false,
        scripts_pull_json: false,
        save_place: false,
        remote_local_saves: listed.remote_local_saves,
        held_save_place: listed.held_save_place,
    }
}

/// A server that lists the notes and scripts features and `save-guards`.
pub const EVERY_FEATURE: FeaturesSupport = features_listing(Listed {
    notes: [true; 3],
    scripts: [true; 3],
    save_guards: true,
    ..NOTHING
});

/// A server that lists only the routing features given: `notes_remote_local`, `notes_hide` and
/// `push_json`, then `scripts_remote_local`, `scripts_hide` and `scripts_push_json`.
pub const fn routing_features(notes: [bool; 3], scripts: [bool; 3]) -> FeaturesSupport {
    features_listing(Listed {
        notes,
        scripts,
        ..NOTHING
    })
}

/// A server that lists `offline-fallback` or not, and no other feature.
pub const fn offline_features(listed: bool) -> FeaturesSupport {
    features_listing(Listed {
        offline_fallback: listed,
        ..NOTHING
    })
}

/// A server that lists `remote-local-saves` or not, and no other feature.
pub const fn local_saves_features(listed: bool) -> FeaturesSupport {
    features_listing(Listed {
        remote_local_saves: listed,
        ..NOTHING
    })
}

/// What a server with `scripts` support and `features` supports (team documents and database kinds
/// unknown).
pub fn with_features(scripts: ScriptsSupport, features: FeaturesSupport) -> MemorySupport {
    MemorySupport {
        features,
        ..MemorySupport::from(scripts)
    }
}

/// A server that lists `notes-lang` or not, and no other feature.
pub const fn lang_features(listed: bool) -> FeaturesSupport {
    features_listing(Listed {
        notes_lang: listed,
        ..NOTHING
    })
}

/// A server that lists `held-save-place` or not, and no other feature.
pub const fn held_place_features(listed: bool) -> FeaturesSupport {
    features_listing(Listed {
        held_save_place: listed,
        ..NOTHING
    })
}

/// What a server without scripts and with `features` supports.
pub fn support_of(features: FeaturesSupport) -> MemorySupport {
    with_features(ScriptsSupport::Unsupported, features)
}

/// A home whose config file is `~/.config/harness-memory/config.toml` unless a test names another.
pub struct Home {
    _dir: TempDir,
    pub env: Env,
}

impl Home {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path()).with_os(OsKind::Linux);

        Self { _dir: dir, env }
    }

    /// The text of the config file the app wrote.
    pub fn config_text(&self) -> String {
        std::fs::read_to_string(self.config()).unwrap()
    }

    /// The config file the app writes (`HARNESS_MEMORY_CONFIG` when the test sets it).
    pub fn config(&self) -> std::path::PathBuf {
        harness_business::ecosystem::memory_config_path(&self.env).expect("a config path")
    }
}
