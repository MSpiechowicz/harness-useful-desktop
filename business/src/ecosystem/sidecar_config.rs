//! The config file of a Harness family server (`<config dir>/<binary>/config.toml`): where it is
//! ([`config_path`]), reading it ([`read_config_file`]), writing it ([`write_config_file`]) and
//! removing it ([`remove_config_file`]). [`memory_remote`](super::memory_remote) is the memory
//! server's, which adds the keys it knows.
//!
//! The server refuses keys it does not know, so the file holds only [`ConfigSpec::known_keys`]. A
//! file the app cannot read, cannot parse, or that holds other keys is reported and never
//! overwritten, so a hand-written file is not lost. The file is replaced in one rename (a fresh
//! `0600` file on Unix), its folder is created `0700`, and a symbolic link at the file or at its
//! folder is refused.

use std::path::{Path, PathBuf};

use toml::{Table, Value};

use super::sidecar::Sidecar;
use crate::env::{Env, OsKind};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::security::SafeReader;

/// The largest config file read (64 KiB; the file holds a handful of short keys).
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// How many unknown key names a problem lists.
const LISTED_KEYS: usize = 5;

/// The key of the team database URL.
pub(crate) const URL_KEY: &str = "database_url";

/// The key of the team database kind.
pub(crate) const KIND_KEY: &str = "database_kind";

/// The key of the team database token.
pub(crate) const TOKEN_KEY: &str = "database_token";

/// Which config file, and what is in it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConfigSpec {
    /// The server the file belongs to.
    pub sidecar: &'static Sidecar,
    /// The keys the server reads from the file.
    pub known_keys: &'static [&'static str],
}

/// What is wrong with a config file that was read.
#[derive(Debug, Default)]
pub(crate) struct ConfigStatus {
    /// Why the app must not replace or remove the file, if it must not.
    pub blocked: Option<String>,
    /// True when the server would not use the file as it is: a `blocked` one, or one other users
    /// may change or that another user owns. Its values must not be passed to the server some
    /// other way.
    pub refused: bool,
    /// The first thing wrong with the file (the `blocked` reason first).
    pub problem: Option<String>,
}

impl ConfigStatus {
    fn blocked(reason: String) -> Self {
        Self {
            problem: Some(reason.clone()),
            blocked: Some(reason),
            refused: true,
        }
    }

    /// Notes `problem` unless something else was noted first.
    pub(crate) fn note(&mut self, problem: impl Into<String>) {
        self.problem.get_or_insert_with(|| problem.into());
    }
}

/// The server's config file, found as the server finds it: `<prefix>_CONFIG` when set, else
/// under `$XDG_CONFIG_HOME` (or `~/.config`) on Linux, `~/Library/Application Support` on macOS
/// and `%APPDATA%` on Windows, then `<binary>/config.toml`. A relative variable does not count, as
/// for the server. `None` while the variable is relative (the server then refuses to start), and
/// on Windows while `APPDATA` is unset.
pub(crate) fn config_path(sidecar: &Sidecar, env: &Env) -> Option<PathBuf> {
    if let Some(raw) = env.var(&sidecar.var("CONFIG")) {
        let path = env.expand_home(raw);
        return path.has_root().then_some(path);
    }

    let rooted = |name: &str| env.var(name).map(PathBuf::from).filter(|dir| dir.has_root());

    let dir = match env.os {
        OsKind::Linux => rooted("XDG_CONFIG_HOME").unwrap_or_else(|| env.home_join(".config")),
        OsKind::MacOs => env.home_join("Library/Application Support"),
        OsKind::Windows => rooted("APPDATA")?,
    };

    Some(dir.join(sidecar.binary).join("config.toml"))
}

/// What to show as the path while [`config_path`] finds none: the variable as set, else the
/// Windows path.
pub(crate) fn path_hint(sidecar: &Sidecar, env: &Env) -> String {
    match env.var(&sidecar.var("CONFIG")) {
        Some(raw) => raw.to_owned(),
        None => format!("%APPDATA%\\{}\\config.toml", sidecar.binary),
    }
}

