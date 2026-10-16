//! PI packages declared in settings (`packages`: a source string or `{ source, extensions?,
//! skills?, prompts?, themes? }`).
//!
//! The source is redacted at its key path (git URLs may carry credentials). Resource kinds are
//! known for packages whose directory exists (installed git and npm packages, local paths; see
//! [`super::sources`]), from their `pi` manifest or else conventional directories; an object
//! entry's empty filter (`"skills": []`) removes that kind.
//!
//! Settings and manifests may come from an untrusted project, so repeated entries count once and
//! every list read here is capped.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::layers::Layer;
use super::sources::{display_name, git_source, install_dir, package_kind};
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::{ConfigFormat, ExtensionScope, PackageInfo};
use crate::pi_family::{
    MAX_DECLARED_ENTRIES, PerLayer, Truncation, canonical_inside, distinct, distinct_within, parse_text, read_json,
};
use crate::security::{RedactionContext, Redactor, SafeReader};

/// Resource kinds a package can provide.
const RESOURCE_KINDS: [&str; 4] = ["extensions", "skills", "prompts", "themes"];

/// True when a package source in the user settings (`<agent dir>/settings.json`) contains
/// `needle` (e.g. a repository name). The sources are only compared, never returned.
pub(crate) fn user_has_package(paths: &HarnessPaths, needle: &str) -> bool {
    let settings = read_json(&paths.agent_dir.join("settings.json"));
    let Some(items) = settings.get("packages").and_then(Value::as_array) else {
        return false;
    };

    items.iter().any(|item| {
        let source = match item {
            Value::String(raw) => Some(raw.as_str()),
            Value::Object(fields) => fields.get("source").and_then(Value::as_str),
            _ => None,
        };

        source.is_some_and(|source| source.contains(needle))
    })
}

/// A package of the user settings as written, for "Share setup" ([`crate::sync`]). Nothing is
/// redacted: the caller decides what may leave the machine and never logs or shows these values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageRecord {
    /// The source string, unmasked (with any pin it has).
    pub source: String,
    /// `npm`, `git`, `local` or `unknown` (see [`package_kind`]).
    pub kind: &'static str,
    /// True for an object entry with any field besides `source` (resource filters such as
    /// `"skills": []`).
    pub filtered: bool,
}

/// The packages of the user settings (`<agent dir>/settings.json`), in file order: string entries
/// and the `source` of object entries; anything else is skipped.
pub(crate) fn user_package_records(paths: &HarnessPaths) -> Vec<PackageRecord> {
    let settings = read_json(&paths.agent_dir.join("settings.json"));
    let Some(items) = settings.get("packages").and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| match item {
            Value::String(raw) => Some((raw.as_str(), false)),
            Value::Object(fields) => {
                let filtered = fields.keys().any(|key| key != "source");

                fields
                    .get("source")
                    .and_then(Value::as_str)
                    .map(|source| (source, filtered))
            }
            _ => None,
        })
        .map(|(source, filtered)| PackageRecord {
            source: source.to_owned(),
            kind: package_kind(source),
            filtered,
        })
        .collect()
}

/// At most this many distinct package entries are read from one settings layer (people list a
/// handful), so a crafted settings file cannot stall the view.
const MAX_PACKAGES: usize = 128;

/// At most this many package resource paths of one kind are loaded per settings layer, over all
/// its packages (so project packages cannot crowd out the user's).
const MAX_LAYER_RESOURCE_PATHS: usize = 256;

/// Largest `package.json` read; real manifests are a few KiB.
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

/// The packages of every settings layer, resolved once per view (each manifest is read once).
#[derive(Debug, Clone)]
pub struct Packages {
    /// Every configured package, user layer first, one per directory (or source) in each layer.
    pub listed: Vec<PackageInfo>,
    /// The installed packages, project first (PI's precedence among packages), one per directory.
    pub installed: Vec<InstalledPackage>,
    /// The layers whose packages or package resources were cut short by a cap.
    pub truncation: Truncation,
}

impl Packages {
    /// `(package, path)` for every path the installed packages load `resource` from, in package
    /// order, at most [`MAX_LAYER_RESOURCE_PATHS`] for each settings layer.
    pub fn resource_paths(&self, resource: &str) -> Vec<(&InstalledPackage, &Path)> {
        let mut taken = PerLayer::new(0);

        self.all_resource_paths(resource)
            .filter(|(package, _)| {
                let count = taken.get(package.scope);
                *count += 1;
                *count <= MAX_LAYER_RESOURCE_PATHS
            })
            .collect()
    }

