//! The body of a team hook: what may be stored in the team database, the body a local hook is
//! shared as, and the local file a team hook is copied to.
//!
//! A team body is a version 1 hook file with two more rules: no `seed` (every machine seeds its
//! own Harness family hooks), and a project hook's `project` is the repository key
//! (`git:host/owner/repo`), never a local path. Sharing rewrites only `project`, so every other
//! field (unknown ones included) travels as written.

use serde_json::{Map, Value};

use super::ProjectKey;
use crate::error::{BusinessError, Result};
use crate::hooks::format::{parse_hook_file, parse_team_file};
use crate::model::{Hook, HookScope};

/// The hook in the team body `bytes` of hook `id`, or why it breaks the team rules.
pub fn parse_team_hook(id: &str, bytes: &[u8]) -> std::result::Result<Hook, String> {
    let hook = parse_team_file(id, bytes)?;

    if hook.seed.is_some() {
        return Err("a Harness family hook is not shared".to_owned());
    }

    Ok(hook)
}

/// The body the local hook file `bytes` (hook `id`) is shared as: unchanged for a global hook, its
/// `project` replaced by `key` for a project hook. `Forbidden` for a Harness family hook and for a
/// project without an `origin` remote (a `path:` key), `InvalidInput` for a file that is not a
/// valid hook or a project hook without `key`.
pub fn share_body(id: &str, bytes: &[u8], key: Option<&ProjectKey>) -> Result<Vec<u8>> {
    let hook = parse_hook_file(id, bytes).map_err(BusinessError::InvalidInput)?;

    if hook.seed.is_some() {
        return Err(BusinessError::Forbidden(
            "Harness family hooks are not shared, every machine adds its own".to_owned(),
        ));
    }

    let body = match hook.scope {
        HookScope::Global => bytes.to_vec(),
        HookScope::Project => {
            let key =
                key.ok_or_else(|| BusinessError::InvalidInput("the project's repository is unknown".to_owned()))?;

            if !key.can_share() {
                return Err(BusinessError::Forbidden(
                    "the repository has no origin remote, so its hooks stay on this computer".to_owned(),
                ));
            }

            with_project(bytes, &key.key)?
        }
    };

    parse_team_hook(id, &body).map_err(BusinessError::InvalidInput)?;

    Ok(body)
}

/// The local file the team body `bytes` of hook `id` is copied to, as a hook and the object it was
/// read from (for its unknown fields): a project hook's `project` becomes `root`. `InvalidInput`
/// for a body that breaks the team rules, or a project hook without `root`.
pub(crate) fn local_copy(id: &str, bytes: &[u8], root: Option<&str>) -> Result<(Hook, Map<String, Value>)> {
    let team = parse_team_hook(id, bytes).map_err(BusinessError::InvalidInput)?;

    let local = match team.scope {
        HookScope::Global => bytes.to_vec(),
        HookScope::Project => {
            let root = root.ok_or_else(|| {
                BusinessError::InvalidInput("open the project to copy one of its team hooks".to_owned())
            })?;

            with_project(bytes, root)?
        }
    };

    let hook = parse_hook_file(id, &local).map_err(BusinessError::InvalidInput)?;

    Ok((hook, object(&local)))
}

/// `bytes` (a hook object) with `project` set to `project`, written as the apps write hook files.
fn with_project(bytes: &[u8], project: &str) -> Result<Vec<u8>> {
    let mut map = object(bytes);

    if map.is_empty() {
        return Err(BusinessError::InvalidInput("the hook is not a JSON object".to_owned()));
    }

    map.insert("project".to_owned(), Value::from(project));

    let mut out = serde_json::to_vec_pretty(&Value::Object(map))
        .map_err(|error| BusinessError::Internal(format!("encoding a hook: {error}")))?;
    out.push(b'\n');

    Ok(out)
}

fn object(bytes: &[u8]) -> Map<String, Value> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}
