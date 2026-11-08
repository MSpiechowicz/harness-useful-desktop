//! Creating and editing a note from the Notes view (`memory_note_save`), through the pooled
//! `harness-memory mcp` connection of [`MemoryScripts`] (see
//! [`MemoryScripts::call_tool`]), passing the project root as `project_path` when there is one.
//!
//! - Without a note to replace, `note_save {title, body, kind, tags, entities, scope}`. Its answer is text
//!   only: `created|updated note <id> (<scope>, revision <n>)`, after a frame line and followed
//!   by similar notes when there are any, so the id is read from the first line of that shape
//!   (`None` when there is none). A same-title note of the repository is refused because the app
//!   names the repository with `project_path`: `note <id> already has this title and …` (at the
//!   start of the answer), which is [`NoteSaveOutcome::Exists`] with that id.
//! - The server rewrites a same-title global note instead (`updated`), so a new global note is
//!   looked up first (see [`memory_notes_titles`](super::memory_notes_titles)): one titled as the
//!   server compares titles is `exists` with its id, and nothing is saved.
//! - With one (an edit, or replacing the note that had the title), `note_update {id, title, body,
//!   kind, tags}`: the note keeps its scope and its entities (`note_update` takes none). It answers the same line (`updated`), `note <other>
//!   already has this title: …` when another note of its scope has the new title (`exists`), and
//!   `note <id> not found …` when the project cannot see it (`notFound`).
//!
//! A server with `save-guards` ([`SaveRoute::Cli`]) takes the same note through `notes save` or
//! `notes update` instead (see [`memory_user_save`](super::memory_user_save)), which no save guard
//! stops.
//!
//! An edit names the revision the editor opened (`expected_revision`). A server that also lists
//! `note-expect-revision` ([`SaveRoute::CliExpectRevision`]) saves the note only at that revision:
//! a note that changed since, or is gone, is refused as a conflict and nothing is saved. A server
//! without it, and the MCP route, save as before (the revision is not sent).
//!
//! A new note may name where it goes (`place`, `remote` or `local`) while its scope is routed to
//! "Remote + this computer": a server that lists `save-place` ([`SaveRoute::places`]) takes
//! `notes save --place`, and answers where the note went ([`NoteSaved::place`]). A `place` for an
//! edit is `invalidInput`, and one the server cannot take (no `save-place`, or the MCP route) is
//! `unsupported`: nothing runs. A team database that cannot be reached fails the save with the
//! server's (redacted) text and nothing is saved on either side.
//!
//! The draft is checked against the server's limits first, and a new repository note needs a
//! project (`invalidInput`). Neither the title nor the body is logged.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value, json};

use super::mcp_client::ToolOutput;
use super::memory_hidden::entry_server;
use super::memory_notes_titles::{ImportTitles, global_titled};
use super::memory_scripts::{MemoryScripts, tool_error_text, with_project};
use super::memory_user_save::{SaveRoute, save_note_cli};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryCopyKind, MemorySavePlace, NoteDraft, NoteSaveOutcome, NoteSaved, ScriptScope};
use crate::process::ProcessRunner;

/// Harness Memory's `TITLE_MAX`, in characters.
pub const TITLE_MAX: usize = 120;
/// Its `BODY_MAX`, in characters.
pub const BODY_MAX: usize = 4000;
/// Its `TAGS_MAX`.
pub const TAGS_MAX: usize = 8;
/// Its `TAG_MAX`, in characters.
pub const TAG_MAX: usize = 32;
/// Its `ENTITIES_PER_NOTE`.
pub const ENTITIES_MAX: usize = 10;
/// Its `ENTITY_MAX`, in characters.
pub const ENTITY_MAX: usize = 80;

const NOTE_SAVE_TOOL: &str = "note_save";
const NOTE_UPDATE_TOOL: &str = "note_update";

