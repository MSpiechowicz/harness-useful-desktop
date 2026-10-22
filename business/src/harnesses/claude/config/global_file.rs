//! The part of Claude Code's global file (`~/.claude.json`, see [`global_config_file`]) the view
//! shows. The file also holds the account, device ids, caches and every other project's state,
//! so it is read through an allow-list: the user's MCP servers (top-level `mcpServers`) and, for
//! the open project, a few keys of its `projects` entry. Nothing else leaves this module. It is the
//! one file read with [`read_global_file`] (a name every other read refuses, and a cap of
//! [`MAX_GLOBAL_FILE_BYTES`]); a larger file is reported as such, never as an empty one.

use std::path::Path;

use serde_json::{Map, Value};

use super::super::paths::{MAX_GLOBAL_FILE_BYTES, global_config_file, read_global_file};
use super::files::SettingsFile;
use crate::env::Env;
use crate::error::BusinessError;
use crate::model::{ConfigFormat, ConfigLayer};
use crate::pi_family::{Document, parse_text};

/// Top-level keys shown.
const USER_KEYS: &[&str] = &["mcpServers"];

/// Keys of the open project's entry shown: local MCP servers, the `.mcp.json` approvals, the
/// approved tools and the trust answer.
const PROJECT_KEYS: &[&str] = &[
    "mcpServers",
    "enabledMcpjsonServers",
    "disabledMcpjsonServers",
    "disabledMcpServers",
    "allowedTools",
    TRUST_KEY,
];

/// The project entry's answer to Claude Code's trust dialog.
const TRUST_KEY: &str = "hasTrustDialogAccepted";

/// Most project keys resolved on disk when none names the project as given.
const MAX_RESOLVED_KEYS: usize = 256;

/// The global file's sections, each with its allow-listed content.
#[derive(Debug, Default)]
pub(in crate::harnesses::claude) struct GlobalSections {
    /// The user's MCP servers. Present whenever the file exists, with the read or parse error if any.
    pub user: Option<(SettingsFile, Document)>,
    /// The open project's entry, when the file names the project.
    pub project: Option<(SettingsFile, Document)>,
}

/// Reads the global file of `env` (see the module docs).
pub(in crate::harnesses::claude) fn global_sections(env: &Env, project: Option<&Path>) -> GlobalSections {
    let path = global_config_file(env);

    let root = match read(&path) {
        Document::Missing => return GlobalSections::default(),
        Document::Parsed(Value::Object(root)) => root,
        Document::Parsed(_) => {
            let error = Document::Invalid("Expected a key/value object at the top level".to_owned());
            return user_only(&path, error);
        }
        invalid @ Document::Invalid(_) => return user_only(&path, invalid),
    };

    let project = project.and_then(|project| {
        let projects = root.get("projects")?.as_object()?;
        let entry = project_entry(projects, project)?.as_object()?;

        Some((project_file(&path), Document::Parsed(allowed(entry, PROJECT_KEYS))))
    });

    GlobalSections {
        user: Some((user_file(&path), Document::Parsed(allowed(&root, USER_KEYS)))),
        project,
    }
}

/// What Claude Code's global file says about trusting a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::harnesses::claude) enum ProjectTrust {
    /// The user accepted Claude Code's trust dialog for the folder.
    Trusted,
    /// No file, no entry for the folder, or an entry without an accepted trust dialog.
    Untrusted,
    /// The file exists but could not be read: unreadable, invalid or too large.
    Unreadable,
}

/// Whether the user accepted Claude Code's trust dialog for `project`: its `projects` entry in the
/// global file has `hasTrustDialogAccepted: true`. Only the entry of the folder itself counts, as
/// trust is per folder. A file that exists but cannot be read trusts nothing either, reported as
/// [`ProjectTrust::Unreadable`].
pub(in crate::harnesses::claude) fn project_trust(env: &Env, project: &Path) -> ProjectTrust {
    let sections = global_sections(env, Some(project));

    if let Some((_, Document::Parsed(entry))) = &sections.project {
        if entry.get(TRUST_KEY) == Some(&Value::Bool(true)) {
            return ProjectTrust::Trusted;
        }

        return ProjectTrust::Untrusted;
    }

    match sections.user {
        Some((_, Document::Invalid(_))) => ProjectTrust::Unreadable,
        _ => ProjectTrust::Untrusted,
    }
}

/// The global file at `path` as a document: missing, parsed, or the reason it was not read.
fn read(path: &Path) -> Document {
    match read_global_file(path) {
        Ok(text) => match parse_text(&text, ConfigFormat::Json) {
            Ok(value) => Document::Parsed(value),
            Err(summary) => Document::Invalid(summary),
        },
        Err(BusinessError::NotFound(_)) => Document::Missing,
        Err(_) if is_too_large(path) => Document::Invalid(format!(
            "The file is larger than {} MiB, so it is not read",
            MAX_GLOBAL_FILE_BYTES / (1024 * 1024)
        )),
        Err(error) => Document::Invalid(error.to_string()),
    }
}

fn is_too_large(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > MAX_GLOBAL_FILE_BYTES)
}

fn user_only(path: &Path, document: Document) -> GlobalSections {
    GlobalSections {
        user: Some((user_file(path), document)),
        project: None,
    }
}

fn user_file(path: &Path) -> SettingsFile {
    SettingsFile {
        id: "user.globalFile".to_owned(),
        title: "MCP servers".to_owned(),
        description: "Your MCP servers in .claude.json, available in every project. Nothing else in this file is shown.",
        path: path.to_path_buf(),
        layer: ConfigLayer::User,
        within: None,
    }
}

