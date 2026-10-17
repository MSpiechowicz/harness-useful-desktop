//! The Memory view's notes (`memory_notes_list`, `memory_note_get`, `memory_note_delete`,
//! `memory_notes_delete_all`), through the installed `harness-memory notes list|get|delete` (15 s
//! each, 60 s to delete them all). The app never opens the memory database.
//!
//! A `project` must be the active or a recent project; its canonical root goes to the server as
//! `--project`. Null asks for the global notes only.

use harness_business::ecosystem::{ListWindow, NotesQuery, delete_all_notes, delete_note, get_note, list_notes};
use harness_business::model::{MemoryDeleteAll, MemoryNotesPage, NoteDetail, NoteKind, NoteSort, SortOrder};
use tauri::State;

use super::memory_scripts::project_root;
use crate::error::CommandResult;
use crate::state::AppState;

/// One page of the project's repository notes and the global ones. Never fails for a missing or
/// old server: see the page's `availability` (`notes` is empty unless `available`).
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
    offset: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
    kind: Option<NoteKind>,
    sort: Option<NoteSort>,
    order: Option<SortOrder>,
) -> CommandResult<MemoryNotesPage> {
    let query = NotesQuery {
        window: ListWindow::new(offset, limit, query, order)?,
        kind,
        sort,
    };

    list(&state, project, &query).await
}

/// One note with its body and links. `notFound` when it is gone or the server is not installed,
/// `unsupported` when the server predates the notes view.
#[tauri::command]
pub async fn memory_note_get(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
) -> CommandResult<NoteDetail> {
    get(&state, id, project).await
}

/// Deletes one note (the UI confirms first). `notFound` when it is gone or the server is not
/// installed, `unsupported` when the server predates the notes view.
#[tauri::command]
pub async fn memory_note_delete(state: State<'_, AppState>, id: i64, project: Option<String>) -> CommandResult<()> {
    delete(&state, id, project).await
}

/// Deletes every note the list shows for the project without filters (the UI confirms first):
/// how many, and how many of them were in the team database. `notFound` when the server is not
/// installed, `unsupported` when it predates deleting them all, `forbidden` for a project that is
/// not open or recent.
#[tauri::command]
pub async fn memory_notes_delete_all(
    state: State<'_, AppState>,
    project: Option<String>,
) -> CommandResult<MemoryDeleteAll> {
    delete_all(&state, project).await
}

async fn list(state: &AppState, project: Option<String>, query: &NotesQuery) -> CommandResult<MemoryNotesPage> {
    let project = project_root(state, project).await?;

    Ok(list_notes(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        project.as_deref(),
        query,
    )
    .await)
}

async fn get(state: &AppState, id: i64, project: Option<String>) -> CommandResult<NoteDetail> {
    let project = project_root(state, project).await?;

    Ok(get_note(&state.env, &state.memory_scripts, &state.runner, id, project.as_deref()).await?)
}

async fn delete(state: &AppState, id: i64, project: Option<String>) -> CommandResult<()> {
    let project = project_root(state, project).await?;

    Ok(delete_note(&state.env, &state.memory_scripts, &state.runner, id, project.as_deref()).await?)
}

async fn delete_all(state: &AppState, project: Option<String>) -> CommandResult<MemoryDeleteAll> {
    let project = project_root(state, project).await?;

    Ok(delete_all_notes(&state.env, &state.memory_scripts, &state.runner, project.as_deref()).await?)
}

#[cfg(test)]
mod tests {
    use harness_business::model::{IpcErrorCode, MemoryScriptsAvailability};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{ProjectHome, path_arg, place_memory_server};

    fn first_page() -> NotesQuery {
        NotesQuery {
            window: ListWindow::new(None, None, None, None).expect("the first page"),
            kind: None,
            sort: None,
        }
    }

    #[test]
    fn a_project_that_is_not_open_or_recent_is_refused() {
        let setup = ProjectHome::new("notes-stranger");
        let (state, stranger) = (&setup.state, &setup.stranger);

        let codes = [
            block_on(list(state, path_arg(stranger), &first_page())).map(|_| ()),
            block_on(get(state, 3, path_arg(stranger))).map(|_| ()),
            block_on(delete(state, 3, path_arg(stranger))),
            block_on(delete_all(state, path_arg(stranger))).map(|_| ()),
        ]
        .map(|result| result.expect_err("unknown project").code());

        assert_eq!(codes, [IpcErrorCode::Forbidden; 4]);
    }

    #[test]
    fn the_installed_server_reads_the_project_notes_and_deletes_one() {
        let setup = ProjectHome::new("notes-list");
        let (state, project) = (&setup.state, &setup.project);

        let page = block_on(list(state, None, &first_page())).expect("listed");
        assert_eq!(page.availability, MemoryScriptsAvailability::NotInstalled);

        let error = block_on(get(state, 3, None)).expect_err("not installed");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        if place_memory_server(&setup.home).is_none() {
            return;
        }

        let page = block_on(list(state, path_arg(project), &first_page())).expect("listed");
        assert_eq!(page.availability, MemoryScriptsAvailability::Available);
        assert_eq!(page.total, 8, "the fixtures, one of a kind the app skips");
        assert_eq!(page.notes.len(), 7);

        let detail = block_on(get(state, 3, path_arg(project))).expect("repository note");
        assert_eq!(detail.note.summary.title, "Watcher misses renames");

        let error = block_on(get(state, 3, None)).expect_err("a repository note without its project");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        block_on(delete(state, 3, path_arg(project))).expect("deleted");

        let deleted = block_on(delete_all(state, path_arg(project))).expect("deleted them all");
        assert_eq!(
            deleted,
            MemoryDeleteAll {
                deleted: 8,
                team: Some(3)
            }
        );

        let global = block_on(delete_all(state, None)).expect("deleted the global ones");
        assert_eq!(global.deleted, 5);

        block_on(state.memory_scripts.shutdown());
    }
}
