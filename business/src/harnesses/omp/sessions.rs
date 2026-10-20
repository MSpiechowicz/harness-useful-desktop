//! Where OMP keeps a project's sessions, mirroring `session/session-paths.ts`,
//! `pi-utils/src/dirs.ts` and `cli/args.ts` (18.6.1).
//!
//! - `PI_CODING_AGENT_SESSION_DIR` is OMP's `--session-dir`: the folder itself.
//! - Otherwise `<sessions root>/<encoded project>` (see
//!   [`omp_dir_name`](crate::pi_family::sessions::omp_dir_name)). The root is
//!   `<XDG data>/sessions` when OMP stores data under `$XDG_DATA_HOME/omp` (Linux and macOS, default
//!   agent dir, directory exists), else `<agent dir>/sessions`.

use std::path::{Path, PathBuf};

use super::paths::omp_paths;
use crate::env::{Env, OsKind};
use crate::error::Result;
use crate::pi_family::sessions::{
    SessionStore, canonical_text, configured_session_dir, configured_store, omp_dir_name, temp_root,
};

/// The root of OMP's per-project session folders.
pub fn omp_sessions_root(env: &Env) -> PathBuf {
    let paths = omp_paths(env, None);

    let profile_root = match &paths.profile {
        Some(name) => paths.config_root.join("profiles").join(name),
        None => paths.config_root.clone(),
    };

    // `data_root` differs from the profile root only when OMP uses XDG data, which flattens
    // the `agent/` prefix (`$XDG_DATA_HOME/omp/sessions`).
    if paths.data_root == profile_root {
        paths.agent_dir.join("sessions")
    } else {
        paths.data_root.join("sessions")
    }
}

/// The folder holding the session files of `project`.
pub fn omp_sessions_dir(env: &Env, project: &Path) -> PathBuf {
    if let Some(raw) = env.var("PI_CODING_AGENT_SESSION_DIR") {
        return configured_session_dir(env, project, raw);
    }

    let name = omp_dir_name(
        &canonical_text(project),
        &canonical_text(&env.home),
        &canonical_text(&temp_root(env)),
        env.os == OsKind::Windows,
    );

    omp_sessions_root(env).join(name)
}

/// Where OMP keeps the session files of every project: the configured session directory, else
/// one folder per project below [`omp_sessions_root`].
pub fn omp_session_store(env: &Env) -> Result<SessionStore> {
    if let Some(raw) = env.var("PI_CODING_AGENT_SESSION_DIR") {
        return configured_store(env, raw);
    }

    Ok(SessionStore::PerProject(omp_sessions_root(env)))
}
