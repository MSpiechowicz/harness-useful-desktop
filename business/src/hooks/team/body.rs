//! The body of a team hook: what may be stored in the team database, the body a local hook is
//! shared as, and the local file a team hook is copied to.
//!
//! A team body is a version 1 hook file with two more rules: no `seed` (every machine seeds its
//! own Harness family hooks), and a project hook's `project` is the repository key
//! (`git:host/owner/repo`), never a local path. Sharing rewrites only `project`, so every other
//! field (unknown ones included) travels as written.

use super::ProjectKey;
use crate::error::Result;
use crate::hooks::HookKind;
use crate::hooks::format::parse_team_file;
use crate::model::Hook;

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
    crate::signed_docs::team::share_body::<HookKind>(id, bytes, key)
}
