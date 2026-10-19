//! Routines: small JSON files in a shared folder that say when something runs (a daily time, every
//! N minutes, a cron expression, app start) and what runs (an agent chat with a prompt, a shell
//! command or a saved Harness Memory script). `docs/routines-format.md` is the file contract. The
//! app window schedules and runs routines; its MCP server (`harness-desktop mcp`) writes routine
//! files for agents through the same [`RoutineStore`] but never runs them.
//!
//! Routines are one kind of signed document ([`RoutineKind`]): the folder, the approvals and the
//! team copies are [`crate::signed_docs`]'s, this module adds what a routine is and when it runs.
//!
//! - [`store`]: the folder ([`routines_root`]), reading and validating the files ([`format`]),
//!   listing them with their status ([`view`]), and writes and deletes by the app window or an
//!   agent. A routine runs only while the SHA-256 of its exact file bytes is approved.
//! - [`cron`] and [`when`]: the schedules, as slots in the machine's time zone (daylight saving
//!   gaps and folds included), and the preview of the next runs.
//! - [`plan`]: which routines are due at a moment (pure), with catch-up after the machine slept and
//!   a clock set backwards; [`schedule_state`] keeps what was handled in
//!   `<app_local_data_dir>/routine-schedule.json`.
//! - [`runs`]: the run records per routine (`routine-runs/<id>.json`) and their pages;
//!   [`session_tags`]: which harness session a routine run started (`routine-sessions.json`).
//! - [`team`]: the team routines shared through Harness Memory, a catalogue each machine turns
//!   on (`routines-state.json`, [`state`]).
//!
//! The scheduler, the runners and the notifications live in the app window (`src-tauri`).

pub mod cron;
mod draft;
mod files;
pub mod format;
pub mod plan;
pub mod runs;
pub mod schedule_state;
pub mod session_tags;
pub mod state;
pub mod store;
pub mod team;
pub mod view;
pub mod when;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub use format::{MAX_ROUTINE_FILE_BYTES, parse_routine_file, routine_bytes};
pub use plan::{Due, Launch, Scheduled, due, forget_unplanned, next_wake, scheduled_from};
pub use runs::RunStore;
pub use schedule_state::ScheduleState;
pub use session_tags::SessionTags;
pub use store::{MAX_ROUTINES_PER_SCOPE, RoutineStore};
pub use when::{local_zone, schedule_preview};

pub use crate::signed_docs::{project_identity, sha256_hex};

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link};
use crate::model::{HookOrigin, Routine};
use crate::signed_docs::team::TEAM_FOLDER;
use crate::signed_docs::{DocKind, doc_root, is_doc_id, unsafe_folder, unsafe_root};

/// `HARNESS_ROUTINES_DIR`: overrides the routines root (absolute, a leading `~` expanded).
pub const DIR_VAR: &str = "HARNESS_ROUTINES_DIR";

/// The folder under the platform base.
pub const APP_FOLDER: &str = "harness-routines";

/// The folder inside the root that holds the routine files.
pub const ROUTINES_FOLDER: &str = "routines";

/// The folder inside the root that holds the folders global routines run in (an agent or a
/// command has no project): one per routine, `<root>/workspace/<id>` for a local routine and
/// `<root>/workspace/team-<id>` for a team routine ([`workspace_name`]).
pub const WORKSPACE_FOLDER: &str = "workspace";

/// What the folder of a team routine in the workspace starts with.
pub const TEAM_WORKSPACE_PREFIX: &str = "team-";

/// The approvals file in the app's config folder.
pub const APPROVALS_FILE: &str = "routine-approvals.json";

/// The folder of approved copies in the app's config folder.
pub const SNAPSHOT_FOLDER: &str = "routine-approved";

/// The team routines' local state in the app's config folder.
pub const STATE_FILE: &str = "routines-state.json";

/// Routines as a kind of signed document (see [`crate::signed_docs`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoutineKind;

