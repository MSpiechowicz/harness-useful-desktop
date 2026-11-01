//! Harness Memory's `config.toml`: reading it ([`read_config`]), writing it ([`write_config`],
//! [`render`]) and removing it ([`remove_config`]).
//!
//! The file handling (the refusals, the atomic write, the permissions) is
//! [`sidecar_config`](crate::ecosystem::sidecar_config)'s; this file knows the keys Harness Memory
//! reads ([`KNOWN_KEYS`]) and what they mean.

use std::path::Path;

use toml::Table;

use super::routing::{Routing, notes_name, notes_value, set_routing, set_target, target_name};
use super::url::{DOCUMENTDB_KIND, MONGODB_KIND};
use crate::ecosystem::catalog::MEMORY_SIDECAR;
use crate::ecosystem::sidecar_config::{
    ConfigSpec, ConfigStatus, KIND_KEY, TOKEN_KEY, URL_KEY, bool_value, database_text, push_string, read_config_file,
    remove_config_file, string_value, write_config_file,
};
use crate::error::Result;
use crate::model::{MemoryNotesRouting, MemoryNotesTarget};

/// The keys Harness Memory reads from the file.
pub(super) const KNOWN_KEYS: [&str; 11] = [
    URL_KEY,
    KIND_KEY,
    TOKEN_KEY,
    REPOSITORY_KEY,
    GLOBAL_KEY,
    SCRIPTS_KEY,
    TRUST_TEAM_KEY,
    HOOKS_KEY,
    TRUST_HOOKS_KEY,
    ROUTINES_KEY,
    TRUST_ROUTINES_KEY,
];

/// The memory server's config file.
pub(super) const SPEC: ConfigSpec = ConfigSpec {
    sidecar: &MEMORY_SIDECAR,
    known_keys: &KNOWN_KEYS,
};

const REPOSITORY_KEY: &str = "repository_notes";
const GLOBAL_KEY: &str = "global_notes";
const SCRIPTS_KEY: &str = "scripts";
const TRUST_TEAM_KEY: &str = "trust_team_scripts";
const HOOKS_KEY: &str = "hooks";
const TRUST_HOOKS_KEY: &str = "trust_team_hooks";
const ROUTINES_KEY: &str = "routines";
const TRUST_ROUTINES_KEY: &str = "trust_team_routines";

/// What the config file holds, and what is wrong with it.
#[derive(Debug, Default)]
pub(super) struct ConfigFile {
    /// `database_url`, when it is a string.
    pub url: Option<String>,
    /// `database_kind`, when it is a string (`mongodb` or `documentdb`, any case; another value is
    /// noted). Unset lets the server pick by the host.
    pub database_kind: Option<String>,
    /// `database_token`, when it is a non-empty string.
    pub token: Option<String>,
    /// `repository_notes` (`remote` when unset or not understood).
    pub repository_notes: MemoryNotesRouting,
    /// `global_notes` (`remote` when unset or not understood).
    pub global_notes: MemoryNotesRouting,
    /// `scripts`, when it is set to `remote`, `local` or `remote+local` (unset means remote).
    pub scripts: Option<MemoryNotesRouting>,
    /// `trust_team_scripts`, when it is a boolean (unset means false).
    pub trust_team_scripts: Option<bool>,
    /// `hooks`, when it is set to `remote` or `local` (unset means local: hooks are not shared).
    pub hooks: Option<MemoryNotesTarget>,
    /// `trust_team_hooks`, when it is a boolean (unset means false).
    pub trust_team_hooks: Option<bool>,
    /// `routines`, when it is set to `remote` or `local` (unset means local: not shared).
    pub routines: Option<MemoryNotesTarget>,
    /// `trust_team_routines`, when it is a boolean (unset means false).
    pub trust_team_routines: Option<bool>,
    /// Whether the app may change the file, and the first thing wrong with it.
    pub status: ConfigStatus,
}

/// The file at `path`: empty (and fine) when it does not exist. See [`read_config_file`].
pub(super) fn read_config(path: &Path) -> ConfigFile {
    let (mut config, status) = read_config_file(&SPEC, path, parse_config);

    config.status = status;
    config
}

/// The settings in `table`, whose keys are all known.
fn parse_config(table: &Table, status: &mut ConfigStatus) -> ConfigFile {
    ConfigFile {
        url: string_value(table, URL_KEY, status),
        database_kind: kind_value(table, status),
        token: string_value(table, TOKEN_KEY, status).filter(|token| !token.trim().is_empty()),
        repository_notes: notes_value(table, REPOSITORY_KEY, status),
        global_notes: notes_value(table, GLOBAL_KEY, status),
        scripts: set_routing(table, SCRIPTS_KEY, status),
        trust_team_scripts: bool_value(table, TRUST_TEAM_KEY, status),
        hooks: set_target(table, HOOKS_KEY, status),
        trust_team_hooks: bool_value(table, TRUST_HOOKS_KEY, status),
        routines: set_target(table, ROUTINES_KEY, status),
        trust_team_routines: bool_value(table, TRUST_ROUTINES_KEY, status),
        status: ConfigStatus::default(),
    }
}

/// `database_kind` as written; a value other than `mongodb` or `documentdb` (any case) is noted.
fn kind_value(table: &Table, status: &mut ConfigStatus) -> Option<String> {
    let kind = string_value(table, KIND_KEY, status)?;
    let known = [MONGODB_KIND, DOCUMENTDB_KIND]
        .iter()
        .any(|name| kind.trim().eq_ignore_ascii_case(name));

    if !known {
        status.note(format!(
            "{KIND_KEY} must be \"{MONGODB_KIND}\" or \"{DOCUMENTDB_KIND}\""
        ));
    }

    Some(kind)
}

/// The file's text: the URL, `database_kind` and the token when there are, both note routing
/// keys, and the `scripts`, `trust_team_scripts`, `hooks`, `trust_team_hooks`, `routines` and
/// `trust_team_routines` keys when there are.
pub(super) fn render(url: &str, database_kind: Option<&str>, token: Option<&str>, routing: Routing) -> String {
    let mut text = database_text(&MEMORY_SIDECAR, url, database_kind, token);

    push_string(&mut text, REPOSITORY_KEY, notes_name(routing.repository_notes));
    push_string(&mut text, GLOBAL_KEY, notes_name(routing.global_notes));

    if let Some(scripts) = routing.scripts {
        push_string(&mut text, SCRIPTS_KEY, notes_name(scripts));
    }

    if let Some(trust) = routing.trust_team_scripts {
        text.push_str(&format!("{TRUST_TEAM_KEY} = {trust}\n"));
    }

    let shared = [
        (HOOKS_KEY, routing.hooks, TRUST_HOOKS_KEY, routing.trust_team_hooks),
        (
            ROUTINES_KEY,
            routing.routines,
            TRUST_ROUTINES_KEY,
            routing.trust_team_routines,
        ),
    ];

    for (key, target, trust_key, trust) in shared {
        if let Some(target) = target {
            push_string(&mut text, key, target_name(target));
        }

        if let Some(trust) = trust {
            text.push_str(&format!("{trust_key} = {trust}\n"));
        }
    }

    text
}

/// Replaces the file at `path` with `text` (see [`write_config_file`]).
pub(super) fn write_config(path: &Path, text: &str) -> Result<()> {
    write_config_file(&MEMORY_SIDECAR, path, text)
}

/// Removes the file at `path` (see [`remove_config_file`]).
pub(super) fn remove_config(path: &Path) -> Result<()> {
    remove_config_file(&MEMORY_SIDECAR, path)
}
