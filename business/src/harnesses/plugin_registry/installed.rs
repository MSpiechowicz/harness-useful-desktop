//! `installed_plugins.json`, version 2 (see the [module docs](super)).
//!
//! `installPath` comes from a file, so adapters read a plugin directory only after
//! [`verified_dir`] placed it inside one of their plugin roots.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::env::Env;
use crate::pi_family::{Document, read_json, text_field};
use crate::security::canonical_within;

/// One plugin of the registry with its install records.
#[derive(Debug, Clone, PartialEq)]
pub struct Registered {
    /// The registry key, `name@marketplace`.
    pub id: String,
    /// Its install records in file order; empty when the value is not an array.
    pub installs: Vec<Value>,
}

/// The plugins of the registry at `path` in key order. A missing file, or one without a `plugins`
/// object, has none. The error is a secret-free summary of why the file could not be read.
pub fn read_registry(path: &Path) -> Result<Vec<Registered>, String> {
    let document = read_json(path);

    if let Document::Invalid(summary) = &document {
        return Err(summary.clone());
    }

    let plugins = document.get("plugins").and_then(Value::as_object);
    let registered = plugins
        .into_iter()
        .flatten()
        .map(|(id, installs)| Registered {
            id: id.clone(),
            installs: installs.as_array().cloned().unwrap_or_default(),
        })
        .collect();

    Ok(registered)
}

/// The plugin name and marketplace of a registry id (`name@marketplace`). An id without `@`, or
/// with nothing before the last one, is all name.
pub fn split_id(id: &str) -> (&str, Option<&str>) {
    match id.rsplit_once('@') {
        Some((name, marketplace)) if !name.is_empty() => (name, Some(marketplace)),
        _ => (id, None),
    }
}

/// An install record's `installPath`, with `~` expanded.
pub fn install_path(env: &Env, install: &Value) -> Option<PathBuf> {
    text_field(install, "installPath").map(|raw| env.expand_home(&raw))
}

/// The canonical plugin directory when `dir` is a directory inside one of `allowed_roots`.
pub fn verified_dir(allowed_roots: &[PathBuf], dir: &Path) -> Option<PathBuf> {
    allowed_roots
        .iter()
        .find_map(|root| canonical_within(root, dir).ok())
        .filter(|dir| dir.is_dir())
}

#[cfg(test)]
mod tests {
    use super::split_id;

    #[test]
    fn ids_split_at_the_last_at_sign() {
        assert_eq!(split_id("demo@market"), ("demo", Some("market")));
        assert_eq!(split_id("@scope/demo@market"), ("@scope/demo", Some("market")));
        assert_eq!(split_id("@market"), ("@market", None));
        assert_eq!(split_id("loose"), ("loose", None));
    }
}
