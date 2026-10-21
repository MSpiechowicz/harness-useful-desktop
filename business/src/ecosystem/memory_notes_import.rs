//! Importing notes from a CSV or JSON file into Harness Memory (`memory_notes_import_preview`,
//! then `memory_notes_import_apply`).
//!
//! [`read_notes_file`] reads at most 500 notes of a file of at most 2 MiB (see [`parse`]), checks
//! each against the limits [`save_note`] checks, and marks later rows with a title an earlier row
//! has (any case and spacing). It answers the review the webview shows and the rows Rust keeps
//! ([`StoredNotesImport`]), so the webview only sends back the indexes it keeps.
//!
//! [`apply_notes_import`] saves those rows in the file's order with [`save_note`], each with its
//! own scope or the dialog's default. A row whose title a note of its scope already has is
//! skipped (`exists`), or with `replace` that note is rewritten (`replaced`). `updated` counts
//! the notes the server rewrote by itself. The global notes are listed at most once per import
//! (see [`ImportTitles`]), when a row's title is one the search cannot tell. Neither titles nor bodies are logged.
//!
//! Field values: `kind` is one of the note kinds in any case (`optimization` too), `note` when
//! empty. `scope` is `repo` (or `repository`, `project`) or `global`, the default when empty.
//! `tags` are split on commas, semicolons, pipes and whitespace and lowercased; a tag with any
//! other character than `a-z`, `0-9`, `_` and `-` is a problem (`invalidChars`), never cut into
//! pieces. At most [`TAGS_MAX`] tags of a row are kept: one more makes it `tooMany`, and the rest
//! are not read.

mod parse;
mod template;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub use parse::{FileRefusal, MAX_IMPORT_BYTES, MAX_IMPORT_ROWS};
pub use template::import_template;

use super::memory_notes_save::{BODY_MAX, TAG_MAX, TAGS_MAX, TITLE_MAX, save_note, save_note_in};
use super::memory_notes_titles::{ImportTitles, title_key};
use super::memory_scripts::MemoryScripts;
use super::memory_user_save::SaveRoute;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    NoteDraft, NoteKind, NoteSaveOutcome, NotesImportExisting, NotesImportFailure, NotesImportField, NotesImportPlan,
    NotesImportProblem, NotesImportProblemCode, NotesImportProgress, NotesImportResult, NotesImportRow, ScriptScope,
};
use crate::process::ProcessRunner;
use parse::{RawRow, read_file};

/// How much of a title and of a body the review shows, in characters.
const PREVIEW_CHARS: usize = 200;

/// The rows of a reviewed file, kept in Rust until they are imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredNotesImport {
    id: String,
    project: Option<PathBuf>,
    rows: Vec<StoredRow>,
}

