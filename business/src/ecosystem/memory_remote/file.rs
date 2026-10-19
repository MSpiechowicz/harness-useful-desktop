//! Harness Memory's `config.toml`: reading it ([`read_config`]), writing it ([`write_config`],
//! [`render`]) and removing it ([`remove_config`]).
//!
//! The server refuses keys it does not know, so the file holds only [`KNOWN_KEYS`]. A file the app
//! cannot read, cannot parse, or that holds other keys is reported and never overwritten, so a
//! hand-written file is not lost. The file is replaced in one rename (a fresh `0600` file on Unix),
//! its folder is created `0700`, and a symbolic link at the file or at its folder is refused.

use std::path::Path;

use toml::{Table, Value};

use super::routing::{Routing, notes_name, notes_value, set_routing, set_target, target_name};
use super::url::{DOCUMENTDB_KIND, MONGODB_KIND};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::model::{MemoryNotesRouting, MemoryNotesTarget};
use crate::security::SafeReader;

/// The largest config file read (64 KiB; the file holds a handful of short keys).
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

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

const URL_KEY: &str = "database_url";
const KIND_KEY: &str = "database_kind";
const TOKEN_KEY: &str = "database_token";
const REPOSITORY_KEY: &str = "repository_notes";
const GLOBAL_KEY: &str = "global_notes";
const SCRIPTS_KEY: &str = "scripts";
const TRUST_TEAM_KEY: &str = "trust_team_scripts";
const HOOKS_KEY: &str = "hooks";
const TRUST_HOOKS_KEY: &str = "trust_team_hooks";
const ROUTINES_KEY: &str = "routines";
const TRUST_ROUTINES_KEY: &str = "trust_team_routines";

/// How many unknown key names a problem lists.
const LISTED_KEYS: usize = 5;

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
    /// Why the app must not replace or remove the file, if it must not.
    pub blocked: Option<String>,
    /// True when Harness Memory would not use the file as it is: a `blocked` one, or one other users
    /// may change or that another user owns. Its values must not be passed to the server some other
    /// way.
    pub refused: bool,
    /// The first thing wrong with the file (the `blocked` reason first).
    pub problem: Option<String>,
}

impl ConfigFile {
    fn blocked(reason: String) -> Self {
        Self {
            problem: Some(reason.clone()),
            blocked: Some(reason),
            refused: true,
            ..Self::default()
        }
    }

    pub(super) fn note(&mut self, problem: impl Into<String>) {
        self.problem.get_or_insert_with(|| problem.into());
    }
}

/// The file at `path`: empty (and fine) when it does not exist. Its permissions are checked the
/// way Harness Memory checks them (Unix): a file other users may change or that another user owns
/// is refused by the server, one other users may read is reported.
pub(super) fn read_config(path: &Path) -> ConfigFile {
    let link = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_symlink(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return ConfigFile::default(),
        Err(error) => return ConfigFile::blocked(format!("the file cannot be read ({})", error.kind())),
    };

    let text = match SafeReader::read_to_string(path, MAX_CONFIG_BYTES) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) if !link => return ConfigFile::default(),
        Err(error) => return ConfigFile::blocked(unreadable(&error)),
    };

    let mut config = parse_config(&text);

    if link {
        let reason = "the file is a symbolic link; the app does not change it".to_owned();
        config.blocked.get_or_insert_with(|| reason.clone());
        config.refused = true;
        config.note(reason);
    }

    if let Some(problem) = permission_problem(path) {
        config.refused |= problem.refused;
        config.note(problem.message);
    }

    config
}

/// What is wrong with the file's permissions.
struct PermissionProblem {
    message: String,
    /// Harness Memory refuses the file (not just a warning).
    refused: bool,
}

