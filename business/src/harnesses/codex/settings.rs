//! Codex's `config.toml` files (Codex CLI 0.154), read through
//! [`SafeReader`](crate::security::SafeReader) and parsed into JSON values in source order.
//!
//! - User: `<home>/config.toml` ([`user_config_path`]).
//! - Project: `<project>/.codex/config.toml`, which Codex applies only in a folder it trusts
//!   (`[projects."<path>"] trust_level = "trusted"` in the user file, for the folder or a folder
//!   above it). A project file that resolves outside the project is not read.
//!
//! The values are only read here: nothing is ever written to these files by the app.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::paths::{PROJECT_DIR_NAME, codex_home};
use crate::env::Env;
use crate::model::ConfigFormat;
use crate::pi_family::{Document, read_document, text_field};
use crate::security::canonical_within;

/// The settings file name, in the Codex home and in a project's `.codex`.
pub const CONFIG_FILE: &str = "config.toml";

/// Why a project file is not read.
pub(super) const OUTSIDE_PROJECT: &str = "The file resolves outside the project, so it is not read";

/// The user's `config.toml`.
pub fn user_config_path(env: &Env) -> PathBuf {
    codex_home(env).join(CONFIG_FILE)
}

/// The parsed user `config.toml`.
pub(super) fn user_config(env: &Env) -> Document {
    read_document(&user_config_path(env), ConfigFormat::Toml)
}

/// The project's `.codex/config.toml`.
pub(super) fn project_config_path(project: &Path) -> PathBuf {
    project.join(PROJECT_DIR_NAME).join(CONFIG_FILE)
}

/// The parsed project file; [`Document::Invalid`] when it resolves outside the project.
pub(super) fn project_config(project: &Path) -> Document {
    let path = project_config_path(project);

    if path.exists() && canonical_within(project, &path).is_err() {
        return Document::Invalid(OUTSIDE_PROJECT.to_owned());
    }

    read_document(&path, ConfigFormat::Toml)
}

/// Whether the user file trusts `project` or a folder above it.
pub(super) fn is_trusted(user: &Document, project: &Path) -> bool {
    let Some(projects) = user.get("projects").and_then(Value::as_object) else {
        return false;
    };

    project.ancestors().any(|dir| {
        projects
            .get(dir.to_string_lossy().as_ref())
            .and_then(|entry| text_field(entry, "trust_level"))
            .is_some_and(|level| level == "trusted")
    })
}

/// The settings that apply, in Codex's order: the user file, the project file in a trusted
/// folder, then the profile the last of them selects (`profile = "<name>"`, a `[profiles.<name>]`
/// table of the same file or the user's).
pub(super) struct Layers {
    values: Vec<Value>,
}

impl Layers {
    /// The layers that apply to `project` (the user file alone without one).
    pub(super) fn load(env: &Env, project: Option<&Path>) -> Self {
        let user = user_config(env);
        let mut files: Vec<Value> = user.value().cloned().into_iter().collect();

        if let Some(project) = project.filter(|project| is_trusted(&user, project)) {
            let same_as_user = project_config_path(project) == user_config_path(env);

            if !same_as_user && let Document::Parsed(value) = project_config(project) {
                files.push(value);
            }
        }

        let profile = files.iter().rev().find_map(|file| text_field(file, "profile"));
        let selected = profile.and_then(|name| {
            files
                .iter()
                .rev()
                .find_map(|file| file.get("profiles").and_then(|profiles| profiles.get(&name)).cloned())
        });

        files.extend(selected);
        Self { values: files }
    }

    /// The last string the layers set for `key`.
    pub(super) fn text(&self, key: &str) -> Option<String> {
        self.values.iter().rev().find_map(|value| text_field(value, key))
    }

    /// The last value the layers set for `key`.
    pub(super) fn value(&self, key: &str) -> Option<&Value> {
        self.values.iter().rev().find_map(|value| value.get(key))
    }
}
