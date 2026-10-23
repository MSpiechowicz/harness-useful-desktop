//! The Notes view's notes (`memory_notes_list`, `memory_note_get`, `memory_note_delete`,
//! `memory_notes_delete_all`, `memory_note_hide`, `memory_notes_discard_local`), through the
//! installed `harness-memory notes list|get|delete|hide|unhide|discard-local` (15 s each, 60 s to
//! delete them all or discard local ones), and one created or edited through `harness-memory mcp`
//! (`memory_note_save`: `note_save` or `note_update`). The app never opens the memory database.
//!
//! A `project` must be the active or a recent project; its canonical root goes to the server as
//! `--project`. Null asks for the global notes only.

use harness_business::ecosystem::{
    ListWindow, NotesQuery, Repos, delete_all_notes, delete_note, discard_local, get_note, hide_entry, list_notes,
    save_note,
};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, MemoryDeleteAll, MemoryNotesPage, MemoryRepos, NoteDetail, NoteDraft, NoteKind,
    NoteLangFilter, NoteSaved, NoteSort, ScriptPlace, ScriptScope, SortOrder,
};
use tauri::State;

use super::memory_scripts::project_root;
use crate::error::CommandResult;
use crate::state::AppState;

/// One page of the project's repository notes and the global ones, of one `kind`, one `scope` and
/// one `place` and one detected `lang` (`en`, `other`, `unknown`) when asked. A server that does not list
/// `notes-lang` ignores `lang` (its notes carry no language). Never fails for a missing or old
/// server: see the page's `availability`
/// (`notes` is empty unless `available`); a server that does not filter by place drops `place` and
/// answers `places: null`, one that does not combine filters drops `scope` too and answers
/// `combinedFilters: false`. `hidden` lists the team notes hidden on this machine instead.
/// `invalidInput` for a limit of 0 or above 200, or a query above 256 characters, `forbidden` for
/// a project that is not open or recent.
#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "IPC arguments arrive one per name; grouping them would change the command's contract"
)]
pub async fn memory_notes_list(
    state: State<'_, AppState>,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    offset: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
    kind: Option<NoteKind>,
    scope: Option<ScriptScope>,
    place: Option<ScriptPlace>,
    lang: Option<NoteLangFilter>,
    sort: Option<NoteSort>,
    order: Option<SortOrder>,
    hidden: Option<bool>,
) -> CommandResult<MemoryNotesPage> {
    let query = NotesQuery {
        window: ListWindow::new(offset, limit, query, order)?,
        kind,
        scope,
        place,
        lang,
        sort,
        hidden: hidden.unwrap_or(false),
    };

    list(&state, project, repos, &query).await
}

/// One note with its body and links; with `hidden` a team note hidden on this machine. `notFound`
/// when it is gone (or hidden, without `hidden`) or the server is not installed, `unsupported` when
/// the server predates the notes view.
#[tauri::command]
pub async fn memory_note_get(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    hidden: Option<bool>,
) -> CommandResult<NoteDetail> {
    get(&state, id, project, repos, hidden.unwrap_or(false)).await
}

/// Hides a team note on this machine (`hidden`), or shows it again; the team database is not
/// changed. `notFound` when it is gone or the server is not installed, `unsupported` when the
/// server predates hiding, `process` with the server's line when it refuses (a note saved on this
/// machine, no team database), `forbidden` for a project that is not open or recent.
#[tauri::command]
pub async fn memory_note_hide(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    hidden: bool,
) -> CommandResult<()> {
    let project = project_root(&state, project).await?;

    Ok(hide_entry(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        MemoryCopyKind::Notes,
        id,
        Repos::of(project.as_deref(), repos.as_ref())?,
        hidden,
    )
    .await?)
}

/// Deletes notes from this machine's file in the note scopes routed to the team database, where
/// the server no longer reads them (the UI confirms first): `"all"` or those local ids, of `scope`
/// only when given. How many were deleted. `invalidInput` for `"none"` or no ids, `notFound` when
/// the server is not installed, `unsupported` when it predates discarding, `process` with the
/// server's line when it refuses (a scope not routed to the team database).
#[tauri::command]
pub async fn memory_notes_discard_local(
    state: State<'_, AppState>,
    ids: LocalNoteIds,
    scope: Option<ScriptScope>,
) -> CommandResult<u32> {
    let notes = MemoryCopyKind::Notes;

    Ok(discard_local(&state.env, &state.memory_scripts, &state.runner, notes, &ids, scope).await?)
}

