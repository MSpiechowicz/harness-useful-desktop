//! OpenCode plugins: the plugin entries of each settings file (npm specs, or local paths) and the
//! plugins in `plugin/` and `plugins/` of each config directory. Nothing is installed or resolved.
//!
//! - OpenCode 1 (`config/config.ts`, 1.19.3) reads the `plugin` key; an entry is a spec or
//!   `[spec, options]`. `file://` URLs, paths starting with `.` (relative to the settings file)
//!   and absolute paths are local files. Each directory adds its `*.ts` / `*.js` files. Files and
//!   directories count in merge order.
//! - OpenCode 2 (`core/src/config/plugin/source.ts`, 2.1.2) reads `plugin` and `plugins`; an entry
//!   is a spec, `[spec, options]` or `{package, options}`, and `-<target>` removes a plugin. Only
//!   `./` and `../` paths are relative, and a configured local plugin must be a directory (a file
//!   is skipped). Each directory adds its `*.ts` / `*.js` files and its subdirectories. Directory
//!   plugins come first, then the configured ones, so an entry can remove a discovered plugin.
//!
//! A plugin listed again (same npm package or same file) keeps its place and takes the later scope.

use std::path::{Path, PathBuf};

use super::super::config::{Layers, PluginEntry, Step, plugin_entries};
use super::super::plugin_ids::file_url_drive_path;
use crate::harness::OpenCodeGeneration;
use crate::model::{ExtensionScope, PluginInfo};
use crate::pi_family::list_dir;
use crate::security::redact_text;

/// What one entry or directory does to the plugin list.
enum Change {
    /// Adds the plugin, defined in the file (a settings file or the plugin itself).
    Add(PluginInfo, PathBuf),
    /// Removes the plugin with this id (OpenCode 2).
    Remove(String),
}

/// See the module docs.
pub(in crate::harnesses::opencode) fn plugins(layers: &Layers) -> Vec<PluginInfo> {
    let mut plugins: Vec<PluginInfo> = Vec::new();

    for change in changes(layers) {
        match change {
            Change::Add(plugin, _) => match plugins.iter_mut().find(|existing| existing.id == plugin.id) {
                Some(existing) => *existing = plugin,
                None => plugins.push(plugin),
            },
            Change::Remove(id) => plugins.retain(|plugin| plugin.id != id),
        }
    }

    plugins
}

/// Every place that defines a plugin, in merge order: the plugin's id as [`plugins`] lists it, and
/// the settings file naming it or the plugin file itself.
pub(in crate::harnesses::opencode) fn plugin_sources(layers: &Layers) -> Vec<(String, PathBuf)> {
    changes(layers)
        .into_iter()
        .filter_map(|change| match change {
            Change::Add(plugin, source) => Some((plugin.id, source)),
            Change::Remove(_) => None,
        })
        .collect()
}

/// The id `generation` lists the plugin entry `spec` of `config_file` under.
pub(in crate::harnesses::opencode) fn plugin_entry_id(
    spec: &str,
    config_file: &Path,
    generation: OpenCodeGeneration,
) -> String {
    match configured_plugin(spec, config_file, ExtensionScope::User, generation) {
        Some(plugin) => plugin.id,
        None => spec.to_owned(),
    }
}

/// The changes of every step in the order the generation applies them.
fn changes(layers: &Layers) -> Vec<Change> {
    let generation = layers.generation;
    let mut steps: Vec<(bool, Vec<Change>)> = layers
        .steps
        .iter()
        .map(|step| match step {
            Step::File { path, place, document } => {
                let entries = document.value().map(|value| plugin_entries(value, generation));
                let changes = entries.into_iter().flatten();
                let found = changes.filter_map(|entry| entry_change(entry, path, place.scope(), generation));

                (false, found.collect())
            }
            Step::Dir(dir) => (true, dir_plugins(&dir.path, dir.place.scope(), generation)),
        })
        .collect();

    if generation == OpenCodeGeneration::V2 {
        // A stable sort: directories first, each group in merge order.
        steps.sort_by_key(|(is_dir, _)| !is_dir);
    }

    steps.into_iter().flat_map(|(_, changes)| changes).collect()
}

fn entry_change(
    entry: PluginEntry,
    config_file: &Path,
    scope: ExtensionScope,
    generation: OpenCodeGeneration,
) -> Option<Change> {
    match entry {
        PluginEntry::Spec(spec) | PluginEntry::WithOptions(Some(spec)) => {
            let plugin = configured_plugin(&spec, config_file, scope, generation)?;
            Some(Change::Add(plugin, config_file.to_path_buf()))
        }
        PluginEntry::Remove(target) => {
            let plugin = configured_plugin(&target, config_file, scope, generation)?;
            Some(Change::Remove(plugin.id))
        }
        PluginEntry::WithOptions(None) | PluginEntry::Other => None,
    }
}