/// The line of a saved note, on any line of the answer.
static SAVED_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(created|updated) note (\d+) \(").expect("static regex is valid"));

/// The line of a change to a team note that waits for the team database (`note_update` while it
/// cannot be reached).
static QUEUED_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^note (\d+) queued: ").expect("static regex is valid"));

/// The refusal of a title another note has (`note_save` and `note_update`), at the start of the
/// answer.
static TAKEN_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\s*note (\d+) already has this title").expect("static regex is valid"));

/// Saves `draft` as a new note, or over note `replace_id` (see the module docs). `notFound` when
/// that note is gone or the server is not installed, `unsupported` when the server predates
/// saving from the app, `invalidInput` for a draft over a limit or a repository note without a
/// project, `process` with the server's (redacted) text when it refuses otherwise. `runner` runs
/// the server's commands that read the global notes, and the save itself on `route`
/// [`SaveRoute::Cli`]. An edit (`replace_id`) with `expected_revision`, the revision the editor
/// opened, is a `conflict` (nothing saved) when the note changed since or is gone, on a route
/// that can check it ([`SaveRoute::expects_revision`]); `invalidInput` for a negative revision.
/// `place` names where a new note goes (see the module docs).
#[expect(
    clippy::too_many_arguments,
    reason = "one argument per part of the request, as the IPC command takes them"
)]
pub async fn save_note(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    route: SaveRoute,
    project: Option<&Path>,
    draft: &NoteDraft,
    replace_id: Option<i64>,
    expected_revision: Option<i64>,
    place: Option<MemorySavePlace>,
) -> Result<NoteSaved> {
    save_note_in(
        env,
        probe,
        runner,
        route,
        project,
        draft,
        replace_id,
        expected_revision,
        place,
        None,
    )
    .await
}

/// [`save_note`], within an import whose global titles `titles` keeps (a new global note is
/// remembered there).
#[expect(
    clippy::too_many_arguments,
    reason = "the import passes its title cache on top of what a single save takes"
)]
pub(crate) async fn save_note_in(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    route: SaveRoute,
    project: Option<&Path>,
    draft: &NoteDraft,
    replace_id: Option<i64>,
    expected_revision: Option<i64>,
    place: Option<MemorySavePlace>,
    mut titles: Option<&mut ImportTitles>,
) -> Result<NoteSaved> {
    let fields = checked(draft)?;
    let place = route.place_for(place, replace_id.is_some())?;

    if expected_revision.is_some_and(|revision| revision < 0) {
        return Err(invalid("a note revision is not negative".to_owned()));
    }

    let (tool, arguments) = match replace_id {
        Some(id) => {
            let mut arguments = fields;
            arguments.insert("id".to_owned(), json!(id));
            (NOTE_UPDATE_TOOL, arguments)
        }
        None => {
            if draft.scope == ScriptScope::Repo && project.is_none() {
                return Err(invalid("a repository note needs a project".to_owned()));
            }

            if draft.scope == ScriptScope::Global
                && let Some(id) = global_titled(env, probe, runner, draft.title.trim(), titles.as_deref_mut()).await?
            {
                return Ok(NoteSaved {
                    outcome: NoteSaveOutcome::Exists,
                    id: Some(id),
                    queued: false,
                    place: None,
                });
            }

            let mut arguments = fields;
            arguments.insert("entities".to_owned(), json!(entities_of(draft)?));
            arguments.insert("scope".to_owned(), json!(draft.scope));
            (NOTE_SAVE_TOOL, arguments)
        }
    };

    let saved = if route.through_cli() {
        // Only an edit on a server that checks revisions names the one the editor opened.
        let expected = expected_revision.filter(|_| route.expects_revision() && replace_id.is_some());

        let server = entry_server(env, probe, MemoryCopyKind::Notes).await?;
        save_note_cli(env, runner, &server, project, arguments, replace_id, expected, place).await?
    } else {
        let output = probe.call_tool(env, tool, with_project(arguments, project)).await?;
        answer(tool, &output, replace_id)?
    };

    tracing::info!(tool, id = saved.id, outcome = ?saved.outcome, "memory note saved");

    if draft.scope == ScriptScope::Global
        && replace_id.is_none()
        && let (Some(titles), Some(id)) = (titles, saved.id)
    {
        titles.saved(draft.title.trim(), id);
    }

    Ok(saved)
}

