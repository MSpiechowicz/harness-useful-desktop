//! Harness Knowledge's `config.toml`: reading it ([`read_config`]), writing it ([`render`],
//! [`write_config`]) and removing it ([`remove_config`]).
//!
//! The file handling (the refusals, the atomic write, the permissions) is
//! [`sidecar_config`](crate::ecosystem::sidecar_config)'s. This file knows the one table Harness
//! Knowledge reads: `[database]` with `url` and `token`. The server refuses any other key, in the
//! table or beside it, so a file with one is reported and never overwritten.

use std::path::Path;

use toml::{Table, Value};

use crate::ecosystem::catalog::KNOWLEDGE_SIDECAR;
use crate::ecosystem::sidecar_config::{
    ConfigSpec, ConfigStatus, push_string, read_config_file, remove_config_file, string_value, write_config_file,
};
use crate::error::Result;

/// The table of the database settings.
const TABLE: &str = "database";

/// The key of the database URL in the table.
const URL_KEY: &str = "url";

/// The key of the libSQL token in the table.
const TOKEN_KEY: &str = "token";

/// The keys Harness Knowledge reads beside the file's tables (just the one table).
const KNOWN_KEYS: [&str; 1] = [TABLE];

/// The knowledge server's config file.
const SPEC: ConfigSpec = ConfigSpec {
    sidecar: &KNOWLEDGE_SIDECAR,
    known_keys: &KNOWN_KEYS,
};

/// What the config file holds, and what is wrong with it.
#[derive(Debug, Default)]
pub(super) struct ConfigFile {
    /// `[database] url`, when it is a string.
    pub url: Option<String>,
    /// `[database] token`, when it is a non-empty string.
    pub token: Option<String>,
    /// Whether the app may change the file, and the first thing wrong with it.
    pub status: ConfigStatus,
}

/// The file at `path`: empty (and fine) when it does not exist. See [`read_config_file`].
pub(super) fn read_config(path: &Path) -> ConfigFile {
    let (mut config, status) = read_config_file(&SPEC, path, parse_config);

    config.status = status;
    config
}

/// The settings in `table`, whose top-level keys are all known.
fn parse_config(table: &Table, status: &mut ConfigStatus) -> ConfigFile {
    let Some(database) = table.get(TABLE) else {
        return ConfigFile::default();
    };

    let Value::Table(database) = database else {
        block(status, format!("{TABLE} must be a table with url and token"));
        return ConfigFile::default();
    };

    let unknown = database.keys().any(|key| key != URL_KEY && key != TOKEN_KEY);

    if unknown {
        block(
            status,
            format!(
                "the {TABLE} table has keys {} does not know; remove them by hand first",
                KNOWLEDGE_SIDECAR.product
            ),
        );
    }

    ConfigFile {
        url: string_value(database, URL_KEY, status),
        token: string_value(database, TOKEN_KEY, status).filter(|token| !token.trim().is_empty()),
        status: ConfigStatus::default(),
    }
}

/// Notes `reason` as what blocks the app from changing the file (and the server from using it).
fn block(status: &mut ConfigStatus, reason: String) {
    status.refused = true;
    status.note(reason.clone());
    status.blocked.get_or_insert(reason);
}

/// The file's text: a comment, then the `[database]` table with the URL and the token when there
/// is one.
pub(super) fn render(url: &str, token: Option<&str>) -> String {
    let mut text = format!(
        "# {} team database, saved by Harness Desktop. It holds the database credentials:\n\
         # keep it private (chmod 600).\n[{TABLE}]\n",
        KNOWLEDGE_SIDECAR.product
    );

    push_string(&mut text, URL_KEY, url);

    if let Some(token) = token {
        push_string(&mut text, TOKEN_KEY, token);
    }

    text
}

/// Replaces the file at `path` with `text` (see [`write_config_file`]).
pub(super) fn write_config(path: &Path, text: &str) -> Result<()> {
    write_config_file(&KNOWLEDGE_SIDECAR, path, text)
}

/// Removes the file at `path` (see [`remove_config_file`]).
pub(super) fn remove_config(path: &Path) -> Result<()> {
    remove_config_file(&KNOWLEDGE_SIDECAR, path)
}
