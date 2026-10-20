//! Removing an OpenCode plugin (`ExtensionAction::PluginUninstall`). OpenCode has no uninstall
//! command, so the app removes the plugin's entry from the `plugin` list of the global config file,
//! the one the setup step registers MCP servers in ([`mcp_target`]: `opencode.jsonc` when present,
//! else `opencode.json`). It never edits another file.
//!
//! - `name` is the plugin's id as the extensions view lists it: the package name of an npm spec
//!   (`demo`, `demo@1.2` and `demo@latest` all match `demo`), the file path of a local one (in any
//!   form of that path: `/` or `\` separators, a `\\?\` prefix or a lower-case drive letter).
//! - Every string entry with that id goes; other keys and entries stay, in order. The write follows
//!   [`change_file`]: atomic, permissions kept, and a symlink, anything but a regular file, a file
//!   over 2 MiB or one that is not valid JSONC is refused (an error, nothing written).
//! - Left to the user ([`ConfigEdit::manual_edit`], `ok` false, nothing written): a file with
//!   comments a rewrite would lose, a `[spec, options]` entry for the plugin, or a plugin that only
//!   another file defines (another settings file, the project's, or a plugin file).
//! - Not listed anywhere: `ok`, nothing written, so removing twice is harmless.
//! - Removed (or a dry run that would remove) while another file still defines the plugin: the
//!   summary names that file, since OpenCode keeps loading the plugin from it.
//! - A dry run reads the file and reports what the run would do. It never writes.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::OpenCode;
use super::config::merge_steps;
use super::extensions::{plugin_entry_id, plugin_sources, same_plugin_id};
use crate::ecosystem::mcp_merge::{Blocked, Change, Shape, Target, change_file, load};
use crate::ecosystem::setup::mcp_target;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::MAX_CLI_TARGET_CHARS;
use crate::model::{CliResult, ConfigEdit, McpRefusal};
use crate::pi_family::tilde;
use crate::security::redact_text;
use crate::sync::validate::display_safe;

/// The key listing OpenCode's plugins.
const PLUGIN_KEY: &str = "plugin";

/// What a removal found in the global file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Found {
    /// String entries with the id (removed, or that a dry run would remove).
    Entries,
    /// A `[spec, options]` entry with the id.
    WithOptions,
    /// No entry with the id.
    Nothing,
    /// The `plugin` key holds something other than a list.
    NotAList,
    /// String entries with the id, in a file with comments a rewrite would lose (nothing written).
    Comments,
}

/// What the action did or would do.
enum Outcome {
    /// Removed from the global file; `still_in` is another file OpenCode still loads it from.
    Removed {
        still_in: Option<PathBuf>,
    },
    NotListed,
    Manual {
        file: PathBuf,
        instruction: String,
    },
}

/// Removes the plugin `name` from the global config file, or previews that (see the module docs).
/// `project` only widens the search for a plugin defined elsewhere.
pub fn remove_plugin(env: &Env, name: &str, project: Option<&Path>, dry_run: bool) -> Result<CliResult> {
    check_name(name)?;

    let file = mcp_target(env, &OpenCode)
        .map(|target| target.path)
        .ok_or_else(|| BusinessError::Internal("OpenCode has no global config file".to_owned()))?;
    let shown = tilde(env, &file);

    let found = if dry_run {
        preview(&file, name)
    } else {
        apply(&file, name)
    };

    let outcome = match found.map_err(|reason| refused(&shown, reason))? {
        Found::Entries => Outcome::Removed {
            still_in: other_source(env, project, &file, name),
        },
        Found::Nothing => elsewhere(env, project, &file, name).unwrap_or(Outcome::NotListed),
        Found::WithOptions => Outcome::Manual {
            instruction: format!("{shown} sets options for this plugin. Remove its entry from \"plugin\" yourself."),
            file: file.clone(),
        },
        Found::NotAList => {
            return Err(BusinessError::InvalidInput(format!(
                "\"plugin\" in {shown} is not a list"
            )));
        }
        Found::Comments => Outcome::Manual {
            instruction: format!(
                "{shown} has comments the app would lose. Remove this plugin from \"plugin\" there yourself."
            ),
            file: file.clone(),
        },
    };

    Ok(result(env, outcome, &shown, dry_run))
}

/// `name` must be a non-empty id the UI can show back (no control or invisible characters).
fn check_name(name: &str) -> Result<()> {
    if name.is_empty() || name.chars().count() > MAX_CLI_TARGET_CHARS || !display_safe(name) {
        return Err(BusinessError::InvalidInput(format!(
            "the plugin name must be non-empty, at most {MAX_CLI_TARGET_CHARS} characters, without control characters"
        )));
    }

    Ok(())
}

/// What a run would find, without writing.
fn preview(file: &Path, name: &str) -> Result<Found, McpRefusal> {
    let target = load(file, Shape::OpenCode)?;

    match decide(&target, file, name) {
        Change::Keep(found) => Ok(found),
        Change::Write(_, found) if target.rewritable() => Ok(found),
        Change::Write(..) => Ok(Found::Comments),
    }
}

