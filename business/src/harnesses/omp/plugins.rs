//! Installed OMP plugins.
//!
//! - Marketplace installs: `<plugins>/installed_plugins.json` (user) and
//!   `<project>/.omp/plugins/installed_plugins.json` (project), Claude-compatible version 2:
//!   `{ plugins: { "name@marketplace": [{ scope, installPath, version, enabled? }] } }`.
//! - npm / linked plugins: names from `<plugins>/package.json` dependencies and the lock file,
//!   installed at `<plugins>/node_modules/<name>`.
//! - Runtime state: `<plugins>/omp-plugins.lock.json` (`plugins.<name>.{enabled, enabledFeatures}`;
//!   its `settings` may hold secrets and are never read into the view) and the project's
//!   `.omp/plugin-overrides.json` (`disabled`, `features`).
//!
//! `installPath` comes from a file, so a plugin directory is only read after it passes
//! `canonical_within` the plugins directory (or the project's `.omp/plugins`).

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::paths::plugins_dir;
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::{ExtensionScope, Notice, NoticeLevel, PluginFeature, PluginInfo};
use crate::pi_family::{Document, package_manifest, read_json, string_list, text_field};
use crate::security::canonical_within;

static PACKAGE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:@[A-Za-z0-9][A-Za-z0-9._~-]*/)?[A-Za-z0-9][A-Za-z0-9._~-]*$").expect("static regex is valid")
});

/// The plugins view plus the directories of enabled plugins (scanned for bundled skills).
#[derive(Debug, Default)]
pub struct InstalledPlugins {
    /// Installed plugins, marketplace installs first.
    pub plugins: Vec<PluginInfo>,
    /// `(plugin name, plugin directory)` of every enabled plugin whose directory was verified.
    pub enabled_roots: Vec<(String, PathBuf)>,
    /// Problems reading the registries.
    pub notices: Vec<Notice>,
    /// npm package names of the marketplace installs (so npm plugins are not listed twice).
    packages: Vec<String>,
}

/// One marketplace install.
struct Install {
    info: PluginInfo,
    /// The verified plugin directory.
    root: Option<PathBuf>,
    /// The npm package name used by the lock file and overrides.
    package: String,
}

/// Enabled flags and feature selections from the lock file and project overrides.
struct RuntimeState {
    lock: Map<String, Value>,
    disabled_in_project: Vec<String>,
    project_features: Map<String, Value>,
}

impl RuntimeState {
    fn load(plugins_dir: &Path, project_dir: Option<&Path>) -> Self {
        let lock = read_json(&plugins_dir.join("omp-plugins.lock.json"));
        let overrides = match project_dir {
            Some(dir) => read_json(&dir.join("plugin-overrides.json")),
            None => Document::Missing,
        };

        Self {
            lock: object(lock.get("plugins")),
            disabled_in_project: string_list(overrides.get("disabled")),
            project_features: object(overrides.get("features")),
        }
    }

    fn is_enabled(&self, package: &str) -> bool {
        let locked = self.lock.get(package).and_then(|state| state.get("enabled"));

        locked != Some(&Value::Bool(false)) && !self.disabled_in_project.iter().any(|name| name == package)
    }

    /// Explicit feature selection; `None` means "the manifest defaults".
    fn selected_features(&self, package: &str) -> Option<Vec<String>> {
        let selection = self
            .project_features
            .get(package)
            .or_else(|| self.lock.get(package).and_then(|state| state.get("enabledFeatures")))?;

        selection.is_array().then(|| string_list(Some(selection)))
    }
}

/// See the module docs.
pub fn installed_plugins(env: &Env, paths: &HarnessPaths) -> InstalledPlugins {
    let plugins_dir = plugins_dir(paths);
    let state = RuntimeState::load(&plugins_dir, paths.project_dir.as_deref());
    let mut found = InstalledPlugins::default();

    let mut registries = vec![(plugins_dir.join("installed_plugins.json"), ExtensionScope::User)];
    let mut allowed_roots = vec![plugins_dir.clone()];

    if let Some(dir) = &paths.project_dir {
        registries.push((
            dir.join("plugins").join("installed_plugins.json"),
            ExtensionScope::Project,
        ));
        allowed_roots.push(dir.join("plugins"));
    }

    for (registry, scope) in registries {
        let installs = Installs {
            env,
            state: &state,
            allowed_roots: &allowed_roots,
            scope,
        };
        installs.collect(&registry, &mut found);
    }

    npm_plugins(&plugins_dir, &state, &mut found);
    found
}

/// Reads one `installed_plugins.json`.
struct Installs<'a> {
    env: &'a Env,
    state: &'a RuntimeState,
    allowed_roots: &'a [PathBuf],
    scope: ExtensionScope,
}