/// Why [`config_path`] found no file.
pub(crate) fn unknown_path(sidecar: &Sidecar, env: &Env) -> String {
    let var = sidecar.var("CONFIG");

    if env.var(&var).is_some() {
        format!("{var} is not an absolute path, so {} refuses it", sidecar.product)
    } else {
        "APPDATA is not set, so the config folder is unknown".to_owned()
    }
}

/// The file at `path`: `T::default()` (and fine) when it does not exist. `parse` reads the keys
/// of a file that parsed and holds only known keys, and notes what it finds wrong. The file's
/// permissions are checked the way the server checks them (Unix): a file other users may change
/// or that another user owns is refused by the server, one other users may read is reported.
pub(crate) fn read_config_file<T: Default>(
    spec: &ConfigSpec,
    path: &Path,
    parse: impl FnOnce(&Table, &mut ConfigStatus) -> T,
) -> (T, ConfigStatus) {
    let blocked = |reason: String| (T::default(), ConfigStatus::blocked(reason));

    let link = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_symlink(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (T::default(), ConfigStatus::default()),
        Err(error) => return blocked(format!("the file cannot be read ({})", error.kind())),
    };

    let text = match SafeReader::read_to_string(path, MAX_CONFIG_BYTES) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) if !link => return (T::default(), ConfigStatus::default()),
        Err(error) => return blocked(unreadable(&error)),
    };

    let table = match parse_table(spec, &text) {
        Ok(table) => table,
        Err(reason) => return blocked(reason),
    };

    let mut status = ConfigStatus::default();
    let config = parse(&table, &mut status);

    if link {
        let reason = "the file is a symbolic link; the app does not change it".to_owned();
        status.blocked.get_or_insert_with(|| reason.clone());
        status.refused = true;
        status.note(reason);
    }

    if let Some(problem) = permission_problem(spec.sidecar, path) {
        status.refused |= problem.refused;
        status.note(problem.message);
    }

    (config, status)
}

/// What is wrong with the file's permissions.
struct PermissionProblem {
    message: String,
    /// The server refuses the file (not just a warning).
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

/// The table in `text`, or why the file is blocked. A TOML error is reported by its line only: the
/// parser's message can quote the line, which may hold the URL.
fn parse_table(spec: &ConfigSpec, text: &str) -> std::result::Result<Table, String> {
    let table = toml::from_str::<Table>(text).map_err(|error| {
        let line = error
            .span()
            .map(|span| text[..span.start.min(text.len())].matches('\n').count() + 1);

        match line {
            Some(line) => format!("the file is not valid TOML (line {line})"),
            None => "the file is not valid TOML".to_owned(),
        }
    })?;

    let unknown: Vec<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|key| !spec.known_keys.contains(key))
        .collect();

    if unknown.is_empty() {
        Ok(table)
    } else {
        Err(unknown_keys(spec.sidecar, &unknown))
    }
}

/// The problem for keys the server does not know (their names, control characters dropped).
fn unknown_keys(sidecar: &Sidecar, keys: &[&str]) -> String {
    let mut names: Vec<String> = keys
        .iter()
        .take(LISTED_KEYS)
        .map(|key| key.chars().filter(|c| !c.is_control()).take(40).collect())
        .collect();

    if keys.len() > LISTED_KEYS {
        names.push(format!("and {} more", keys.len() - LISTED_KEYS));
    }

    format!(
        "the file has keys {} does not know ({}); remove them by hand first",
        sidecar.product,
        names.join(", ")
    )
}

/// `key` when it is a string; anything else is noted.
pub(crate) fn string_value(table: &Table, key: &str, status: &mut ConfigStatus) -> Option<String> {
    match table.get(key)? {
        Value::String(value) => Some(value.clone()),
        _ => {
            status.note(format!("{key} must be a string"));
            None
        }
    }
}