impl DocKind for RoutineKind {
    const NOUN: &'static str = "routine";
    const PLURAL: &'static str = ROUTINES_FOLDER;
    const ROOT_VAR: &'static str = DIR_VAR;
    const APP_FOLDER: &'static str = APP_FOLDER;
    const MAX_FILE_BYTES: usize = MAX_ROUTINE_FILE_BYTES;
    const MAX_PER_SCOPE: usize = MAX_ROUTINES_PER_SCOPE;
    const APPROVALS_FILE: &'static str = APPROVALS_FILE;
    const SNAPSHOT_FOLDER: &'static str = SNAPSHOT_FOLDER;
    const STATE_FILE: &'static str = STATE_FILE;
    const TRUST_KEY: &'static str = "trustTeamRoutines";
    const SEPARATE_TEAM_APPROVALS: bool = true;

    type Doc = Routine;

    fn parse(id: &str, bytes: &[u8]) -> std::result::Result<Routine, String> {
        parse_routine_file(id, bytes)
    }

    fn parse_team(id: &str, bytes: &[u8]) -> std::result::Result<Routine, String> {
        format::parse_team_routine(id, bytes)
    }

    fn to_bytes(routine: &Routine, base: Map<String, Value>) -> Vec<u8> {
        routine_bytes(routine, base)
    }

    fn forget_team_state(config_dir: &Path, ids: &[String]) -> Result<()> {
        team::forget_team_entries(config_dir, ids)
    }
}

/// The routines root: [`DIR_VAR`] when it is absolute (after `~` expansion), else the platform
/// data folder followed by [`APP_FOLDER`] (see [`doc_root`]). `None` on Windows without
/// `LOCALAPPDATA`: routines are then unavailable.
pub fn routines_root(env: &Env) -> Option<PathBuf> {
    doc_root::<RoutineKind>(env)
}

/// `<root>/workspace`, the folder that holds the working folders of global agent and command
/// routines.
pub fn workspace_dir(root: &Path) -> PathBuf {
    root.join(WORKSPACE_FOLDER)
}

/// The name of the folder in `<root>/workspace` the global routine `id` of `origin` runs in: the
/// id for a local routine, `team-<id>` for a team one, so a local and a team routine with the same
/// id never share a folder.
pub fn workspace_name(id: &str, origin: HookOrigin) -> String {
    match origin {
        HookOrigin::Local => id.to_owned(),
        HookOrigin::Team => format!("{TEAM_WORKSPACE_PREFIX}{id}"),
    }
}

/// The routine id and origin a workspace folder `name` belongs to; `None` for any other name.
pub fn workspace_owner(name: &str) -> Option<(&str, HookOrigin)> {
    let (id, origin) = match name.strip_prefix(TEAM_WORKSPACE_PREFIX) {
        Some(id) => (id, HookOrigin::Team),
        None => (name, HookOrigin::Local),
    };

    is_doc_id(id).then_some((id, origin))
}

/// `<root>/workspace/<name>` (see [`workspace_name`]).
pub fn routine_workspace_dir(root: &Path, name: &str) -> PathBuf {
    workspace_dir(root).join(name)
}

/// Creates `<root>/workspace` and `<root>/workspace/<name>` (`0700` on Unix) when missing and
/// returns the routine's folder as a checked canonical path ([`checked_workspace`]). `name` is a
/// [`workspace_name`]. A symbolic link at either is refused (`invalidInput`), so a routine never
/// runs in a folder someone pointed elsewhere, and two routines never share one.
pub fn ensure_routine_workspace(root: &Path, name: &str) -> Result<PathBuf> {
    check_workspace_name(name)?;

    let parent = workspace_dir(root);
    refuse_link(&parent)?;
    create_private_dir(&parent, "creating the routines workspace folder")?;

    let dir = parent.join(name);
    refuse_link(&dir)?;
    create_private_dir(&dir, "creating a routine's workspace folder")?;

    checked_workspace(root, Some(name))
}