    fn all_resource_paths(&self, resource: &str) -> impl Iterator<Item = (&InstalledPackage, &Path)> {
        self.installed.iter().flat_map(move |package| {
            package
                .resource_paths(resource)
                .iter()
                .map(move |path| (package, path.as_path()))
        })
    }

    /// Records the layers with more than [`MAX_LAYER_RESOURCE_PATHS`] paths of some kind.
    fn note_capped_resources(&mut self) {
        for resource in RESOURCE_KINDS {
            let mut counts = PerLayer::new(0);
            for (package, _) in self.all_resource_paths(resource) {
                *counts.get(package.scope) += 1;
            }

            self.truncation
                .note(ExtensionScope::Project, counts.project > MAX_LAYER_RESOURCE_PATHS);
            self.truncation
                .note(ExtensionScope::User, counts.user > MAX_LAYER_RESOURCE_PATHS);
        }
    }
}

/// See [`Packages`]. A layer's repeated entries (the same source, or sources installed in the same
/// directory) count once, the first one kept.
pub fn resolve_packages(env: &Env, layers: &[Layer]) -> Packages {
    let mut listed_by_layer = Vec::new();
    let mut installed = Vec::new();
    let mut installed_dirs = HashSet::new();
    let mut manifests = Manifests::default();
    let mut truncation = Truncation::default();

    for layer in layers {
        let mut listed = Vec::new();
        let mut layer_dirs = HashSet::new();

        let (layer_entries, capped) = entries(layer);
        truncation.note(layer.scope, capped);

        for entry in layer_entries {
            let kind = package_kind(entry.raw);
            let dir = install_dir(env, layer, entry.raw, kind);

            let repeated = dir.as_ref().is_some_and(|dir| !layer_dirs.insert(dir.clone()));
            if repeated {
                continue;
            }

            let name = display_name(entry.raw, kind);
            let package =
                dir.map(|dir| InstalledPackage::new(name.clone(), layer.scope, dir, entry.filters, &mut manifests));
            listed.push(package_info(layer, &entry, kind, name, package.as_ref()));
            truncation.note(layer.scope, package.as_ref().is_some_and(|package| package.capped));

            if let Some(package) = package
                && installed_dirs.insert(package.dir.clone())
            {
                installed.push(package);
            }
        }

        listed_by_layer.push(listed);
    }

    let mut packages = Packages {
        listed: listed_by_layer.into_iter().rev().flatten().collect(),
        installed,
        truncation,
    };
    packages.note_capped_resources();

    packages
}

/// The `pi` manifests read so far, by canonical `package.json` path, so that many packages
/// linking one manifest read it once.
#[derive(Debug, Default)]
struct Manifests(HashMap<PathBuf, Option<Value>>);

impl Manifests {
    /// The `pi` object of `<dir>/package.json` (PI reads no other manifest key), at most
    /// [`MAX_MANIFEST_BYTES`] of it.
    fn pi_manifest(&mut self, dir: &Path) -> Option<Value> {
        let path = dunce::canonicalize(dir.join("package.json")).ok()?;

        self.0
            .entry(path)
            .or_insert_with_key(|path| {
                let text = SafeReader::read_to_string(path, MAX_MANIFEST_BYTES).ok()?;
                let package = parse_text(&text, ConfigFormat::Json).ok()?;

                package.get("pi").filter(|manifest| manifest.is_object()).cloned()
            })
            .clone()
    }
}

/// A configured package whose directory exists: installed by PI (git, npm) or a local path.
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    /// Display name ([`PackageInfo::name`]).
    pub name: String,
    /// Scope of the settings layer that configures it.
    pub scope: ExtensionScope,
    /// Canonical package directory.
    pub dir: PathBuf,
    /// [`Self::resource_paths`] of each of [`RESOURCE_KINDS`], resolved once.
    paths: [Vec<PathBuf>; 4],
    /// True when a manifest list held more than [`MAX_DECLARED_ENTRIES`] entries.
    capped: bool,
}

impl InstalledPackage {
    /// Reads the package's manifest and resolves its resource paths; an object entry's
    /// `filters` can remove kinds.
    fn new(
        name: String,
        scope: ExtensionScope,
        dir: PathBuf,
        filters: Option<&Map<String, Value>>,
        manifests: &mut Manifests,
    ) -> Self {
        let manifest = manifests.pi_manifest(&dir);
        let declared = RESOURCE_KINDS.map(|resource| declared_paths(&dir, manifest.as_ref(), filters, resource));
        let capped = declared.iter().any(|(_, capped)| *capped);

        Self {
            name,
            scope,
            dir,
            paths: declared.map(|(paths, _)| paths),
            capped,
        }
    }

