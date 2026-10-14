//! Where PI keeps a project's sessions (PI docs `sessions.md`, `settings.md`,
//! `session-format.md`).
//!
//! A configured session directory is the folder itself: `PI_CODING_AGENT_SESSION_DIR`, else the
//! `sessionDir` setting of the project (`.pi/settings.json`, which PI reads before project trust
//! is decided), else the user's. Relative values resolve against the project. Without one,
//! sessions live in `<agent dir>/sessions/--<project>--` (see
//! [`pi_dir_name`](crate::pi_family::sessions::pi_dir_name)).

use std::path::{Path, PathBuf};

use super::pi_paths;
use crate::env::Env;
use crate::model::SessionFolder;
use crate::pi_family::sessions::{canonical_text, configured_session_dir, pi_dir_name};
use crate::pi_family::{read_json, text_field};

/// The folder holding the session files of `project`.
pub fn pi_sessions_dir(env: &Env, project: &Path) -> PathBuf {
    resolve(env, project).0
}

/// [`pi_sessions_dir`] for display; `configured` when the project's `sessionDir` chose it.
///
/// A configured folder shows where it really is (canonical when it exists), so a committed
/// `sessionDir` that is a symlink into another folder cannot pass for one inside the project.
pub fn pi_session_folder(env: &Env, project: &Path) -> SessionFolder {
    let (mut path, configured) = resolve(env, project);

    if configured && let Ok(canonical) = dunce::canonicalize(&path) {
        path = canonical;
    }

    SessionFolder {
        path: path.to_string_lossy().into_owned(),
        configured,
    }
}

/// The session folder and whether the project's own settings chose it.
fn resolve(env: &Env, project: &Path) -> (PathBuf, bool) {
    let paths = pi_paths(env, Some(project));

    let setting = |file: PathBuf| {
        read_json(&file)
            .value()
            .and_then(|settings| text_field(settings, "sessionDir"))
            .filter(|raw| !raw.trim().is_empty())
    };

    if let Some(raw) = env.var("PI_CODING_AGENT_SESSION_DIR") {
        return (configured_session_dir(env, project, raw), false);
    }

    if let Some(raw) = paths.project_dir.and_then(|dir| setting(dir.join("settings.json"))) {
        return (configured_session_dir(env, project, &raw), true);
    }

    if let Some(raw) = setting(paths.agent_dir.join("settings.json")) {
        return (configured_session_dir(env, project, &raw), false);
    }

    let default = paths
        .agent_dir
        .join("sessions")
        .join(pi_dir_name(&canonical_text(project)));

    (default, false)
}