/// The canonical path of `<root>/workspace` (`name` `None`) or `<root>/workspace/<name>`, once it
/// exists: each level a real folder, not a symbolic link, owned by the user and not writable by
/// group or others (Unix), in a root that is the same, and the whole path the canonical root
/// followed by those names. `notFound` while it is missing, `invalidInput` otherwise.
pub fn checked_workspace(root: &Path, name: Option<&str>) -> Result<PathBuf> {
    let root =
        dunce::canonicalize(root).map_err(|error| BusinessError::from_io("reading the routines folder", error))?;
    let mut expected = workspace_dir(&root);
    let mut levels = vec![expected.clone()];

    if let Some(name) = name {
        check_workspace_name(name)?;
        expected = expected.join(name);
        levels.push(expected.clone());
    }

    for level in &levels {
        let metadata = std::fs::symlink_metadata(level)
            .map_err(|error| BusinessError::from_io("reading the routines workspace folder", error))?;

        if !metadata.is_dir() {
            return Err(moved());
        }

        if let Some(problem) = unsafe_folder::<RoutineKind>(&metadata) {
            return Err(BusinessError::InvalidInput(problem));
        }
    }

    if let Some(problem) = unsafe_root::<RoutineKind>(&root) {
        return Err(BusinessError::InvalidInput(problem));
    }

    let canonical = dunce::canonicalize(&expected)
        .map_err(|error| BusinessError::from_io("reading the routines workspace folder", error))?;

    if canonical != expected {
        return Err(moved());
    }

    Ok(canonical)
}

/// `invalidInput` unless `name` is a [`workspace_name`].
fn check_workspace_name(name: &str) -> Result<()> {
    match workspace_owner(name) {
        Some(_) => Ok(()),
        None => Err(BusinessError::InvalidInput(
            "a routine's folder is named after the routine".to_owned(),
        )),
    }
}

fn moved() -> BusinessError {
    BusinessError::InvalidInput("the routines workspace is not a plain folder of the routines folder".to_owned())
}

/// The working folders in `<root>/workspace`, as checked canonical paths ([`checked_workspace`]):
/// the entries named like a routine's folder ([`workspace_name`]) that are real folders, by name.
pub fn routine_workspaces(root: &Path) -> Vec<PathBuf> {
    workspace_names(root)
        .iter()
        .filter_map(|name| checked_workspace(root, Some(name)).ok())
        .collect()
}

/// The entries of `<root>/workspace` named like a routine's folder, by name.
fn workspace_names(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(workspace_dir(root)) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| workspace_owner(name).is_some())
        .collect();
    names.sort();

    names
}

/// True while the routine a workspace folder `name` belongs to is still there: a local routine
/// file `<root>/routines/<id>.json`, or a team copy `<root>/team/<id>.json` (an entry that cannot
/// be read counts, as for approvals).
pub fn workspace_in_use(root: &Path, name: &str) -> bool {
    let Some((id, origin)) = workspace_owner(name) else {
        return false;
    };

    let folder = match origin {
        HookOrigin::Local => ROUTINES_FOLDER,
        HookOrigin::Team => TEAM_FOLDER,
    };

    match std::fs::symlink_metadata(root.join(folder).join(format!("{id}.json"))) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Deletes the workspace folders whose routine is gone ([`workspace_in_use`]) and that `keep`
/// does not name (a run of their routine still goes on), each only after [`checked_workspace`]
/// confirmed it is a real folder of the workspace: a symbolic link is never followed or removed.
/// The sessions their runs kept stay in the harnesses' session stores. Returns the names deleted.
pub fn forget_workspaces(root: &Path, keep: impl Fn(&str) -> bool) -> Vec<String> {
    let mut forgotten = Vec::new();

    for name in workspace_names(root) {
        if workspace_in_use(root, &name) || keep(&name) {
            continue;
        }

        let Ok(folder) = checked_workspace(root, Some(&name)) else {
            continue;
        };

        // `remove_dir_all` deletes the links inside the folder, never what they point to.
        match std::fs::remove_dir_all(&folder) {
            Ok(()) => forgotten.push(name),
            Err(error) => tracing::warn!(kind = ?error.kind(), "a routine's workspace folder was not deleted"),
        }
    }

    forgotten
}