impl Installs<'_> {
    fn collect(&self, registry: &Path, found: &mut InstalledPlugins) {
        let document = read_json(registry);

        if let Document::Invalid(summary) = &document {
            found
                .notices
                .push(warning(format!("Installed plugins could not be read: {summary}")));
            return;
        }

        for (id, entries) in object(document.get("plugins")) {
            let Some(entry) = entries.as_array().and_then(|entries| entries.first()) else {
                continue;
            };

            let install = self.plugin(&id, entry);
            if install.info.enabled
                && let Some(root) = install.root
            {
                found.enabled_roots.push((install.info.name.clone(), root));
            }

            found.packages.push(install.package);
            found.plugins.push(install.info);
        }
    }

    fn plugin(&self, id: &str, entry: &Value) -> Install {
        let (id_name, marketplace) = match id.rsplit_once('@') {
            Some((name, marketplace)) if !name.is_empty() => (name, Some(marketplace.to_owned())),
            _ => (id, None),
        };

        let install_path = text_field(entry, "installPath").map(|raw| self.env.expand_home(&raw));
        let verified = install_path.as_deref().and_then(|dir| self.verify(dir));
        let package = verified
            .as_deref()
            .and_then(|dir| read_json(&dir.join("package.json")).value().cloned());
        let manifest = verified.as_deref().and_then(package_manifest);

        let package_name = package
            .as_ref()
            .and_then(|package| text_field(package, "name"))
            .unwrap_or_else(|| id_name.to_owned());
        let entry_enabled = entry.get("enabled") != Some(&Value::Bool(false));
        let shown_path = verified.clone().or(install_path);

        let info = PluginInfo {
            id: id.to_owned(),
            name: manifest
                .as_ref()
                .and_then(|manifest| text_field(manifest, "name"))
                .unwrap_or_else(|| id_name.to_owned()),
            version: text_field(entry, "version"),
            description: description(package.as_ref(), manifest.as_ref()),
            enabled: entry_enabled && self.state.is_enabled(&package_name),
            scope: self.scope,
            marketplace,
            features: features(manifest.as_ref(), self.state.selected_features(&package_name)),
            path: shown_path.map(|path| path.to_string_lossy().into_owned()),
            latest_version: None,
            update_available: false,
        };

        Install {
            info,
            root: verified,
            package: package_name,
        }
    }

    /// The canonical plugin directory when it lies inside an allowed root.
    fn verify(&self, dir: &Path) -> Option<PathBuf> {
        self.allowed_roots
            .iter()
            .find_map(|root| canonical_within(root, dir).ok())
            .filter(|dir| dir.is_dir())
    }
}

/// npm and linked plugins not already listed as marketplace installs.
fn npm_plugins(plugins_dir: &Path, state: &RuntimeState, found: &mut InstalledPlugins) {
    let manifest = read_json(&plugins_dir.join("package.json"));
    let mut names: Vec<String> = object(manifest.get("dependencies")).keys().cloned().collect();
    names.extend(state.lock.keys().cloned());

    for name in names {
        let listed = found.packages.contains(&name) || found.plugins.iter().any(|plugin| plugin.id == name);
        if listed || !PACKAGE_NAME.is_match(&name) {
            continue;
        }

        let dir = plugins_dir.join("node_modules").join(&name);
        let Some(package) = read_json(&dir.join("package.json")).value().cloned() else {
            continue;
        };
        let manifest = package_manifest(&dir);

        let plugin = PluginInfo {
            id: name.clone(),
            name: name.clone(),
            version: text_field(&package, "version"),
            description: description(Some(&package), manifest.as_ref()),
            enabled: state.is_enabled(&name),
            scope: ExtensionScope::User,
            marketplace: None,
            features: features(manifest.as_ref(), state.selected_features(&name)),
            path: Some(dir.to_string_lossy().into_owned()),
            latest_version: None,
            update_available: false,
        };

        if plugin.enabled {
            found.enabled_roots.push((name, dir));
        }
        found.plugins.push(plugin);
    }
}

fn description(package: Option<&Value>, manifest: Option<&Value>) -> Option<String> {
    manifest
        .and_then(|manifest| text_field(manifest, "description"))
        .or_else(|| package.and_then(|package| text_field(package, "description")))
}

/// Manifest features; enabled when selected, or by their `default` without a selection.
fn features(manifest: Option<&Value>, selected: Option<Vec<String>>) -> Vec<PluginFeature> {
    object(manifest.and_then(|manifest| manifest.get("features")))
        .into_iter()
        .map(|(name, definition)| {
            let enabled = match &selected {
                Some(names) => names.contains(&name),
                None => definition.get("default") == Some(&Value::Bool(true)),
            };

            PluginFeature { name, enabled }
        })
        .collect()
}

fn object(value: Option<&Value>) -> Map<String, Value> {
    value.and_then(Value::as_object).cloned().unwrap_or_default()
}

fn warning(message: String) -> Notice {
    Notice {
        level: NoticeLevel::Warning,
        message,
    }
}
