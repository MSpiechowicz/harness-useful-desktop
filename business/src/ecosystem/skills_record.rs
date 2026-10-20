//! The Harness Skills for OpenCode on this machine. They have no binary to ask for a version: their
//! installer (`install.sh`, `install.ps1`) writes an install record,
//! `<OpenCode config dir>/plugins/useful-skills/install-record.json`, and that record is what counts
//! as installed.
//!
//! The record counts only when it is a regular file (a symlink does not count) of at most
//! [`MAX_RECORD_BYTES`] holding a JSON object whose `version` is a version string: at most
//! [`MAX_VERSION_CHARS`] ASCII letters, digits, `.`, `-` and `+`, starting with a digit. Any other
//! record is logged and counts as not installed.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde_json::Value;

use super::installer::Installed;
use crate::env::Env;
use crate::harnesses::opencode;
use crate::security::SafeReader;

/// Size cap for reading the record.
pub const MAX_RECORD_BYTES: u64 = 256 * 1024;

/// Longest version accepted from the record.
pub const MAX_VERSION_CHARS: usize = 64;

/// Where the installer writes the record: `plugins/useful-skills/install-record.json` in OpenCode's
/// global config directory.
pub fn opencode_skills_record(env: &Env) -> PathBuf {
    opencode::config_dir(env)
        .join("plugins")
        .join("useful-skills")
        .join("install-record.json")
}

/// The installed version of the Harness Skills for OpenCode, from the install record; `None` when
/// there is no usable record (see the module docs).
pub fn opencode_skills_version(env: &Env) -> Option<String> {
    let path = opencode_skills_record(env);

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(%error, "unreadable Harness Skills install record");
            return None;
        }
    };

    if !metadata.file_type().is_file() {
        tracing::warn!("Harness Skills install record is not a regular file; treated as not installed");
        return None;
    }

    let text = match SafeReader::read_to_string(&path, MAX_RECORD_BYTES) {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!(%error, "unreadable Harness Skills install record");
            return None;
        }
    };

    let version = record_version(&text);
    if version.is_none() {
        tracing::warn!("Harness Skills install record without a valid version; treated as not installed");
    }

    version
}

/// The Harness Skills for OpenCode as found: the record's version, found without one when the
/// record is there but unusable, missing when there is none.
pub fn opencode_skills_installed(env: &Env) -> Installed {
    if let Some(version) = opencode_skills_version(env) {
        return Installed::Version(version);
    }

    if fs::symlink_metadata(opencode_skills_record(env)).is_ok() {
        return Installed::NoVersion;
    }

    Installed::Missing
}

/// The `version` of a record's text, when it is a JSON object with a valid one.
fn record_version(text: &str) -> Option<String> {
    let document: Value = serde_json::from_str(text).ok()?;
    let version = document.as_object()?.get("version")?.as_str()?;

    is_version(version).then(|| version.to_owned())
}

/// True for a version string as the module docs describe it.
fn is_version(text: &str) -> bool {
    let starts_with_digit = text.chars().next().is_some_and(|first| first.is_ascii_digit());
    let allowed = text
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '+'));

    starts_with_digit && allowed && text.len() <= MAX_VERSION_CHARS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_json_object_with_a_version_string_counts() {
        assert_eq!(record_version(r#"{"version":"0.4.0"}"#).as_deref(), Some("0.4.0"));
        assert_eq!(
            record_version(r#"{"version":"1.2.3-beta.1+build.7","files":[]}"#).as_deref(),
            Some("1.2.3-beta.1+build.7")
        );

        for text in [
            "",
            "not json",
            r#"["0.4.0"]"#,
            r#""0.4.0""#,
            "{}",
            r#"{"version":""}"#,
            r#"{"version":4}"#,
            r#"{"version":null}"#,
            r#"{"version":"v0.4.0"}"#,
            r#"{"version":" 0.4.0"}"#,
            r#"{"version":"0.4.0\n"}"#,
            r#"{"version":"0.4.0 <b>"}"#,
            r#"{"version":"0.4.0\u001b[31m"}"#,
        ] {
            assert_eq!(record_version(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_version_longer_than_the_cap_does_not_count() {
        let at_cap = format!("1.{}", "0".repeat(MAX_VERSION_CHARS - 2));
        let over_cap = format!("{at_cap}0");

        assert!(is_version(&at_cap));
        assert!(!is_version(&over_cap));
    }
}