/// Why the file could not be read, without its content.
fn unreadable(error: &BusinessError) -> String {
    match error {
        BusinessError::NotFound(_) => "the file is a symbolic link to a missing file".to_owned(),
        BusinessError::Forbidden(_) => "the file cannot be read: permission denied".to_owned(),
        BusinessError::InvalidInput(message) if message.ends_with("is not valid UTF-8") => {
            "the file is not valid UTF-8".to_owned()
        }
        BusinessError::InvalidInput(message) if message.ends_with(&format!("larger than {MAX_CONFIG_BYTES} bytes")) => {
            "the file is larger than 64 KiB".to_owned()
        }
        BusinessError::InvalidInput(_) => "the file is not a regular file".to_owned(),
        _ => "the file cannot be read".to_owned(),
    }
}

/// The settings in `text`. A TOML error is reported by its line only: the parser's message can
/// quote the line, which may hold the URL.
fn parse_config(text: &str) -> ConfigFile {
    let table = match toml::from_str::<Table>(text) {
        Ok(table) => table,
        Err(error) => {
            let line = error
                .span()
                .map(|span| text[..span.start.min(text.len())].matches('\n').count() + 1);

            return ConfigFile::blocked(match line {
                Some(line) => format!("the file is not valid TOML (line {line})"),
                None => "the file is not valid TOML".to_owned(),
            });
        }
    };

    let unknown: Vec<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|key| !KNOWN_KEYS.contains(key))
        .collect();

    if !unknown.is_empty() {
        return ConfigFile::blocked(unknown_keys(&unknown));
    }

    let mut config = ConfigFile::default();

    config.url = string_value(&table, URL_KEY, &mut config);
    config.database_kind = kind_value(&table, &mut config);
    config.token = string_value(&table, TOKEN_KEY, &mut config).filter(|token| !token.trim().is_empty());
    config.repository_notes = notes_value(&table, REPOSITORY_KEY, &mut config);
    config.global_notes = notes_value(&table, GLOBAL_KEY, &mut config);
    config.scripts = set_routing(&table, SCRIPTS_KEY, &mut config);
    config.trust_team_scripts = bool_value(&table, TRUST_TEAM_KEY, &mut config);
    config.hooks = set_target(&table, HOOKS_KEY, &mut config);
    config.trust_team_hooks = bool_value(&table, TRUST_HOOKS_KEY, &mut config);
    config.routines = set_target(&table, ROUTINES_KEY, &mut config);
    config.trust_team_routines = bool_value(&table, TRUST_ROUTINES_KEY, &mut config);

    config
}

/// The problem for keys Harness Memory does not know (their names, control characters dropped).
fn unknown_keys(keys: &[&str]) -> String {
    let mut names: Vec<String> = keys
        .iter()
        .take(LISTED_KEYS)
        .map(|key| key.chars().filter(|c| !c.is_control()).take(40).collect())
        .collect();

    if keys.len() > LISTED_KEYS {
        names.push(format!("and {} more", keys.len() - LISTED_KEYS));
    }

    format!(
        "the file has keys Harness Memory does not know ({}); remove them by hand first",
        names.join(", ")
    )
}

fn string_value(table: &Table, key: &str, config: &mut ConfigFile) -> Option<String> {
    match table.get(key)? {
        Value::String(value) => Some(value.clone()),
        _ => {
            config.note(format!("{key} must be a string"));
            None
        }
    }
}

/// `database_kind` as written; a value other than `mongodb` or `documentdb` (any case) is noted.
fn kind_value(table: &Table, config: &mut ConfigFile) -> Option<String> {
    let kind = string_value(table, KIND_KEY, config)?;
    let known = [MONGODB_KIND, DOCUMENTDB_KIND]
        .iter()
        .any(|name| kind.trim().eq_ignore_ascii_case(name));

    if !known {
        config.note(format!(
            "{KIND_KEY} must be \"{MONGODB_KIND}\" or \"{DOCUMENTDB_KIND}\""
        ));
    }

    Some(kind)
}

fn bool_value(table: &Table, key: &str, config: &mut ConfigFile) -> Option<bool> {
    match table.get(key)? {
        Value::Boolean(value) => Some(*value),
        _ => {
            config.note(format!("{key} must be true or false"));
            None
        }
    }
}

