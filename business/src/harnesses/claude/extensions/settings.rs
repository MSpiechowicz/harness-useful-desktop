//! The settings the extensions view needs from every layer the config view lists
//! ([`settings_files`], [`managed_files`]): plugins switched on or off, marketplaces named in
//! settings, and the answers for the project's `.mcp.json` servers. Claude applies the layers
//! lowest first (user, project, local, managed settings and then their drop-ins by name), so a
//! later layer wins.

use std::path::Path;

use serde_json::{Map, Value};

use super::super::config::{SettingsFile, managed_files, settings_files};
use crate::env::Env;
use crate::model::ConfigLayer;
use crate::pi_family::{Document, read_json, string_list};

/// The parsed settings layers, lowest first. Unreadable files are left out (the config view
/// reports them).
pub(in crate::harnesses::claude) struct Settings {
    layers: Vec<Value>,
}

impl Settings {
    pub(super) fn load(env: &Env, project: Option<&Path>) -> Self {
        Self::read(settings_files(env, project).into_iter().chain(managed_files(env)))
    }

    /// The user's own settings file only (`<config>/settings.json`).
    pub(in crate::harnesses::claude) fn user(env: &Env) -> Self {
        Self::read(
            settings_files(env, None)
                .into_iter()
                .filter(|file| file.layer == ConfigLayer::User),
        )
    }

    fn read(files: impl Iterator<Item = SettingsFile>) -> Self {
        let layers = files
            .filter_map(|file| match read_json(&file.path) {
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

    /// Every layer's string list at `key`, combined.
    pub(super) fn lists(&self, key: &str) -> Vec<String> {
        self.layers
            .iter()
            .flat_map(|layer| string_list(layer.get(key)))
            .collect()
    }
}
