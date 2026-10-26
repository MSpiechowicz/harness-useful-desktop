//! The settings the extensions view needs from every layer the config view lists
//! ([`settings_files`], [`managed_files`]): plugins switched on or off, marketplaces named in
//! settings, and the answers for the project's `.mcp.json` servers (and the permission mode, for
//! the chat). Claude applies the layers
//! lowest first (user, project, local, managed settings and then their drop-ins by name), so a
//! later layer wins.

use std::path::Path;

use serde_json::{Map, Value};

use super::super::config::{SettingsFile, managed_files, settings_files};
use crate::env::Env;
use crate::pi_family::{Document, string_list};

/// The parsed settings layers, lowest first. Unreadable files are left out (the config view
/// reports them).
pub(in crate::harnesses::claude) struct Settings {
    layers: Vec<Value>,
}

impl Settings {
    pub(in crate::harnesses::claude) fn load(env: &Env, project: Option<&Path>) -> Self {
        Self::read(settings_files(env, project).into_iter().chain(managed_files(env)))
    }

    fn read(files: impl Iterator<Item = SettingsFile>) -> Self {
        let layers = files
            .filter_map(|file| match file.read() {
                Document::Parsed(value) => Some(value),
                _ => None,
            })
            .collect();

        Self { layers }
    }

    /// The object at `key` merged entry by entry, a later layer replacing an entry.
    pub(in crate::harnesses::claude) fn merged(&self, key: &str) -> Map<String, Value> {
        let mut merged = Map::new();

        for entries in self.layers.iter().filter_map(|layer| layer.get(key)?.as_object()) {
            merged.extend(entries.iter().map(|(name, value)| (name.clone(), value.clone())));
        }

        merged
    }

    /// The boolean at `key` of the last layer that sets one.
    pub(super) fn flag(&self, key: &str) -> Option<bool> {
        self.layers.iter().rev().find_map(|layer| layer.get(key)?.as_bool())
    }

    /// The strings at JSON `pointer` (`/permissions/defaultMode`) of the layers that set one,
    /// lowest first.
    pub(in crate::harnesses::claude) fn texts(&self, pointer: &str) -> Vec<&str> {
        self.layers
            .iter()
            .filter_map(|layer| layer.pointer(pointer)?.as_str())
            .collect()
    }

    /// Every layer's string list at `key`, combined.
    pub(super) fn lists(&self, key: &str) -> Vec<String> {
        self.layers
            .iter()
            .flat_map(|layer| string_list(layer.get(key)))
            .collect()
    }
}
