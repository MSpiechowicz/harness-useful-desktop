//! Harness Memory's `config.toml`: reading it ([`read_config`]), writing it ([`write_config`],
//! [`render`]) and removing it ([`remove_config`]).
//!
//! The server refuses keys it does not know, so the file holds only [`KNOWN_KEYS`]. A file the app
//! cannot read, cannot parse, or that holds other keys is reported and never overwritten, so a
//! hand-written file is not lost. The file is replaced in one rename (a fresh `0600` file on Unix),
//! its folder is created `0700`, and a symbolic link at the file or at its folder is refused.

use std::path::Path;

use toml::{Table, Value};

use crate::error::{BusinessError, Result};
use crate::fs_atomic::replace_atomically;
use crate::model::MemoryNotesTarget;
use crate::security::SafeReader;

/// The largest config file read (64 KiB; the file holds a handful of short keys).
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// The keys Harness Memory reads from the file.
pub(super) const KNOWN_KEYS: [&str; 4] = [URL_KEY, TOKEN_KEY, REPOSITORY_KEY, GLOBAL_KEY];

const URL_KEY: &str = "database_url";
const TOKEN_KEY: &str = "database_token";
const REPOSITORY_KEY: &str = "repository_notes";
const GLOBAL_KEY: &str = "global_notes";

/// How many unknown key names a problem lists.
const LISTED_KEYS: usize = 5;

/// What the config file holds, and what is wrong with it.
#[derive(Debug, Default)]
pub(super) struct ConfigFile {
    /// `database_url`, when it is a string.
    pub url: Option<String>,
    /// `database_token`, when it is a non-empty string.
    pub token: Option<String>,
    /// `repository_notes` (`remote` when unset or not understood).
    pub repository_notes: MemoryNotesTarget,
    /// `global_notes` (`remote` when unset or not understood).
    pub global_notes: MemoryNotesTarget,
    /// Why the app must not replace or remove the file, if it must not.
    pub blocked: Option<String>,
    /// The first thing wrong with the file (the `blocked` reason first).
    pub problem: Option<String>,
}

impl ConfigFile {
    fn blocked(reason: String) -> Self {
        Self {
            problem: Some(reason.clone()),
            blocked: Some(reason),
            ..Self::default()
        }
    }

    fn note(&mut self, problem: impl Into<String>) {
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
        config.note(reason);
    }

    if let Some(problem) = permission_problem(path) {
        config.note(problem);
    }

    config
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
    config.token = string_value(&table, TOKEN_KEY, &mut config).filter(|token| !token.trim().is_empty());
    config.repository_notes = target_value(&table, REPOSITORY_KEY, &mut config);
    config.global_notes = target_value(&table, GLOBAL_KEY, &mut config);

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

fn target_value(table: &Table, key: &str, config: &mut ConfigFile) -> MemoryNotesTarget {
    match table.get(key).map(|value| value.as_str()) {
        None => MemoryNotesTarget::Remote,
        Some(Some("remote")) => MemoryNotesTarget::Remote,
        Some(Some("local")) => MemoryNotesTarget::Local,
        Some(_) => {
            config.note(format!("{key} must be \"remote\" or \"local\""));
            MemoryNotesTarget::Remote
        }
    }
}

/// The file's text: the URL, the token when there is one, and both routing keys.
pub(super) fn render(
    url: &str,
    token: Option<&str>,
    repository_notes: MemoryNotesTarget,
    global_notes: MemoryNotesTarget,
) -> String {
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

    if let Some(token) = token {
        line(TOKEN_KEY, token);
    }

    line(REPOSITORY_KEY, target_name(repository_notes));
    line(GLOBAL_KEY, target_name(global_notes));

    text
}

fn target_name(target: MemoryNotesTarget) -> &'static str {
    match target {
        MemoryNotesTarget::Remote => "remote",
        MemoryNotesTarget::Local => "local",
    }
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

    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);

    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);

    builder
        .create(dir)
        .map_err(|error| BusinessError::from_io("creating the Harness Memory config folder", error))
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

fn refuse_link(path: &Path) -> Result<()> {
    let is_link = std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink());

    if is_link {
        return Err(BusinessError::InvalidInput(format!(
            "{} is a symbolic link; the app does not write through it",
            path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default()
        )));
    }

    Ok(())
}

/// What Harness Memory would say about the file's permissions (Unix; `None` elsewhere).
#[cfg(unix)]
fn permission_problem(path: &Path) -> Option<String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = std::fs::metadata(path).ok()?;
    let mode = metadata.permissions().mode();

    if mode & 0o022 != 0 {
        return Some(
            "other users can change the file, so Harness Memory refuses it; save here or run chmod 600 on it"
                .to_owned(),
        );
    }

    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Some(
            "the file belongs to another user, so Harness Memory refuses it; save here to replace it".to_owned(),
        );
    }

    (mode & 0o044 != 0).then(|| {
        "other users can read the file, which holds the database credentials; save here or run chmod 600 on it"
            .to_owned()
    })
}

#[cfg(not(unix))]
fn permission_problem(_path: &Path) -> Option<String> {
    None
}