fn project_file(path: &Path) -> SettingsFile {
    SettingsFile {
        id: "project.globalFile".to_owned(),
        title: "This project in .claude.json".to_owned(),
        description: "MCP servers only you use here, the .mcp.json servers you approved or turned off, approved tools and the trust answer.",
        path: path.to_path_buf(),
        layer: ConfigLayer::Project,
        within: None,
    }
}

/// The `keys` of `entries` that are set, in file order.
fn allowed(entries: &Map<String, Value>, keys: &[&str]) -> Value {
    let kept = entries
        .iter()
        .filter(|(key, _)| keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    Value::Object(kept)
}

/// The entry Claude keeps for `project`: the key that names it (with either separator), else the
/// first key that resolves to the same folder on disk (a symlinked path).
fn project_entry<'a>(projects: &'a Map<String, Value>, project: &Path) -> Option<&'a Value> {
    let canonical = dunce::canonicalize(project).ok();
    let wanted: Vec<String> = std::iter::once(project.to_path_buf())
        .chain(canonical.clone())
        .map(|path| key_form(&path.to_string_lossy()))
        .collect();

    let named = projects.iter().find(|(key, _)| wanted.contains(&key_form(key)));
    if let Some((_, entry)) = named {
        return Some(entry);
    }

    let canonical = canonical?;
    projects
        .iter()
        .filter(|(key, _)| may_resolve_to(key, &canonical))
        .take(MAX_RESOLVED_KEYS)
        .find(|(key, _)| resolves_to(key, &canonical))
        .map(|(_, entry)| entry)
}

/// True when `key` (a path Claude wrote, such as an install record's `projectPath`) names
/// `project`, with either separator or through a link to the same folder.
pub(in crate::harnesses::claude) fn names_project(key: &str, project: &Path) -> bool {
    let wanted = key_form(key);
    if wanted == key_form(&project.to_string_lossy()) {
        return true;
    }

    dunce::canonicalize(project)
        .is_ok_and(|canonical| wanted == key_form(&canonical.to_string_lossy()) || resolves_to(key, &canonical))
}

fn resolves_to(key: &str, canonical: &Path) -> bool {
    may_resolve_to(key, canonical) && dunce::canonicalize(key).is_ok_and(|resolved| resolved == canonical)
}

/// True when `key` is worth resolving on disk to compare it with `canonical`: an absolute local
/// path whose last folder has the project's name (case aside). A network path (`//host/share`,
/// `\\host\share`) is never resolved, as that would reach out to the host.
fn may_resolve_to(key: &str, canonical: &Path) -> bool {
    let path = Path::new(key);
    let network = key.starts_with("//") || key.starts_with("\\\\");

    if network || !path.is_absolute() {
        return false;
    }

    let name = |path: &Path| path.file_name().map(|name| name.to_string_lossy().to_lowercase());
    name(path).is_some() && name(path) == name(canonical)
}

/// `path` with no trailing separator, and with `/` separators on Windows (where `\\` is one too).
fn key_form(path: &str) -> String {
    let path = if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };

    match path.trim_end_matches('/') {
        "" => path,
        trimmed => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_project_entry_is_found_without_a_trailing_separator() {
        let projects = json!({
            "/w/other": { "allowedTools": ["a"] },
            "/w/demo/": { "allowedTools": ["c"] },
        });
        let projects = projects.as_object().unwrap();

        assert_eq!(
            project_entry(projects, Path::new("/w/demo")),
            Some(&json!({ "allowedTools": ["c"] }))
        );
        assert_eq!(project_entry(projects, Path::new("/w/none")), None);
    }

    #[cfg(windows)]
    #[test]
    fn the_project_entry_is_found_with_either_separator_on_windows() {
        let projects = json!({ "C:/w/demo": { "allowedTools": ["b"] } });

        assert_eq!(
            project_entry(projects.as_object().unwrap(), Path::new("C:\\w\\demo")),
            Some(&json!({ "allowedTools": ["b"] }))
        );
    }

    /// A backslash is part of a folder name outside Windows, so `/w/a\b` is not `/w/a/b`.
    #[cfg(unix)]
    #[test]
    fn a_backslash_is_no_separator_outside_windows() {
        let projects = json!({ "/w/a\\b": { "allowedTools": ["d"] } });

        assert_eq!(project_entry(projects.as_object().unwrap(), Path::new("/w/a/b")), None);
        assert!(!names_project("/w/a\\b", Path::new("/w/a/b")));
    }

    #[test]
    fn only_local_keys_naming_the_project_folder_are_resolved() {
        let canonical = Path::new("/w/demo");

        assert!(may_resolve_to("/elsewhere/Demo", canonical));
        assert!(!may_resolve_to("/elsewhere/other", canonical));
        assert!(!may_resolve_to("//host/share/demo", canonical));
        assert!(!may_resolve_to("\\\\host\\share\\demo", canonical));
        assert!(!may_resolve_to("demo", canonical));
    }

    #[test]
    fn only_allow_listed_keys_are_kept() {
        let entry = json!({ "allowedTools": [], "history": ["x"], "mcpServers": {} });

        assert_eq!(
            allowed(entry.as_object().unwrap(), PROJECT_KEYS),
            json!({ "allowedTools": [], "mcpServers": {} })
        );
    }
}
