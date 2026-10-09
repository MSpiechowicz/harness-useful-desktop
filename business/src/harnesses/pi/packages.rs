//! PI packages declared in settings (`packages`: a source string or `{ source, extensions?,
//! skills?, prompts?, themes? }`).
//!
//! The source is redacted at its key path (git URLs may carry credentials). Resource kinds are
//! known only for local packages, from their `pi` manifest or conventional directories; an
//! object entry's empty filter (`"skills": []`) removes that kind.

use std::path::Path;

use serde_json::Value;

use super::layers::{Layer, resolve};
use crate::env::Env;
use crate::model::PackageInfo;
use crate::pi_family::package_manifest;
use crate::security::{RedactionContext, Redactor};

/// Resource kinds a package can provide.
const RESOURCE_KINDS: [&str; 4] = ["extensions", "skills", "prompts", "themes"];

/// Packages from the user layer, then the project layer.
pub fn packages(env: &Env, layers: &[Layer]) -> Vec<PackageInfo> {
    layers
        .iter()
        .rev()
        .flat_map(|layer| layer_packages(env, layer))
        .collect()
}

fn layer_packages(env: &Env, layer: &Layer) -> Vec<PackageInfo> {
    let Some(items) = layer.settings.get("packages").and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| package(env, layer, index, item))
        .collect()
}

fn package(env: &Env, layer: &Layer, index: usize, item: &Value) -> Option<PackageInfo> {
    let segment = format!("[{index}]");
    let (raw, key_path) = match item {
        Value::String(raw) => (raw.as_str(), vec!["packages", segment.as_str()]),
        Value::Object(fields) => (
            fields.get("source")?.as_str()?,
            vec!["packages", segment.as_str(), "source"],
        ),
        _ => return None,
    };

    let kind = package_kind(raw);
    let local_dir = (kind == "local").then(|| resolve(env, layer, raw));
    let filters = item.as_object();

    let resources = RESOURCE_KINDS
        .iter()
        .filter(|resource| local_dir.as_deref().is_some_and(|dir| provides(dir, resource)))
        .filter(|resource| !filtered_out(filters, resource))
        .map(|resource| (*resource).to_owned())
        .collect();

    let source = Redactor.redact_at(
        &Value::String(raw.to_owned()),
        &mut RedactionContext::at(key_path.iter().copied()),
    );

    Some(PackageInfo {
        source,
        kind: kind.to_owned(),
        scope: layer.scope,
        resources,
    })
}

/// `npm`, `git`, `local` or `unknown`, from the source prefix.
fn package_kind(raw: &str) -> &'static str {
    let is_git = ["git:", "https://", "http://", "ssh://", "git@"]
        .iter()
        .any(|prefix| raw.starts_with(prefix));
    let is_local = ["./", "../", "/", "~", ".\\", "..\\"]
        .iter()
        .any(|prefix| raw.starts_with(prefix))
        || raw.as_bytes().get(1) == Some(&b':');

    if raw.starts_with("npm:") {
        "npm"
    } else if is_git {
        "git"
    } else if is_local {
        "local"
    } else {
        "unknown"
    }
}

/// True when the package's `pi` manifest declares `resource` or the conventional directory exists.
fn provides(dir: &Path, resource: &str) -> bool {
    let declared = package_manifest(dir).is_some_and(|manifest| manifest.get(resource).is_some());

    declared || dir.join(resource).is_dir()
}

/// True when an object entry sets `resource` to an empty list.
fn filtered_out(filters: Option<&serde_json::Map<String, Value>>, resource: &str) -> bool {
    filters
        .and_then(|fields| fields.get(resource))
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}