/// The file's text: the URL, `database_kind` and the token when there are, both note routing
/// keys, and the `scripts`, `trust_team_scripts`, `hooks`, `trust_team_hooks`, `routines` and
/// `trust_team_routines` keys when there are.
pub(super) fn render(url: &str, database_kind: Option<&str>, token: Option<&str>, routing: Routing) -> String {
    let mut text = String::from(
        "# Harness Memory team database, saved by Harness Desktop. It holds the database credentials:\n\
         # keep it private (chmod 600).\n",
    );

    let mut line = |key: &str, value: &str| {
        text.push_str(key);
        text.push_str(" = ");
        text.push_str(&basic_string(value));
        text.push('\n');
    };

    line(URL_KEY, url);

    if let Some(kind) = database_kind {
        line(KIND_KEY, kind);
    }

    if let Some(token) = token {
        line(TOKEN_KEY, token);
    }

    line(REPOSITORY_KEY, notes_name(routing.repository_notes));
    line(GLOBAL_KEY, notes_name(routing.global_notes));

    if let Some(scripts) = routing.scripts {
        line(SCRIPTS_KEY, notes_name(scripts));
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
            text.push_str(&format!("{key} = {}\n", basic_string(target_name(target))));
        }

        if let Some(trust) = trust {
            text.push_str(&format!("{trust_key} = {trust}\n"));
        }
    }

    text
}

/// `value` as a TOML basic string: quotes and backslashes escaped, control characters as
/// `\uXXXX` (or their short escapes), everything else (any Unicode) as written.
pub(super) fn basic_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');

    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }

    out.push('"');
    out
}

/// Replaces the file at `path` with `text`, creating its folder (`0700` on Unix) when missing.
/// Refuses a symbolic link at the file or its folder, and anything but a regular file.
pub(super) fn write_config(path: &Path, text: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| BusinessError::Internal("the config file has no folder".to_owned()))?;

    ensure_dir(dir)?;
    ensure_replaceable(path)?;
    replace_atomically(path, text.as_bytes(), "saving the Harness Memory config file")
}

/// Removes the file at `path` (nothing to do when it does not exist). The folder is kept.
pub(super) fn remove_config(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        refuse_link(dir)?;
    }

    ensure_replaceable(path)?;

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BusinessError::from_io("removing the Harness Memory config file", error)),
    }
}

/// Creates `dir` (`0700` on Unix, its missing parents too) unless it is already a real folder.
fn ensure_dir(dir: &Path) -> Result<()> {
    refuse_link(dir)?;

    if dir.is_dir() {
        return Ok(());
    }

    create_private_dir(dir, "creating the Harness Memory config folder")
}

/// Refuses a symbolic link or anything but a regular file at `path`; a missing file is fine.
fn ensure_replaceable(path: &Path) -> Result<()> {
    refuse_link(path)?;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => Err(BusinessError::InvalidInput(
            "the Harness Memory config file is not a regular file".to_owned(),
        )),
        _ => Ok(()),
    }
}

/// What Harness Memory would say about the file's permissions (Unix; `None` elsewhere).
#[cfg(unix)]
fn permission_problem(path: &Path) -> Option<PermissionProblem> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = std::fs::metadata(path).ok()?;
    let mode = metadata.permissions().mode();

    if mode & 0o022 != 0 {
        return Some(PermissionProblem {
            message: "other users can change the file, so Harness Memory refuses it; save here or run chmod 600 on it"
                .to_owned(),
            refused: true,
        });
    }

    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Some(PermissionProblem {
            message: "the file belongs to another user, so Harness Memory refuses it; save here to replace it"
                .to_owned(),
            refused: true,
        });
    }

    (mode & 0o044 != 0).then(|| PermissionProblem {
        message:
            "other users can read the file, which holds the database credentials; save here or run chmod 600 on it"
                .to_owned(),
        refused: false,
    })
}

#[cfg(not(unix))]
fn permission_problem(_path: &Path) -> Option<PermissionProblem> {
    None
}