/// Removes the entries, unless the file has comments.
fn apply(file: &Path, name: &str) -> Result<Found, McpRefusal> {
    match change_file(file, Shape::OpenCode, |target| decide(target, file, name)) {
        Ok(found) => Ok(found),
        Err(Blocked::ManualEdit) => Ok(Found::Comments),
        Err(Blocked::Refused(reason)) => Err(reason),
    }
}

/// Writes the document without the string entries of `name`, when there are some and no entry
/// with options for it.
fn decide(target: &Target, file: &Path, name: &str) -> Change<Found> {
    let Target::Present { document, .. } = target else {
        return Change::Keep(Found::Nothing);
    };

    let entries = match document.get(PLUGIN_KEY) {
        None => return Change::Keep(Found::Nothing),
        Some(Value::Array(entries)) => entries,
        Some(_) => return Change::Keep(Found::NotAList),
    };

    let matches = |spec: Option<&str>| spec.is_some_and(|spec| same_plugin_id(&plugin_entry_id(spec, file), name));

    let with_options = entries.iter().any(|entry| {
        entry
            .as_array()
            .is_some_and(|pair| matches(pair.first().and_then(Value::as_str)))
    });
    if with_options {
        return Change::Keep(Found::WithOptions);
    }

    let kept: Vec<Value> = entries
        .iter()
        .filter(|entry| !matches(entry.as_str()))
        .cloned()
        .collect();
    if kept.len() == entries.len() {
        return Change::Keep(Found::Nothing);
    }

    let mut document: Map<String, Value> = document.clone();
    document.insert(PLUGIN_KEY.to_owned(), Value::Array(kept));

    Change::Write(document, Found::Entries)
}

/// The first file other than `global` that defines `name` (a settings file or a plugin file).
fn other_source(env: &Env, project: Option<&Path>, global: &Path, name: &str) -> Option<PathBuf> {
    plugin_sources(&merge_steps(env, project))
        .into_iter()
        .find(|(id, path)| same_plugin_id(id, name) && path != global)
        .map(|(_, path)| path)
}

/// True for an OpenCode settings file (`*.json`, `*.jsonc`), false for a plugin file.
fn is_settings_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "json" || extension == "jsonc")
}

/// The other file that defines `name`, as an edit the user makes: OpenCode loads it from there.
fn elsewhere(env: &Env, project: Option<&Path>, global: &Path, name: &str) -> Option<Outcome> {
    let path = other_source(env, project, global, name)?;

    let shown = tilde(env, &path);
    let global = tilde(env, global);

    let instruction = if is_settings_file(&path) {
        format!("OpenCode loads this plugin from {shown}. The app only edits {global}, so remove it there yourself.")
    } else {
        format!("This plugin is the file {shown}. Delete that file to remove it.")
    };

    Some(Outcome::Manual {
        file: path,
        instruction,
    })
}

fn result(env: &Env, outcome: Outcome, shown: &str, dry_run: bool) -> CliResult {
    let (ok, summary, file, manual_edit) = match outcome {
        Outcome::Removed { still_in } => {
            let done = if dry_run {
                format!("Would remove this plugin from \"plugin\" in {shown}. Nothing was changed.")
            } else {
                format!("Removed this plugin from \"plugin\" in {shown}.")
            };

            (
                true,
                still_loaded(env, done, still_in.as_deref()),
                shown.to_owned(),
                false,
            )
        }
        Outcome::NotListed => (
            true,
            format!("This plugin is not listed in {shown}. Nothing to change."),
            shown.to_owned(),
            false,
        ),
        Outcome::Manual { file, instruction } => (false, instruction, tilde(env, &file), true),
    };

    CliResult {
        ok,
        summary: Some(redact_text(&summary)),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        truncated: false,
        timed_out: false,
        argv: Vec::new(),
        command: Vec::new(),
        config_edit: Some(ConfigEdit { file, manual_edit }),
    }
}

/// `summary`, followed by where OpenCode still loads the plugin from, if anywhere.
fn still_loaded(env: &Env, summary: String, still_in: Option<&Path>) -> String {
    let Some(path) = still_in else {
        return summary;
    };

    let other = tilde(env, path);

    if is_settings_file(path) {
        format!("{summary} It is still listed in {other}.")
    } else {
        format!("{summary} The plugin file {other} still loads it.")
    }
}

/// The error for a file [`load`] or [`change_file`] refused.
fn refused(shown: &str, reason: McpRefusal) -> BusinessError {
    match reason {
        McpRefusal::Symlink => BusinessError::Forbidden(format!("{shown} is a symlink, so the app leaves it alone")),
        McpRefusal::NotRegularFile => BusinessError::Forbidden(format!("{shown} is not a regular file")),
        McpRefusal::Unreadable => BusinessError::Forbidden(format!("{shown} cannot be read")),
        McpRefusal::TooLarge => BusinessError::InvalidInput(format!("{shown} is larger than 2 MiB")),
        McpRefusal::Unparsable | McpRefusal::NotAnObject | McpRefusal::ServersNotAnObject => {
            BusinessError::InvalidInput(format!("{shown} is not a valid OpenCode config file"))
        }
        McpRefusal::WriteFailed => BusinessError::Io {
            context: format!("could not update {shown}, nothing was changed"),
            source: io::Error::other("the write failed or the file kept changing"),
        },
    }
}
