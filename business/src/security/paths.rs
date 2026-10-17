//! Path canonicalization, containment and argv charset checks.

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{BusinessError, Result};

/// Canonicalizes `candidate` (relative paths are taken relative to `root`) and requires it to lie
/// inside canonical `root`. Both must exist. `..` segments and symlinks are resolved first, so an
/// escape through either is refused with `Forbidden`.
pub fn canonical_within(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root = canonicalize(root)?;
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };

    let canonical = canonicalize(&joined)?;
    if !canonical.starts_with(&root) {
        return Err(BusinessError::Forbidden(format!(
            "{} is outside {}",
            candidate.display(),
            root.display()
        )));
    }

    Ok(canonical)
}

/// Validates a user-chosen project directory: after `~` expansion it must be absolute, exist, be a
/// directory and not be a filesystem root. Returns the canonical path.
pub fn validate_project_dir(env: &Env, path: &Path) -> Result<PathBuf> {
    let expanded = match path.to_str() {
        Some(raw) => env.expand_home(raw),
        None => path.to_path_buf(),
    };

    if !expanded.is_absolute() {
        return Err(BusinessError::InvalidInput(format!(
            "project path must be absolute: {}",
            path.display()
        )));
    }

    let canonical = canonicalize(&expanded)?;

    if !canonical.is_dir() {
        return Err(BusinessError::InvalidInput(format!(
            "project path is not a directory: {}",
            path.display()
        )));
    }

    if canonical.parent().is_none() {
        return Err(BusinessError::InvalidInput(
            "a filesystem root cannot be a project".to_owned(),
        ));
    }

    Ok(canonical)
}

/// True when `arg` is non-empty, does not start with `-` and uses only `A-Z a-z 0-9 @ . _ / : + = , -`.
/// Used for CLI targets (plugin names, package specs) and for any argument to a batch script.
pub fn is_safe_arg(arg: &str) -> bool {
    let allowed = |character: char| character.is_ascii_alphanumeric() || "@._/:+=,-".contains(character);

    !arg.is_empty() && !arg.starts_with('-') && arg.chars().all(allowed)
}

fn canonicalize(path: &Path) -> Result<PathBuf> {
    dunce::canonicalize(path).map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))
}

/// True when `source` names a user before its host: `scheme://user[:password]@host` or
/// `git:user@host` (except the conventional SSH user `git@`). npm specs (`npm:@scope/name@1.0`)
/// have no host part and are never flagged.
pub(crate) fn has_credentials(source: &str) -> bool {
    let authority = |rest: &str| rest.split('/').next().unwrap_or_default().to_owned();

    if let Some((_, rest)) = source.split_once("://") {
        return authority(rest).contains('@');
    }

    if let Some(rest) = source.strip_prefix("git:") {
        return match authority(rest).rsplit_once('@') {
            Some((user, _)) => user != "git",
            None => false,
        };
    }

    false
}