/// Deletes one note (the UI confirms first). `notFound` when it is gone or the server is not
/// installed, `unsupported` when the server predates the notes view.
#[tauri::command]
pub async fn memory_note_delete(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
    repos: Option<MemoryRepos>,
) -> CommandResult<()> {
    delete(&state, id, project, repos).await
}

/// Deletes every note the list shows for the project without filters (the UI confirms first):
/// how many, and how many of them were in the team database. `notFound` when the server is not
/// installed, `unsupported` when it predates deleting them all, `forbidden` for a project that is
/// not open or recent.
#[tauri::command]
pub async fn memory_notes_delete_all(
    state: State<'_, AppState>,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    scope: Option<ScriptScope>,
) -> CommandResult<MemoryDeleteAll> {
    delete_all(&state, project, repos, scope).await
}

/// Creates a note, or with `replace_id` rewrites that note (an edit, or replacing the note that
/// has the title; its scope stays). `exists` (nothing saved) with the other note's id when a note
/// of the project's repository, or of the edited note's scope, has the title. `invalidInput` for
/// a draft over a limit or a new repository note without a project, `notFound` when the note to
/// replace is gone or the server is not installed, `unsupported` when the server predates saving
/// from the app, `forbidden` for a project that is not open or recent.
#[tauri::command]
pub async fn memory_note_save(
    state: State<'_, AppState>,
    project: Option<String>,
    draft: NoteDraft,
    replace_id: Option<i64>,
) -> CommandResult<NoteSaved> {
    save(&state, project, &draft, replace_id).await
}

async fn save(
    state: &AppState,
    project: Option<String>,
    draft: &NoteDraft,
    replace_id: Option<i64>,
) -> CommandResult<NoteSaved> {
    let project = project_root(state, project).await?;
    let route = state.save_route().await;

    Ok(save_note(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        route,
        project.as_deref(),
        draft,
        replace_id,
    )
    .await?)
}

async fn list(
    state: &AppState,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    query: &NotesQuery,
) -> CommandResult<MemoryNotesPage> {
    let project = project_root(state, project).await?;

    // A server without `notes-lang` would refuse `--lang` as a usage error.
    let query = if query.lang.is_some() {
        query.for_server(state.memory_features.support(&state.env, &state.runner).await)
    } else {
        query.clone()
    };

    Ok(list_notes(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        Repos::of(project.as_deref(), repos.as_ref())?,
        &query,
    )
    .await)
}

async fn get(
    state: &AppState,
    id: i64,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    hidden: bool,
) -> CommandResult<NoteDetail> {
    let project = project_root(state, project).await?;

    Ok(get_note(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        id,
        Repos::of(project.as_deref(), repos.as_ref())?,
        hidden,
    )
    .await?)
}

async fn delete(state: &AppState, id: i64, project: Option<String>, repos: Option<MemoryRepos>) -> CommandResult<()> {
    let project = project_root(state, project).await?;
    let repos = Repos::of(project.as_deref(), repos.as_ref())?;

    Ok(delete_note(&state.env, &state.memory_scripts, &state.runner, id, repos).await?)
}