    /// The canonical paths the package loads `resource` from, all inside [`Self::dir`]: the
    /// entries its `pi` manifest declares, or else the conventional `<dir>/<resource>` directory.
    /// Empty when an object entry's empty filter (`"skills": []`) removes the kind. Override
    /// patterns are not evaluated; a glob in the last segment stands for its parent directory.
    /// Repeated entries count once, and at most [`MAX_DECLARED_ENTRIES`] are resolved.
    pub fn resource_paths(&self, resource: &str) -> &[PathBuf] {
        RESOURCE_KINDS
            .iter()
            .position(|kind| *kind == resource)
            .map_or(&[], |index| self.paths[index].as_slice())
    }
}

/// See [`InstalledPackage::resource_paths`]; true when the manifest declares more than
/// [`MAX_DECLARED_ENTRIES`] distinct entries.
fn declared_paths(
    dir: &Path,
    manifest: Option<&Value>,
    filters: Option<&Map<String, Value>>,
    resource: &str,
) -> (Vec<PathBuf>, bool) {
    if filtered_out(filters, resource) {
        return (Vec::new(), false);
    }

    let (declared, capped): (Vec<String>, bool) = match manifest {
        Some(manifest) => {
            let entries = manifest
                .get(resource)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str);

            distinct_within(entries.filter_map(plain_entry), MAX_DECLARED_ENTRIES)
        }
        None => (vec![resource.to_owned()], false),
    };

    let resolved = declared
        .iter()
        .filter_map(|entry| canonical_inside(dir, Path::new(entry)));

    (distinct(resolved, MAX_DECLARED_ENTRIES), capped)
}

/// One `packages` entry: a source string or an object with `source`.
struct Entry<'a> {
    index: usize,
    raw: &'a str,
    filters: Option<&'a Map<String, Value>>,
}

/// The first [`MAX_PACKAGES`] entries of `layer` with distinct sources; true when more followed.
fn entries(layer: &Layer) -> (Vec<Entry<'_>>, bool) {
    let Some(items) = layer.settings.get("packages").and_then(Value::as_array) else {
        return (Vec::new(), false);
    };

    let mut sources = HashSet::new();

    let mut entries: Vec<Entry<'_>> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            // Trimmed once, so the kind, the name and the dedupe key all see the same source.
            let raw = match item {
                Value::String(raw) => raw.as_str(),
                Value::Object(fields) => fields.get("source")?.as_str()?,
                _ => return None,
            }
            .trim();

            Some(Entry {
                index,
                raw,
                filters: item.as_object(),
            })
        })
        .filter(|entry| sources.insert(entry.raw))
        .take(MAX_PACKAGES + 1)
        .collect();

    let capped = entries.len() > MAX_PACKAGES;
    entries.truncate(MAX_PACKAGES);

    (entries, capped)
}

fn package_info(
    layer: &Layer,
    entry: &Entry<'_>,
    kind: &str,
    name: String,
    installed: Option<&InstalledPackage>,
) -> PackageInfo {
    let segment = format!("[{}]", entry.index);
    let key_path = match entry.filters {
        Some(_) => vec!["packages", segment.as_str(), "source"],
        None => vec!["packages", segment.as_str()],
    };

    let resources = RESOURCE_KINDS
        .iter()
        .filter(|resource| installed.is_some_and(|package| !package.resource_paths(resource).is_empty()))
        .map(|resource| (*resource).to_owned())
        .collect();

    // A git source is a repository address, so an owner name like `MSpiechowicz` must not hide it
    // (it is what `pi remove` needs). Other sources keep the general rules.
    let source = if git_source(entry.raw).is_some() {
        Redactor.redact_package_source(entry.raw)
    } else {
        Redactor.redact_at(
            &Value::String(entry.raw.to_owned()),
            &mut RedactionContext::at(key_path.iter().copied()),
        )
    };

    PackageInfo {
        installed: installed.is_some(),
        source,
        kind: kind.to_owned(),
        name,
        scope: layer.scope,
        resources,
    }
}

/// A manifest entry as a path: override patterns (`!`, `+`, `-`) are skipped and a glob in the
/// last segment (`prompts/*.md`) is replaced by its directory; other globs are skipped.
fn plain_entry(entry: &str) -> Option<String> {
    let is_glob = |part: &str| part.contains(['*', '?', '[', '{']);

    if entry.starts_with(['!', '+', '-']) {
        return None;
    }

    if !is_glob(entry) {
        return Some(entry.to_owned());
    }

    let (parent, last) = entry.rsplit_once('/')?;
    (is_glob(last) && !is_glob(parent)).then(|| parent.to_owned())
}

/// True when an object entry sets `resource` to an empty list.
fn filtered_out(filters: Option<&Map<String, Value>>, resource: &str) -> bool {
    filters
        .and_then(|fields| fields.get(resource))
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}
