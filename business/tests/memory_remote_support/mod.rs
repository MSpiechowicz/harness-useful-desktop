//! A temporary Linux home for the tests of Harness Memory's `config.toml` keys
//! (`memory_remote_scripts.rs`, `memory_remote_hooks.rs`).
#![allow(dead_code)]

use harness_business::env::{Env, OsKind};
use tempfile::TempDir;

/// The team database URL the tests save.
pub const URL: &str = "postgres://team@db.example.com/memory";

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