/// `key` when it is a boolean; anything else is noted.
pub(crate) fn bool_value(table: &Table, key: &str, status: &mut ConfigStatus) -> Option<bool> {
    match table.get(key)? {
        Value::Boolean(value) => Some(*value),
        _ => {
            status.note(format!("{key} must be true or false"));
            None
        }
    }
}

/// The start of a file the app writes: the comment, then the URL, `database_kind` and the token
/// when there are.
pub(crate) fn database_text(sidecar: &Sidecar, url: &str, database_kind: Option<&str>, token: Option<&str>) -> String {
    let mut text = format!(
        "# {} team database, saved by Harness Desktop. It holds the database credentials:\n\
         # keep it private (chmod 600).\n",
        sidecar.product
    );

    push_string(&mut text, URL_KEY, url);

    if let Some(kind) = database_kind {
        push_string(&mut text, KIND_KEY, kind);
    }

    if let Some(token) = token {
        push_string(&mut text, TOKEN_KEY, token);
    }

    text
}

/// Appends `key = "value"` to `text`.
pub(crate) fn push_string(text: &mut String, key: &str, value: &str) {
    text.push_str(key);
    text.push_str(" = ");
    text.push_str(&basic_string(value));
    text.push('\n');
}

/// `value` as a TOML basic string: quotes and backslashes escaped, control characters as
/// `\uXXXX` (or their short escapes), everything else (any Unicode) as written.
pub(crate) fn basic_string(value: &str) -> String {
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
pub(crate) fn write_config_file(sidecar: &Sidecar, path: &Path, text: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| BusinessError::Internal("the config file has no folder".to_owned()))?;

    ensure_dir(sidecar, dir)?;
    ensure_replaceable(sidecar, path)?;
    replace_atomically(
        path,
        text.as_bytes(),
        &format!("saving the {} config file", sidecar.product),
    )
}

/// Removes the file at `path` (nothing to do when it does not exist). The folder is kept.
pub(crate) fn remove_config_file(sidecar: &Sidecar, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        refuse_link(dir)?;
    }

    ensure_replaceable(sidecar, path)?;

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BusinessError::from_io(
            format!("removing the {} config file", sidecar.product),
            error,
        )),
    }
}

/// Creates `dir` (`0700` on Unix, its missing parents too) unless it is already a real folder.
fn ensure_dir(sidecar: &Sidecar, dir: &Path) -> Result<()> {
    refuse_link(dir)?;

    if dir.is_dir() {
        return Ok(());
    }

    create_private_dir(dir, &format!("creating the {} config folder", sidecar.product))
}

/// Refuses a symbolic link or anything but a regular file at `path`; a missing file is fine.
fn ensure_replaceable(sidecar: &Sidecar, path: &Path) -> Result<()> {
    refuse_link(path)?;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => Err(BusinessError::InvalidInput(format!(
            "the {} config file is not a regular file",
            sidecar.product
        ))),
        _ => Ok(()),
    }
}

/// What the server would say about the file's permissions (Unix; `None` elsewhere).
#[cfg(unix)]
fn permission_problem(sidecar: &Sidecar, path: &Path) -> Option<PermissionProblem> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = std::fs::metadata(path).ok()?;
    let mode = metadata.permissions().mode();

    if mode & 0o022 != 0 {
        return Some(PermissionProblem {
            message: format!(
                "other users can change the file, so {} refuses it; save here or run chmod 600 on it",
                sidecar.product
            ),
            refused: true,
        });
    }

    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Some(PermissionProblem {
            message: format!(
                "the file belongs to another user, so {} refuses it; save here to replace it",
                sidecar.product
            ),
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
fn permission_problem(_sidecar: &Sidecar, _path: &Path) -> Option<PermissionProblem> {
    None
}