/// The fields of `draft` as the server takes them (title trimmed, tags lowercased without
/// duplicates), or `invalidInput` for what the server would refuse.
fn checked(draft: &NoteDraft) -> Result<Map<String, Value>> {
    let title = draft.title.trim();

    if title.is_empty() || title.chars().count() > TITLE_MAX || title.contains(['\n', '\r']) {
        return Err(invalid(format!(
            "a note title is one line of 1 to {TITLE_MAX} characters"
        )));
    }

    if draft.body.trim().is_empty() || draft.body.trim().chars().count() > BODY_MAX {
        return Err(invalid(format!("a note body is 1 to {BODY_MAX} characters")));
    }

    if draft.tags.len() > TAGS_MAX {
        return Err(invalid(format!("a note has at most {TAGS_MAX} tags")));
    }

    let mut tags: Vec<String> = Vec::new();

    for tag in &draft.tags {
        let tag = tag.trim().to_lowercase();
        let allowed = tag
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || "_-".contains(character));

        if tag.is_empty() || tag.chars().count() > TAG_MAX || !allowed {
            return Err(invalid(format!(
                "each tag is 1 to {TAG_MAX} characters of a-z, 0-9, _ and -"
            )));
        }

        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }

    let mut fields = Map::new();
    fields.insert("title".to_owned(), json!(title));
    fields.insert("body".to_owned(), json!(draft.body));
    fields.insert("kind".to_owned(), json!(draft.kind));
    fields.insert("tags".to_owned(), json!(tags));

    Ok(fields)
}

/// The entities of `draft` as the server takes them (each trimmed, duplicates dropped as the
/// server compares names), or `invalidInput` for what the server would refuse.
fn entities_of(draft: &NoteDraft) -> Result<Vec<String>> {
    if draft.entities.len() > ENTITIES_MAX {
        return Err(invalid(format!("a note has at most {ENTITIES_MAX} entities")));
    }

    let mut seen: Vec<String> = Vec::new();
    let mut entities: Vec<String> = Vec::new();

    for entity in &draft.entities {
        let entity = entity.trim();

        if entity.is_empty() || entity.chars().count() > ENTITY_MAX || entity.contains(['\n', '\r']) {
            return Err(invalid(format!(
                "each entity is one line of 1 to {ENTITY_MAX} characters"
            )));
        }

        let key = entity.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();

        if !seen.contains(&key) {
            seen.push(key);
            entities.push(entity.to_owned());
        }
    }

    Ok(entities)
}

fn invalid(message: String) -> BusinessError {
    BusinessError::InvalidInput(message)
}

/// What the server's answer to `tool` says (see the module docs).
fn answer(tool: &str, output: &ToolOutput, replace_id: Option<i64>) -> Result<NoteSaved> {
    let text = output.text.as_deref().unwrap_or_default();

    if !output.is_error {
        let fallback = match replace_id {
            Some(_) => NoteSaveOutcome::Updated,
            None => NoteSaveOutcome::Created,
        };

        return Ok(saved_from(text, fallback));
    }

    if let Some(id) = taken_title(text) {
        return Ok(NoteSaved {
            outcome: NoteSaveOutcome::Exists,
            id: Some(id),
            queued: false,
            place: None,
        });
    }

    if let Some(id) = replace_id
        && text.trim_start().starts_with(&format!("note {id} not found"))
    {
        return Err(BusinessError::NotFound(format!("note {id}")));
    }

    Err(BusinessError::Process(format!("{tool}: {}", tool_error_text(output))))
}

/// The outcome and id on the first `created|updated note <id> (` line of `text`, or the id and
/// `queued` on a `note <id> queued:` line (a change to a team note that waits for the team
/// database); `fallback` with no id when there is none.
fn saved_from(text: &str, fallback: NoteSaveOutcome) -> NoteSaved {
    let Some(found) = SAVED_LINE.captures(text) else {
        if let Some(queued) = QUEUED_LINE.captures(text) {
            return NoteSaved {
                outcome: NoteSaveOutcome::Updated,
                id: queued[1].parse().ok(),
                queued: true,
                place: None,
            };
        }

        return NoteSaved {
            outcome: fallback,
            id: None,
            queued: false,
            place: None,
        };
    };

    let outcome = match &found[1] {
        "created" => NoteSaveOutcome::Created,
        _ => NoteSaveOutcome::Updated,
    };

    NoteSaved {
        outcome,
        id: found[2].parse().ok(),
        queued: false,
        place: None,
    }
}

/// The id of the note that already has the title, in a refusal.
pub(super) fn taken_title(text: &str) -> Option<i64> {
    TAKEN_TITLE.captures(text).and_then(|found| found[1].parse().ok())
}

#[cfg(test)]
mod tests;
