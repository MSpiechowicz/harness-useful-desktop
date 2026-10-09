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
use crate::pi_family::sessions::{canonical_text, configured_session_dir, pi_dir_name};
use crate::pi_family::{read_json, text_field};

/// The folder holding the session files of `project`.
pub fn pi_sessions_dir(env: &Env, project: &Path) -> PathBuf {
    let paths = pi_paths(env, Some(project));

    let setting = |file: PathBuf| {
        read_json(&file)
            .value()
            .and_then(|settings| text_field(settings, "sessionDir"))
            .filter(|raw| !raw.trim().is_empty())
    };

    let configured = env
        .var("PI_CODING_AGENT_SESSION_DIR")
        .map(str::to_owned)
        .or_else(|| paths.project_dir.and_then(|dir| setting(dir.join("settings.json"))))
        .or_else(|| setting(paths.agent_dir.join("settings.json")));

    match configured {
        Some(raw) => configured_session_dir(env, project, &raw),
        None => paths
            .agent_dir
            .join("sessions")
            .join(pi_dir_name(&canonical_text(project))),
    }
}