impl StoredNotesImport {
    /// The plan id the webview passes back (`NotesImportPlan::plan_id`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The project root the file was reviewed for.
    pub fn project(&self) -> Option<&Path> {
        self.project.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredRow {
    index: u32,
    title: String,
    body: String,
    kind: NoteKind,
    tags: Vec<String>,
    scope: Option<ScriptScope>,
    importable: bool,
}

impl StoredRow {
    fn draft(&self, default_scope: ScriptScope) -> NoteDraft {
        NoteDraft {
            title: self.title.clone(),
            body: self.body.clone(),
            kind: self.kind,
            tags: self.tags.clone(),
            entities: Vec::new(),
            scope: self.scope.unwrap_or(default_scope),
        }
    }
}

/// The review of a file and the rows kept for its import.
#[derive(Debug)]
pub struct NotesImportRead {
    pub plan: NotesImportPlan,
    pub stored: StoredNotesImport,
}

/// Reads the notes of the file `file_name` (its name only) for the project root `project`, under
/// a fresh uuid plan id. Refused for a file that cannot be read as notes (see
/// [`parse::read_file`]); a row's own problems are in its `problems`.
pub fn read_notes_file(
    file_name: &str,
    bytes: &[u8],
    project: Option<&Path>,
) -> std::result::Result<NotesImportRead, FileRefusal> {
    let file = read_file(file_name, bytes).inspect_err(|refusal| {
        tracing::info!(reason = ?refusal.reason, "notes import file refused");
    })?;
    let id = uuid::Uuid::new_v4().to_string();

    let mut first_of_title: HashMap<String, u32> = HashMap::new();
    let mut rows = Vec::with_capacity(file.rows.len());
    let mut stored = Vec::with_capacity(file.rows.len());

    for (index, raw) in (0u32..).zip(file.rows) {
        let mut checked = checked_row(index, raw, project.is_some());
        let key = title_key(&checked.stored.title);

        if !key.is_empty() {
            let first = *first_of_title.entry(key).or_insert(index);
            checked.row.duplicate_of = (first != index).then_some(first);
        }

        rows.push(checked.row);
        stored.push(checked.stored);
    }

    let plan = NotesImportPlan {
        plan_id: id.clone(),
        file_name: file_name.to_owned(),
        format: file.format,
        truncated: file.truncated,
        rows,
    };

    tracing::info!(
        rows = plan.rows.len(),
        truncated = plan.truncated,
        format = ?plan.format,
        "notes import reviewed"
    );

    Ok(NotesImportRead {
        plan,
        stored: StoredNotesImport {
            id,
            project: project.map(Path::to_path_buf),
            rows: stored,
        },
    })
}

/// What the webview asks to import from a reviewed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesImportRequest {
    /// The indexes of the rows to import (any order; duplicates count once).
    pub rows: Vec<u32>,
    /// The scope of the rows whose file gives none.
    pub default_scope: ScriptScope,
    /// Rewrite a repository note that has a row's title instead of skipping the row.
    pub replace: bool,
}

/// Saves the requested rows of `stored` in the file's order, through `probe` or the user's save
/// commands (`route`; `runner` runs the server's commands), for the project it was reviewed for, calling `on_progress` after each. `invalidInput` (nothing saved) for an
/// empty selection or one with an index the file does not have or a row with problems. A server
/// that is missing or predates saving (`notFound`, `unsupported`) ends the import with that
/// error; the rows saved before it stay saved. Any other refusal fails that row only.
pub async fn apply_notes_import(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    route: SaveRoute,
    stored: &StoredNotesImport,
    request: &NotesImportRequest,
    mut on_progress: impl FnMut(NotesImportProgress),
) -> Result<NotesImportResult> {
    let selected = selected_rows(stored, &request.rows)?;
    let total = u32::try_from(selected.len()).unwrap_or(u32::MAX);
    let project = stored.project();
    let mut result = NotesImportResult::default();
    let mut titles = ImportTitles::default();

    for (done, row) in (1u32..).zip(selected) {
        let draft = row.draft(request.default_scope);

        match save_note_in(env, probe, runner, route, project, &draft, None, Some(&mut titles)).await {
            Ok(saved) => match saved.outcome {
                NoteSaveOutcome::Created => result.created += 1,
                NoteSaveOutcome::Updated => result.updated += 1,
                NoteSaveOutcome::Exists => match saved.id {
                    Some(id) if request.replace => {
                        let replaced = save_note(env, probe, runner, route, project, &draft, Some(id)).await;
                        record_replace(&mut result, row.index, replaced);
                    }
                    id => result.exists.push(NotesImportExisting { index: row.index, id }),
                },
            },
            Err(error @ (BusinessError::NotFound(_) | BusinessError::Unsupported(_))) => return Err(error),
            Err(error) => result.failed.push(failure(row.index, &error)),
        }

        on_progress(NotesImportProgress {
            index: row.index,
            done,
            total,
        });
    }

    tracing::info!(
        created = result.created,
        updated = result.updated,
        replaced = result.replaced,
        exists = result.exists.len(),
        failed = result.failed.len(),
        "notes imported"
    );

    Ok(result)
}

/// The rows of `stored` at `indexes`, in the file's order, or `invalidInput`.
fn selected_rows<'a>(stored: &'a StoredNotesImport, indexes: &[u32]) -> Result<Vec<&'a StoredRow>> {
    let wanted: BTreeSet<u32> = indexes.iter().copied().collect();

    if wanted.is_empty() {
        return Err(BusinessError::InvalidInput(
            "select at least one note to import".to_owned(),
        ));
    }

    let selected: Vec<&StoredRow> = stored
        .rows
        .iter()
        .filter(|row| wanted.contains(&row.index) && row.importable)
        .collect();

    if selected.len() != wanted.len() {
        return Err(BusinessError::InvalidInput(
            "only notes of the file without problems can be imported".to_owned(),
        ));
    }

    Ok(selected)
}

