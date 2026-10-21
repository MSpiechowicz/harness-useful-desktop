//! Removing an OpenCode plugin (`ExtensionAction::PluginUninstall`) by editing the global config
//! file, the one the setup step registers MCP servers in ([`global_settings_file`]: OpenCode 1
//! `opencode.jsonc` when present, else `opencode.json`; OpenCode 2 the file its own `plugin add`
//! writes). OpenCode 1 has no uninstall command; OpenCode 2's `plugin remove` has no preview, so
//! both generations get the same edit. It never edits another file.
//!
//! - The lists: `plugin` on OpenCode 1; `plugin` (legacy) and `plugins` on OpenCode 2, both edited
//!   so no copy is left under the other key.
//! - `name` is the plugin's id as the extensions view lists it: the package name of an npm spec
//!   (`demo`, `demo@1.2` and `demo@latest` all match `demo`), the file path of a local one (in any
//!   form of that path: `/` or `\` separators, a `\\?\` prefix or a lower-case drive letter).
//! - Every plain entry with that id goes (a spec string, and on OpenCode 2 a `{package}` without
//!   options); other keys and entries stay, in order. The write follows [`change_file`]: atomic,
//!   permissions kept, and a symlink, anything but a regular file, a file over 2 MiB or one that is
//!   not valid JSONC is refused (an error, nothing written).
//! - Left to the user ([`ConfigEdit::manual_edit`], `ok` false, nothing written): a file with
//!   comments a rewrite would lose, an entry with options for the plugin (`[spec, options]`, on
//!   OpenCode 2 also `{package, options}`), or a plugin that only another file defines (another
//!   settings file, the project's, or a plugin file or folder).
//! - Not listed anywhere: `ok`, nothing written, so removing twice is harmless.
//! - Removed (or a dry run that would remove) while another file still defines the plugin: the
//!   summary names that file, since OpenCode keeps loading the plugin from it.
//! - A dry run reads the file and reports what the run would do. It never writes.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::config::{Layers, layers};
use super::extensions::{plugin_entry_id, plugin_sources};
use super::plugin_ids::same_plugin_id;
use super::share::global_settings_file;
use crate::ecosystem::mcp_merge::{Blocked, Change, Shape, Target, change_file, load};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{MAX_CLI_TARGET_CHARS, OpenCodeGeneration};
use crate::model::{CliResult, ConfigEdit, McpRefusal};
use crate::pi_family::tilde;
use crate::security::redact_text;
use crate::sync::validate::display_safe;

/// The keys listing plugins in a settings file of `generation`.
fn plugin_keys(generation: OpenCodeGeneration) -> &'static [&'static str] {
    match generation {
        OpenCodeGeneration::V1 => &["plugin"],
        OpenCodeGeneration::V2 => &["plugin", "plugins"],
    }
}

