//! Turning a [`HookDraft`] into the hook a writer saves: the writer becomes `updatedBy`, a new
//! hook gets it as `createdBy` too, and an edit keeps `createdAt`, `createdBy`, `seed` and
//! `requires` of the file it rewrites.

use std::path::Path;
use std::time::SystemTime;

use serde_json::{Map, Value};

use super::format::timestamp_key;
use super::project_identity;
use super::view::{MAX_HOOKS_PER_SCOPE, scope_key};
use crate::agent::rfc3339;
use crate::error::{BusinessError, Result};
use crate::model::{Hook, HookAuthor, HookDraft, HookRequirement, HookScope, HookSeed};

/// The hook `draft` describes, written by `author` at `now`. `base` is the JSON object of the file
/// it rewrites (empty for a new hook) and `previous` that file parsed, when it is valid.
pub(crate) fn draft_hook(
    draft: &HookDraft,
    id: &str,
    author: HookAuthor,
    base: &Map<String, Value>,
    previous: Option<&Hook>,
    now: SystemTime,
) -> Result<Hook> {
    let now = rfc3339(now);
    let kept = Kept::from_file(base, previous);

    let project = match draft.scope {
        HookScope::Global => None,
        HookScope::Project => Some(draft_project(draft, previous)?),
    };

    let description = draft
        .description
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned);
    let matcher = draft
        .matcher
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);

    Ok(Hook {
        id: id.to_owned(),
        name: draft.name.clone(),
        description,
        scope: draft.scope,
        project,
        event: draft.event,
        matcher,
        action: draft.action.clone(),
        enabled: draft.enabled,
        requires: kept.requires,
        seed: kept.seed,
        created_at: kept.created_at.unwrap_or_else(|| now.clone()),
        created_by: kept.created_by.unwrap_or(author),
        updated_at: now,
        updated_by: author,
    })
}

/// Refuses `hook` when its scope already holds [`MAX_HOOKS_PER_SCOPE`] other valid hooks
/// (`others`: every valid hook in the folder).
pub(crate) fn check_room<'a>(others: impl Iterator<Item = &'a Hook>, hook: &Hook) -> Result<()> {
    let count = others
        .filter(|other| other.id != hook.id && scope_key(other) == scope_key(hook))
        .count();

    if count >= MAX_HOOKS_PER_SCOPE {
        return Err(BusinessError::InvalidInput(format!(
            "a scope holds at most {MAX_HOOKS_PER_SCOPE} hooks"
        )));
    }

    Ok(())
}

/// The project root a project draft stores: the previous one when the draft keeps it, else the
/// project identity of the folder it names (which must exist and be a folder).
fn draft_project(draft: &HookDraft, previous: Option<&Hook>) -> Result<String> {
    let Some(folder) = draft.project.as_deref().filter(|folder| !folder.is_empty()) else {
        return Err(BusinessError::InvalidInput("a project hook needs a project".to_owned()));
    };

    if previous.and_then(|hook| hook.project.as_deref()) == Some(folder) {
        return Ok(folder.to_owned());
    }

    let root = project_identity(Path::new(folder))?;

    root.into_os_string()
        .into_string()
        .map_err(|_| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))
}

/// The fields an edit keeps from the file it rewrites (each one only when it is valid).
struct Kept {
    created_at: Option<String>,
    created_by: Option<HookAuthor>,
    seed: Option<HookSeed>,
    requires: Vec<HookRequirement>,
}

impl Kept {
    fn from_file(base: &Map<String, Value>, previous: Option<&Hook>) -> Self {
        if let Some(previous) = previous {
            return Self {
                created_at: Some(previous.created_at.clone()),
                created_by: Some(previous.created_by),
                seed: previous.seed,
                requires: previous.requires.clone(),
            };
        }

        // An invalid file being fixed: keep what can still be read.
        let text = |key: &str| base.get(key).and_then(Value::as_str);

        let mut requires: Vec<HookRequirement> = Vec::new();

        for requirement in base
            .get("requires")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str().and_then(HookRequirement::parse))
        {
            if !requires.contains(&requirement) {
                requires.push(requirement);
            }
        }

        Self {
            created_at: text("createdAt")
                .filter(|time| timestamp_key(time).is_some())
                .map(str::to_owned),
            created_by: match text("createdBy") {
                Some("agent") => Some(HookAuthor::Agent),
                Some("desktop") => Some(HookAuthor::Desktop),
                _ => None,
            },
            seed: text("seed").and_then(HookSeed::parse),
            requires,
        }
    }
}