/// Counts the answer of rewriting the note that had the title of row `index`.
fn record_replace(result: &mut NotesImportResult, index: u32, replaced: Result<crate::model::NoteSaved>) {
    match replaced {
        Ok(saved) if saved.outcome == NoteSaveOutcome::Exists => {
            result.exists.push(NotesImportExisting { index, id: saved.id });
        }
        Ok(_) => result.replaced += 1,
        Err(error) => result.failed.push(failure(index, &error)),
    }
}

fn failure(index: u32, error: &BusinessError) -> NotesImportFailure {
    NotesImportFailure {
        index,
        message: error.to_string(),
    }
}

/// A row as the review shows it and as it is kept.
struct CheckedRow {
    row: NotesImportRow,
    stored: StoredRow,
}

fn checked_row(index: u32, raw: RawRow, has_project: bool) -> CheckedRow {
    let mut problems: Vec<NotesImportProblem> = raw
        .wrong_type
        .iter()
        .map(|field| problem(NotesImportProblemCode::WrongType, *field))
        .collect();
    let wrong = |field: NotesImportField| raw.wrong_type.contains(&Some(field));

    let title = raw.title.as_deref().unwrap_or_default().trim().to_owned();
    if !wrong(NotesImportField::Title) {
        problems.extend(title_problem(&title));
    }

    let body = raw.body.clone().unwrap_or_default();
    if !wrong(NotesImportField::Body) {
        problems.extend(body_problem(&body));
    }

    let kind = match raw.kind.as_deref().map(str::trim).filter(|kind| !kind.is_empty()) {
        None => NoteKind::Note,
        Some(kind) => kind_of(kind).unwrap_or_else(|| {
            problems.push(problem(
                NotesImportProblemCode::UnknownKind,
                Some(NotesImportField::Kind),
            ));
            NoteKind::Note
        }),
    };

    let tags = tags_of(&raw.tags);
    problems.extend(tags_problem(&tags));
    let shown_tags: Vec<String> = tags.kept.iter().map(|tag| shortened(tag)).collect();

    let scope = match raw.scope.as_deref().map(str::trim).filter(|scope| !scope.is_empty()) {
        None => None,
        Some(scope) => {
            let found = scope_of(scope);

            match found {
                None => problems.push(problem(
                    NotesImportProblemCode::UnknownScope,
                    Some(NotesImportField::Scope),
                )),
                Some(ScriptScope::Repo) if !has_project => {
                    problems.push(problem(
                        NotesImportProblemCode::NeedsProject,
                        Some(NotesImportField::Scope),
                    ));
                }
                Some(_) => {}
            }

            found
        }
    };

    let importable = problems.is_empty();

    CheckedRow {
        row: NotesImportRow {
            index,
            title: shortened(&title),
            kind,
            tags: shown_tags,
            scope,
            body_preview: shortened(&body.split_whitespace().collect::<Vec<_>>().join(" ")),
            problems,
            duplicate_of: None,
        },
        stored: StoredRow {
            index,
            title,
            body,
            kind,
            tags: tags.kept,
            scope,
            importable,
        },
    }
}

