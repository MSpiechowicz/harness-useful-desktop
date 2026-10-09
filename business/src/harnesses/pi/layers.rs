//! PI settings layers and the resource paths they list (`skills`, `prompts`, `themes`, ...).
//!
//! User settings resolve relative paths from the agent directory, project settings from the
//! project `.pi` directory; `~` is expanded. Entry prefixes: `+path` includes exactly, `-path`
//! excludes exactly, `!glob` excludes by pattern (not evaluated here), `builtin:<name>` names a
//! built-in extension (not a path).

use std::path::PathBuf;

use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::ExtensionScope;
use crate::pi_family::{Document, read_json, string_list};

/// One settings file and how its relative paths resolve.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Scope of resources configured here.
    pub scope: ExtensionScope,
    /// Directory relative resource paths resolve from.
    pub base: PathBuf,
    /// The parsed `settings.json`.
    pub settings: Document,
}

/// A resource path listed in settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listed {
    /// Load this path.
    Include(PathBuf),
    /// Exclude this exact path.
    Exclude(PathBuf),
}

/// The project layer (when a project is open) followed by the user layer; project settings
/// override user settings, resource lists combine.
pub fn layers(paths: &HarnessPaths) -> Vec<Layer> {
    let mut layers = Vec::new();

    if let Some(dir) = &paths.project_dir {
        layers.push(Layer {
            scope: ExtensionScope::Project,
            base: dir.clone(),
            settings: read_json(&dir.join("settings.json")),
        });
    }

    layers.push(Layer {
        scope: ExtensionScope::User,
        base: paths.agent_dir.clone(),
        settings: read_json(&paths.agent_dir.join("settings.json")),
    });

    layers
}

/// The resource paths listed under `key` in `layer` (globs and built-ins are skipped).
pub fn listed(env: &Env, layer: &Layer, key: &str) -> Vec<Listed> {
    string_list(layer.settings.get(key))
        .iter()
        .filter_map(|entry| {
            if entry.starts_with('!') || entry.contains("builtin:") {
                return None;
            }

            if let Some(path) = entry.strip_prefix('-') {
                return Some(Listed::Exclude(resolve(env, layer, path)));
            }

            let path = entry.strip_prefix('+').unwrap_or(entry);
            Some(Listed::Include(resolve(env, layer, path)))
        })
        .collect()
}

/// The included paths only.
pub fn included(env: &Env, layer: &Layer, key: &str) -> Vec<PathBuf> {
    listed(env, layer, key)
        .into_iter()
        .filter_map(|entry| match entry {
            Listed::Include(path) => Some(path),
            Listed::Exclude(_) => None,
        })
        .collect()
}

/// `raw` with `~` expanded and relative paths resolved from the layer base.
pub fn resolve(env: &Env, layer: &Layer, raw: &str) -> PathBuf {
    let expanded = env.expand_home(raw);

    if expanded.is_absolute() {
        expanded
    } else {
        layer.base.join(expanded)
    }
}
