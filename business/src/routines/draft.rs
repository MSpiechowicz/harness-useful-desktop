//! Turning a [`RoutineDraft`] into the routine a writer saves: the writer becomes `updatedBy`, a
//! new routine gets it as `createdBy` too, and an edit keeps `createdAt` and `createdBy` of the
//! file it rewrites (its unknown fields are kept by the writer).

use std::time::SystemTime;

use serde_json::{Map, Value};

use super::RoutineKind;
use crate::agent::rfc3339;
use crate::error::Result;
use crate::model::{HookAuthor, HookScope, Routine, RoutineDraft};
use crate::signed_docs::scope::{draft_project, kept_origin};

/// The routine `draft` describes, written by `author` at `now`. `base` is the JSON object of the
/// file it rewrites (empty for a new routine) and `previous` that file parsed, when it is valid.
/// The values are checked when the bytes are read back before the write.
pub(crate) fn draft_routine(
    draft: &RoutineDraft,
    id: &str,
    author: HookAuthor,
    base: &Map<String, Value>,
    previous: Option<&Routine>,
    now: SystemTime,
) -> Result<Routine> {
    let now = rfc3339(now);
    let (created_at, created_by) = kept_origin(base, previous);

    let project = match draft.scope {
        HookScope::Global => None,
        HookScope::Project => {
            let stored = previous.and_then(|routine| routine.project.as_deref());
            Some(draft_project::<RoutineKind>(draft.project.as_deref(), stored)?)
        }
    };

    let description = draft.description.clone().filter(|text| !text.trim().is_empty());

    Ok(Routine {
        id: id.to_owned(),
        name: draft.name.clone(),
        description,
        scope: draft.scope,
        project,
        when: draft.when.clone(),
        action: draft.action.clone(),
        enabled: draft.enabled,
        created_at: created_at.unwrap_or_else(|| now.clone()),
        created_by: created_by.unwrap_or(author),
        updated_at: now,
        updated_by: author,
    })
}