/// The plugins of a config directory's `plugin/` and `plugins/` (see the module docs).
fn dir_plugins(dir: &Path, scope: ExtensionScope, generation: OpenCodeGeneration) -> Vec<Change> {
    let mut entries: Vec<_> = ["plugin", "plugins"]
        .iter()
        .flat_map(|name| list_dir(&dir.join(name)))
        .filter(|entry| {
            let source = entry.name.ends_with(".ts") || entry.name.ends_with(".js");
            (entry.is_file && source) || (entry.is_dir && generation == OpenCodeGeneration::V2)
        })
        .collect();

    if generation == OpenCodeGeneration::V2 {
        entries.sort_by(|left, right| left.path.cmp(&right.path));
    }

    entries
        .into_iter()
        .map(|entry| Change::Add(local_plugin(&entry.path, scope), entry.path))
        .collect()
}

/// A configured plugin: a local file or directory for a path, else an npm package; `None` for a
/// local file on OpenCode 2. `components` drops the `.` of `./plugin.ts` and, on Windows, turns
/// `C:/x/plugin.ts` into `C:\x\plugin.ts`, so the file matches its directory scan.
fn configured_plugin(
    spec: &str,
    config_file: &Path,
    scope: ExtensionScope,
    generation: OpenCodeGeneration,
) -> Option<PluginInfo> {
    let relative = match generation {
        OpenCodeGeneration::V1 => spec.starts_with('.'),
        OpenCodeGeneration::V2 => spec.starts_with("./") || spec.starts_with("../"),
    };

    let local: Option<PathBuf> = match spec.strip_prefix("file://") {
        Some(path) if cfg!(windows) => Some(Path::new(file_url_drive_path(path)).components().collect()),
        Some(path) => Some(Path::new(path).components().collect()),
        None if relative => config_file.parent().map(|dir| dir.join(spec).components().collect()),
        None if Path::new(spec).is_absolute() => Some(Path::new(spec).components().collect()),
        None => None,
    };

    if let Some(path) = local {
        if generation == OpenCodeGeneration::V2 && path.is_file() {
            return None;
        }

        return Some(local_plugin(&path, scope));
    }

    let (name, version) = npm_package(spec);
    let name = redact_text(name);

    Some(PluginInfo {
        id: name.clone(),
        name,
        version: version.map(redact_text),
        description: None,
        enabled: true,
        scope,
        marketplace: None,
        features: Vec::new(),
        path: None,
        latest_version: None,
        update_available: false,
    })
}

/// Package name and requested version of an npm spec (`name`, `name@1.2`, `@scope/name@^1`).
/// Other specs (git URLs, aliases) are the name as a whole.
fn npm_package(spec: &str) -> (&str, Option<&str>) {
    if spec.contains(':') {
        return (spec, None);
    }

    let name_start = usize::from(spec.starts_with('@'));
    match spec[name_start..].split_once('@') {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => {
            (&spec[..name_start + name.len()], Some(version))
        }
        _ => (spec, None),
    }
}

/// A local plugin, named after its file stem (a directory after its whole name).
fn local_plugin(path: &Path, scope: ExtensionScope) -> PluginInfo {
    let name = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    };
    let path = path.to_string_lossy().into_owned();
    let name = name.map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());

    PluginInfo {
        id: path.clone(),
        name,
        version: None,
        description: None,
        enabled: true,
        scope,
        marketplace: None,
        features: Vec::new(),
        path: Some(path),
        latest_version: None,
        update_available: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_specs_split_into_package_and_version() {
        assert_eq!(npm_package("opencode-wakatime"), ("opencode-wakatime", None));
        assert_eq!(
            npm_package("opencode-wakatime@1.2.0"),
            ("opencode-wakatime", Some("1.2.0"))
        );
        assert_eq!(npm_package("@acme/plugin"), ("@acme/plugin", None));
        assert_eq!(npm_package("@acme/plugin@^2"), ("@acme/plugin", Some("^2")));
        assert_eq!(npm_package("github:acme/plugin"), ("github:acme/plugin", None));
    }

    #[test]
    fn only_dot_slash_paths_are_relative_on_opencode_2() {
        let file = Path::new("/cfg/opencode.json");
        let id = |spec, generation| {
            configured_plugin(spec, file, ExtensionScope::User, generation)
                .unwrap()
                .id
        };

        assert_eq!(id(".hidden", OpenCodeGeneration::V2), ".hidden");
        assert_ne!(id(".hidden", OpenCodeGeneration::V1), ".hidden");
        assert_eq!(
            PathBuf::from(id("./local", OpenCodeGeneration::V2)),
            Path::new("/cfg/local")
        );
    }
}