/// What a removal found in the global file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Found {
    /// Plain entries with the id under these keys (removed, or that a dry run would remove).
    Entries(Vec<&'static str>),
    /// An entry with options for the id, under this key.
    WithOptions(&'static str),
    /// No entry with the id.
    Nothing,
    /// This key holds something other than a list.
    NotAList(&'static str),
    /// Plain entries with the id under these keys, in a file with comments a rewrite would lose
    /// (nothing written).
    Comments(Vec<&'static str>),
}

/// What the action did or would do.
enum Outcome {
    /// Removed from these keys of the global file; `still_in` is another file OpenCode still
    /// loads it from.
    Removed {
        keys: Vec<&'static str>,
        still_in: Option<PathBuf>,
    },
    NotListed,
    Manual {
        file: PathBuf,
        instruction: String,
    },
}

/// One removal: the plugin `name` from `file`, as `generation` reads it.
#[derive(Clone, Copy)]
struct Edit<'a> {
    file: &'a Path,
    name: &'a str,
    generation: OpenCodeGeneration,
}

impl Edit<'_> {
    fn shape(self) -> Shape {
        match self.generation {
            OpenCodeGeneration::V1 => Shape::OpenCode,
            OpenCodeGeneration::V2 => Shape::OpenCodeV2,
        }
    }

    /// True when the entry spec `spec` names the plugin.
    fn matches(self, spec: Option<&str>) -> bool {
        spec.is_some_and(|spec| same_plugin_id(&plugin_entry_id(spec, self.file, self.generation), self.name))
    }
}

/// Removes the plugin `name` from the global config file of `generation`, or previews that (see
/// the module docs). `project` only widens the search for a plugin defined elsewhere.
pub fn remove_plugin(
    env: &Env,
    name: &str,
    project: Option<&Path>,
    dry_run: bool,
    generation: OpenCodeGeneration,
) -> Result<CliResult> {
    check_name(name)?;

    let file = global_settings_file(env, generation);
    let shown = tilde(env, &file);
    let edit = Edit {
        file: &file,
        name,
        generation,
    };

    let found = if dry_run { preview(edit) } else { apply(edit) };
    let layers = layers(env, project, generation);

    let outcome = match found.map_err(|reason| refused(&shown, reason))? {
        Found::Entries(keys) => Outcome::Removed {
            keys,
            still_in: other_source(&layers, &file, name),
        },
        Found::Nothing => elsewhere(env, &layers, &file, name).unwrap_or(Outcome::NotListed),
        Found::WithOptions(key) => Outcome::Manual {
            instruction: format!("{shown} sets options for this plugin. Remove its entry from \"{key}\" yourself."),
            file: file.clone(),
        },
        Found::NotAList(key) => {
            return Err(BusinessError::InvalidInput(format!(
                "\"{key}\" in {shown} is not a list"
            )));
        }
        Found::Comments(keys) => Outcome::Manual {
            instruction: format!(
                "{shown} has comments the app would lose. Remove this plugin from {} there yourself.",
                quoted(&keys)
            ),
            file: file.clone(),
        },
    };

    Ok(result(env, outcome, &shown, dry_run))
}

/// `"plugin"`, or `"plugin" and "plugins"`.
fn quoted(keys: &[&str]) -> String {
    let quoted: Vec<String> = keys.iter().map(|key| format!("\"{key}\"")).collect();

    quoted.join(" and ")
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
fn preview(edit: Edit<'_>) -> Result<Found, McpRefusal> {
    let target = load(edit.file, edit.shape())?;

    match decide(&target, edit) {
        Change::Keep(found) => Ok(found),
        Change::Write(_, found) if target.rewritable() => Ok(found),
        Change::Write(_, Found::Entries(keys)) => Ok(Found::Comments(keys)),
        Change::Write(_, found) => Ok(found),
    }
}

/// Removes the entries, unless the file has comments.
fn apply(edit: Edit<'_>) -> Result<Found, McpRefusal> {
    let mut wanted: Vec<&'static str> = Vec::new();

    let changed = change_file(edit.file, edit.shape(), |target| {
        let change = decide(target, edit);

        if let Change::Write(_, Found::Entries(keys)) = &change {
            wanted.clone_from(keys);
        }

        change
    });

    match changed {
        Ok(found) => Ok(found),
        Err(Blocked::ManualEdit) => Ok(Found::Comments(wanted)),
        Err(Blocked::Refused(reason)) => Err(reason),
    }
}

/// One plugin entry, as far as a removal cares.
enum Entry<'a> {
    /// A spec string, or on OpenCode 2 `{package}` without options.
    Plain(&'a str),
    /// An entry with options: `[spec, options]`, or on OpenCode 2 `{package, options}`.
    WithOptions(Option<&'a str>),
    /// Anything else (left alone).
    Other,
}

/// What the entry `item` is. OpenCode 1 reads any list as `[spec, options]`; OpenCode 2 reads
/// `[spec, {options}]` and `{package, options?}` (see [`super::config`]).
fn entry(item: &Value, generation: OpenCodeGeneration) -> Entry<'_> {
    match (item, generation) {
        (Value::String(spec), _) => Entry::Plain(spec),
        (Value::Array(pair), OpenCodeGeneration::V1) => Entry::WithOptions(pair.first().and_then(Value::as_str)),
        (Value::Array(pair), OpenCodeGeneration::V2) => match pair.as_slice() {
            [Value::String(spec), Value::Object(_)] => Entry::WithOptions(Some(spec)),
            _ => Entry::Other,
        },
        (Value::Object(fields), OpenCodeGeneration::V2) => {
            let package = fields.get("package").and_then(Value::as_str);

            match (package, fields.get("options")) {
                (Some(package), None) => Entry::Plain(package),
                (Some(package), Some(Value::Object(_))) => Entry::WithOptions(Some(package)),
                _ => Entry::Other,
            }
        }
        _ => Entry::Other,
    }
}

/// Writes the document without the plain entries of the plugin under every plugin key, when
/// there are some and no entry with options for it.
fn decide(target: &Target, edit: Edit<'_>) -> Change<Found> {
    let Target::Present { document, .. } = target else {
        return Change::Keep(Found::Nothing);
    };

    let mut lists: Vec<(&'static str, &Vec<Value>)> = Vec::new();
    for key in plugin_keys(edit.generation) {
        match document.get(*key) {
            None => {}
            Some(Value::Array(entries)) => lists.push((key, entries)),
            Some(_) => return Change::Keep(Found::NotAList(key)),
        }
    }

    for (key, entries) in &lists {
        let with_options = entries.iter().any(|item| match entry(item, edit.generation) {
            Entry::WithOptions(spec) => edit.matches(spec),
            _ => false,
        });

        if with_options {
            return Change::Keep(Found::WithOptions(key));
        }
    }

    let mut changed: Map<String, Value> = document.clone();
    let mut removed_from: Vec<&'static str> = Vec::new();

    for (key, entries) in lists {
        let kept: Vec<Value> = entries
            .iter()
            .filter(|item| match entry(item, edit.generation) {
                Entry::Plain(spec) => !edit.matches(Some(spec)),
                _ => true,
            })
            .cloned()
            .collect();

        if kept.len() != entries.len() {
            changed.insert(key.to_owned(), Value::Array(kept));
            removed_from.push(key);
        }
    }

    if removed_from.is_empty() {
        return Change::Keep(Found::Nothing);
    }

    Change::Write(changed, Found::Entries(removed_from))
}

/// The first file other than `global` that defines `name` (a settings file, or a plugin file or
/// folder).
fn other_source(layers: &Layers, global: &Path, name: &str) -> Option<PathBuf> {
    plugin_sources(layers)
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
fn elsewhere(env: &Env, layers: &Layers, global: &Path, name: &str) -> Option<Outcome> {
    let path = other_source(layers, global, name)?;

    let shown = tilde(env, &path);
    let global = tilde(env, global);

    let instruction = if is_settings_file(&path) {
        format!("OpenCode loads this plugin from {shown}. The app only edits {global}, so remove it there yourself.")
    } else if path.is_dir() {
        format!("This plugin is the folder {shown}. Delete that folder to remove it.")
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
        Outcome::Removed { keys, still_in } => {
            let keys = quoted(&keys);
            let done = if dry_run {
                format!("Would remove this plugin from {keys} in {shown}. Nothing was changed.")
            } else {
                format!("Removed this plugin from {keys} in {shown}.")
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
    } else if path.is_dir() {
        format!("{summary} The plugin folder {other} still loads it.")
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
