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
    }
}

/// A server that lists every feature the desktop reads in `harness-memory features`.
pub const EVERY_FEATURE: FeaturesSupport = FeaturesSupport::Listed {
    notes_remote_local: true,
    notes_hide: true,
    push_json: true,
    scripts_remote_local: true,
    scripts_hide: true,
    scripts_push_json: true,
    save_guards: true,
    embedder: false,
    settings: false,
    notes_lang: false,
    notes_useful: false,
    code_index: false,
};

/// What a server with `scripts` support and `features` supports (team documents and database kinds
/// unknown).
pub fn with_features(scripts: ScriptsSupport, features: FeaturesSupport) -> MemorySupport {
    MemorySupport {
        features,
        ..MemorySupport::from(scripts)
    }
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

    /// The config file the app writes (`HARNESS_MEMORY_CONFIG` when the test sets it).
    pub fn config(&self) -> std::path::PathBuf {
        harness_business::ecosystem::memory_config_path(&self.env).expect("a config path")
    }
}