fn problem(code: NotesImportProblemCode, field: Option<NotesImportField>) -> NotesImportProblem {
    NotesImportProblem { code, field }
}

/// The problem of a trimmed title, as [`save_note`] checks it.
fn title_problem(title: &str) -> Option<NotesImportProblem> {
    let code = if title.is_empty() {
        NotesImportProblemCode::Missing
    } else if title.contains(['\n', '\r']) {
        NotesImportProblemCode::MultiLine
    } else if title.chars().count() > TITLE_MAX {
        NotesImportProblemCode::TooLong
    } else {
        return None;
    };

    Some(problem(code, Some(NotesImportField::Title)))
}

/// The problem of a body, as [`save_note`] checks it.
fn body_problem(body: &str) -> Option<NotesImportProblem> {
    let body = body.trim();

    let code = if body.is_empty() {
        NotesImportProblemCode::Missing
    } else if body.chars().count() > BODY_MAX {
        NotesImportProblemCode::TooLong
    } else {
        return None;
    };

    Some(problem(code, Some(NotesImportField::Body)))
}

/// The note kind named `name` (any case; `optimization` is `optimisation`).
fn kind_of(name: &str) -> Option<NoteKind> {
    let kind = match name.to_lowercase().as_str() {
        "outcome" => NoteKind::Outcome,
        "problem" => NoteKind::Problem,
        "fix" => NoteKind::Fix,
        "optimisation" | "optimization" => NoteKind::Optimisation,
        "location" => NoteKind::Location,
        "decision" => NoteKind::Decision,
        "note" => NoteKind::Note,
        _ => return None,
    };

    Some(kind)
}

/// The scope named `name` (any case).
fn scope_of(name: &str) -> Option<ScriptScope> {
    match name.to_lowercase().as_str() {
        "repo" | "repository" | "project" => Some(ScriptScope::Repo),
        "global" => Some(ScriptScope::Global),
        _ => None,
    }
}

/// The tags of a row: at most [`TAGS_MAX`], and whether there were more.
struct Tags {
    kept: Vec<String>,
    too_many: bool,
}

/// The tags in `cells`, split on commas, semicolons, pipes and whitespace, lowercased, without
/// duplicates; reading stops at the first tag over [`TAGS_MAX`].
fn tags_of(cells: &[String]) -> Tags {
    let separator = |character: char| character.is_whitespace() || ",;|".contains(character);
    let mut seen: HashSet<String> = HashSet::new();
    let mut kept: Vec<String> = Vec::new();

    for tag in cells.iter().flat_map(|cell| cell.split(separator)) {
        if tag.is_empty() {
            continue;
        }

        let tag = tag.to_lowercase();

        if seen.contains(&tag) {
            continue;
        }

        if kept.len() == TAGS_MAX {
            return Tags { kept, too_many: true };
        }

        seen.insert(tag.clone());
        kept.push(tag);
    }

    Tags { kept, too_many: false }
}

/// The problem of split tags, as [`save_note`] checks them.
fn tags_problem(tags: &Tags) -> Option<NotesImportProblem> {
    let allowed =
        |character: char| character.is_ascii_lowercase() || character.is_ascii_digit() || "_-".contains(character);

    let code = if tags.too_many {
        NotesImportProblemCode::TooMany
    } else if tags.kept.iter().any(|tag| !tag.chars().all(allowed)) {
        NotesImportProblemCode::InvalidChars
    } else if tags.kept.iter().any(|tag| tag.chars().count() > TAG_MAX) {
        NotesImportProblemCode::TooLong
    } else {
        return None;
    };

    Some(problem(code, Some(NotesImportField::Tags)))
}

/// `text` cut at [`PREVIEW_CHARS`] characters, with `…` when cut.
fn shortened(text: &str) -> String {
    match text.char_indices().nth(PREVIEW_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests;
