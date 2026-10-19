//! Turning a [`HookDraft`] into the hook a writer saves: the writer becomes `updatedBy`, a new
//! hook gets it as `createdBy` too, and an edit keeps `createdAt`, `createdBy`, `seed` and
//! `requires` of the file it rewrites.

use std::time::SystemTime;

use serde_json::{Map, Value};

use super::HookKind;
use crate::agent::rfc3339;
use crate::error::Result;
use crate::model::{Hook, HookAuthor, HookDraft, HookRequirement, HookScope, HookSeed};
use crate::signed_docs::scope::{draft_project, kept_origin};

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
        HookScope::Project => Some(draft_project::<HookKind>(
            draft.project.as_deref(),
            previous.and_then(|hook| hook.project.as_deref()),
        )?),
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

/// The fields an edit keeps from the file it rewrites (each one only when it is valid).
struct Kept {
    created_at: Option<String>,
    created_by: Option<HookAuthor>,
    seed: Option<HookSeed>,
    requires: Vec<HookRequirement>,
}

impl Kept {
    fn from_file(base: &Map<String, Value>, previous: Option<&Hook>) -> Self {
        let (created_at, created_by) = kept_origin(base, previous);

        if let Some(previous) = previous {
            return Self {
                created_at,
                created_by,
                seed: previous.seed,
                requires: previous.requires.clone(),
            };
        }

        // An invalid file being fixed: keep what can still be read.
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
            created_at,
            created_by,
            seed: base.get("seed").and_then(Value::as_str).and_then(HookSeed::parse),
            requires,
        }
    }
}