async fn delete_all(
    state: &AppState,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    scope: Option<ScriptScope>,
) -> CommandResult<MemoryDeleteAll> {
    let project = project_root(state, project).await?;

    Ok(delete_all_notes(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        Repos::of(project.as_deref(), repos.as_ref())?,
        scope,
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use harness_business::model::{IpcErrorCode, MemoryScriptsAvailability, NoteSaveOutcome};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{ProjectHome, path_arg, place_memory_server};

    fn first_page() -> NotesQuery {
        NotesQuery {
            window: ListWindow::new(None, None, None, None).expect("the first page"),
            kind: None,
            scope: None,
            place: None,
            lang: None,
            sort: None,
            hidden: false,
        }
    }

    fn draft(title: &str, scope: ScriptScope) -> NoteDraft {
        NoteDraft {
            title: title.to_owned(),
            body: "Written in the app.".to_owned(),
            kind: NoteKind::Decision,
            tags: vec!["App".to_owned()],
            entities: Vec::new(),
            scope,
        }
    }

    fn saved(outcome: NoteSaveOutcome, id: i64) -> NoteSaved {
        NoteSaved { outcome, id: Some(id) }
    }

    #[test]
    fn a_project_that_is_not_open_or_recent_is_refused() {
        let setup = ProjectHome::new("notes-stranger");
        let (state, stranger) = (&setup.state, &setup.stranger);

        let codes = [
            block_on(list(state, path_arg(stranger), None, &first_page())).map(|_| ()),
            block_on(get(state, 3, path_arg(stranger), None, false)).map(|_| ()),
            block_on(delete(state, 3, path_arg(stranger), None)),
            block_on(delete_all(state, path_arg(stranger), None, None)).map(|_| ()),
            block_on(save(
                state,
                path_arg(stranger),
                &draft("A new note", ScriptScope::Repo),
                None,
            ))
            .map(|_| ()),
        ]
        .map(|result| result.expect_err("unknown project").code());

        assert_eq!(codes, [IpcErrorCode::Forbidden; 5]);
    }

    #[test]
    fn the_installed_server_reads_the_project_notes_and_deletes_one() {
        let setup = ProjectHome::new("notes-list");
        let (state, project) = (&setup.state, &setup.project);

        let page = block_on(list(state, None, None, &first_page())).expect("listed");
        assert_eq!(page.availability, MemoryScriptsAvailability::NotInstalled);

        let error = block_on(get(state, 3, None, None, false)).expect_err("not installed");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        if place_memory_server(&setup.home).is_none() {
            return;
        }

        let page = block_on(list(state, path_arg(project), None, &first_page())).expect("listed");
        assert_eq!(page.availability, MemoryScriptsAvailability::Available);
        assert_eq!(page.total, 8, "the fixtures, one of a kind the app skips");
        assert_eq!(page.notes.len(), 7);

        let detail = block_on(get(state, 3, path_arg(project), None, false)).expect("repository note");
        assert_eq!(detail.note.summary.title, "Watcher misses renames");

        let error = block_on(get(state, 3, None, None, false)).expect_err("a repository note without its project");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        block_on(delete(state, 3, path_arg(project), None)).expect("deleted");

        let deleted = block_on(delete_all(state, path_arg(project), None, None)).expect("deleted them all");
        assert_eq!(
            deleted,
            MemoryDeleteAll {
                deleted: 8,
                team: Some(3),
                kept: None,
            }
        );

        let global = block_on(delete_all(state, None, None, None)).expect("deleted the global ones");
        assert_eq!(global.deleted, 5);

        block_on(state.memory_scripts.shutdown());
    }

    #[test]
    fn the_installed_server_creates_edits_and_refuses_a_taken_title() {
        let setup = ProjectHome::new("notes-save");
        let (state, project) = (&setup.state, &setup.project);
        let new = draft("A note from the app", ScriptScope::Repo);

        let error = block_on(save(state, path_arg(project), &new, None)).expect_err("not installed");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        if place_memory_server(&setup.home).is_none() {
            return;
        }

        let created = block_on(save(state, path_arg(project), &new, None)).expect("created");
        assert_eq!(created, saved(NoteSaveOutcome::Created, 9), "read past the frame line");

        let taken = draft("Watcher misses renames", ScriptScope::Repo);
        let exists = block_on(save(state, path_arg(project), &taken, None)).expect("refused");
        assert_eq!(
            exists,
            saved(NoteSaveOutcome::Exists, 3),
            "the repository note with that title"
        );

        let replaced = block_on(save(state, path_arg(project), &taken, Some(3))).expect("replaced");
        assert_eq!(replaced, saved(NoteSaveOutcome::Updated, 3));

        let renamed = draft("Cache the tools probe", ScriptScope::Repo);
        let exists = block_on(save(state, path_arg(project), &renamed, Some(2))).expect("refused");
        assert_eq!(
            exists,
            saved(NoteSaveOutcome::Exists, 5),
            "an edit to a title another note has"
        );

        let global = draft("Prefer bun over npm", ScriptScope::Global);
        let exists = block_on(save(state, None, &global, None)).expect("refused");
        assert_eq!(
            exists,
            saved(NoteSaveOutcome::Exists, 7),
            "the global note with that title is not rewritten"
        );

        let replaced = block_on(save(state, None, &global, Some(7))).expect("replaced by id");
        assert_eq!(replaced, saved(NoteSaveOutcome::Updated, 7));

        let error = block_on(save(state, None, &taken, Some(3))).expect_err("a repository note without its project");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        let error = block_on(save(state, None, &new, None)).expect_err("a repository note needs a project");
        assert_eq!(error.code(), IpcErrorCode::InvalidInput);

        block_on(state.memory_scripts.shutdown());
    }
}
