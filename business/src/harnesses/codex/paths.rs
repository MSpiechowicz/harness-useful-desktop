//! Codex directory resolution (Codex CLI 0.154).
//!
//! - Config, data and sessions: `$CODEX_HOME`, else `~/.codex`, on every OS. A relative
//!   `CODEX_HOME` is ignored (Codex would resolve it against its working directory).
//! - Sessions: `<home>/sessions/YYYY/MM/DD/rollout-*.jsonl` for every project together.
//! - Project layer: `<project>/.codex`.

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::harness::HarnessPaths;

/// The default home directory name below the user's home, and the project layer's name.
pub const PROJECT_DIR_NAME: &str = ".codex";

/// Resolves every Codex directory for `env` (see the module docs). Settings, skills, plugins and
/// sessions all live in the Codex home; its profiles are sections of `config.toml`, not folders.
pub fn paths(env: &Env, project: Option<&Path>) -> HarnessPaths {
    let home = codex_home(env);

    HarnessPaths {
        config_root: home.clone(),
        agent_dir: home.clone(),
        data_root: home,
        profile: None,
        project_dir: project.map(|project| project.join(PROJECT_DIR_NAME)),
    }
}

/// The Codex home: the absolute `CODEX_HOME`, else `~/.codex`.
pub fn codex_home(env: &Env) -> PathBuf {
    env.var("CODEX_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| env.home_join(PROJECT_DIR_NAME))
}

/// The folder holding the session files of every project: `<home>/sessions`.
pub fn sessions_dir(env: &Env) -> PathBuf {
    codex_home(env).join("sessions")
}
