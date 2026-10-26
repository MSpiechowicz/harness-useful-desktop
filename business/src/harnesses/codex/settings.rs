//! Codex's `config.toml` files (Codex CLI 0.154), read through
//! [`SafeReader`](crate::security::SafeReader) and parsed into JSON values in source order.
//!
//! - User: `<home>/config.toml` ([`user_config_path`]).
//! - Project: `<project>/.codex/config.toml`, which Codex applies only in a folder it trusts
//!   (`[projects."<path>"] trust_level = "trusted"` in the user file, for the folder itself or
//!   the root of its git repository, see [`is_trusted`]). A project file that resolves outside
//!   the project is not read.
//!
//! The values are only read here: nothing is ever written to these files by the app.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

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

/// Whether the user file trusts `project`: Codex looks up the folder itself and the root of its
/// git repository (the main checkout for a worktree), never any other folder above it. Both are
/// physical paths: a chat runs in the canonical project folder, and Codex resolves the repository
/// root from the canonical git directory, so a folder that cannot be resolved is not trusted.
pub(super) fn is_trusted(user: &Document, project: &Path) -> bool {
    let Some(projects) = user.get("projects").and_then(Value::as_object) else {
        return false;
    };
    let Ok(project) = dunce::canonicalize(project) else {
        return false;
    };
    let trusted = |dir: &Path| {
        projects
            .get(dir.to_string_lossy().as_ref())
            .and_then(|entry| text_field(entry, "trust_level"))
            .is_some_and(|level| level == "trusted")
    };

    trusted(&project) || repository_root(&project).is_some_and(|root| trusted(&root))
}

/// The root of the git repository holding the canonical `dir`, as Codex resolves it: the folder
/// above the canonical common git directory of the nearest `.git` (for a worktree, a `.git` file,
/// the one its `commondir` names). `None` when that directory cannot be resolved.
fn repository_root(dir: &Path) -> Option<PathBuf> {
    let checkout = dir.ancestors().find(|folder| folder.join(".git").exists())?;
    let marker = checkout.join(".git");

    let git_dir = if marker.is_dir() {
        marker
    } else {
        let text = std::fs::read_to_string(&marker).ok()?;
        checkout.join(text.trim().strip_prefix("gitdir:")?.trim())
    };

    let common = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(relative) => git_dir.join(relative.trim()),
        Err(_) => git_dir,
    };
    let common = dunce::canonicalize(&common).ok()?;

    common.parent().map(Path::to_path_buf)
}

/// The MCP servers of the settings that apply to a chat in `project` with its Codex home in
/// `codex_home`: the user file's, then the project file's in a trusted folder or one trusted
/// once (`trust_once`), each merged over the user's server of the same name.
pub fn mcp_servers(codex_home: &Path, project: &Path, trust_once: bool) -> Map<String, Value> {
    let user_path = codex_home.join(CONFIG_FILE);
    let user = read_document(&user_path, ConfigFormat::Toml);
    let mut servers = servers_of(&user);

    let project_layer = trust_once || is_trusted(&user, project);
    if project_layer && project_config_path(project) != user_path {
        for (name, server) in servers_of(&project_config(project)) {
            let merged = servers.entry(name).or_insert_with(|| Value::Object(Map::new()));

            if let (Value::Object(merged), Value::Object(server)) = (merged, server) {
                merged.extend(server);
            }
        }
    }

    servers
}

/// The `mcp_servers` table of `document`.
fn servers_of(document: &Document) -> Map<String, Value> {
    document
        .get("mcp_servers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// True for a server name a `-c` key path can name (`mcp_servers.<name>.…`): letters, digits,
/// `-` and `_` (Codex splits the path at every dot).
pub fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
}

/// The settings that apply, in Codex's order: the user file, the project file in a trusted
/// folder, then the profile the last of them selects (`profile = "<name>"`, a `[profiles.<name>]`
/// table of the same file or the user's).
pub(super) struct Layers {
    values: Vec<Value>,
    /// Whether the user file trusts the project (no project: true).
    pub trusted: bool,
}

impl Layers {
    /// The layers that apply to `project` (the user file alone without one).
    pub(super) fn load(env: &Env, project: Option<&Path>) -> Self {
        let user = user_config(env);
        let mut files: Vec<Value> = user.value().cloned().into_iter().collect();
        let trusted = project.is_none_or(|project| is_trusted(&user, project));

        if let Some(project) = project.filter(|_| trusted) {
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
        Self { values: files, trusted }
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
