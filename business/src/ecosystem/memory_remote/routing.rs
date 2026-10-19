//! The routing keys of Harness Memory's `config.toml`: where each kind of record goes while a team
//! database is set. The two note keys (`repository_notes`, `global_notes`) and `scripts` take
//! `remote`, `local` or `remote+local` ([`MemoryNotesRouting`]); `hooks` and `routines` take
//! `remote` or `local` only ([`MemoryNotesTarget`]).

use toml::Table;

use super::file::ConfigFile;
use crate::model::{MemoryNotesRouting, MemoryNotesTarget};

/// The value of [`MemoryNotesRouting::RemoteLocal`] in the file.
const REMOTE_LOCAL: &str = "remote+local";

/// Where each kind of record goes, as [`render`](super::file::render) writes it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Routing {
    pub repository_notes: MemoryNotesRouting,
    pub global_notes: MemoryNotesRouting,
    /// The `scripts` key, `None` for none (an older server refuses a file with the key).
    pub scripts: Option<MemoryNotesRouting>,
    /// The `trust_team_scripts` key, `None` for none (an older server refuses it too).
    pub trust_team_scripts: Option<bool>,
    /// The `hooks` key, `None` for none (a server without team hooks refuses it).
    pub hooks: Option<MemoryNotesTarget>,
    /// The `trust_team_hooks` key, `None` for none (refused by the same servers).
    pub trust_team_hooks: Option<bool>,
    /// The `routines` key, `None` for none (a server without team routines refuses it).
    pub routines: Option<MemoryNotesTarget>,
    /// The `trust_team_routines` key, `None` for none (refused by the same servers).
    pub trust_team_routines: Option<bool>,
}

/// The note routing `key` (`remote` when unset or not understood; another value is noted).
pub(super) fn notes_value(table: &Table, key: &str, config: &mut ConfigFile) -> MemoryNotesRouting {
    set_routing(table, key, config).unwrap_or_default()
}

/// The routing `key` of notes or scripts when it is set to `remote`, `local` or `remote+local`;
/// another value is noted.
pub(super) fn set_routing(table: &Table, key: &str, config: &mut ConfigFile) -> Option<MemoryNotesRouting> {
    match table.get(key)?.as_str() {
        Some("remote") => Some(MemoryNotesRouting::Remote),
        Some("local") => Some(MemoryNotesRouting::Local),
        Some(REMOTE_LOCAL) => Some(MemoryNotesRouting::RemoteLocal),
        _ => {
            config.note(format!("{key} must be \"remote\", \"local\" or \"{REMOTE_LOCAL}\""));
            None
        }
    }
}

/// The routing `key` of hooks or routines when it is set to `remote` or `local`; another value is
/// noted.
pub(super) fn set_target(table: &Table, key: &str, config: &mut ConfigFile) -> Option<MemoryNotesTarget> {
    match table.get(key)?.as_str() {
        Some("remote") => Some(MemoryNotesTarget::Remote),
        Some("local") => Some(MemoryNotesTarget::Local),
        _ => {
            config.note(format!("{key} must be \"remote\" or \"local\""));
            None
        }
    }
}

/// The file's value of the routing of notes or scripts.
pub(super) fn notes_name(routing: MemoryNotesRouting) -> &'static str {
    match routing {
        MemoryNotesRouting::Remote => "remote",
        MemoryNotesRouting::Local => "local",
        MemoryNotesRouting::RemoteLocal => REMOTE_LOCAL,
    }
}

/// The file's value of the routing of hooks or routines.
pub(super) fn target_name(target: MemoryNotesTarget) -> &'static str {
    match target {
        MemoryNotesTarget::Remote => "remote",
        MemoryNotesTarget::Local => "local",
    }
}
