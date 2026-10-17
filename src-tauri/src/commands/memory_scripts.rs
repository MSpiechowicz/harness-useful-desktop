//! The Memory view's scripts (`memory_scripts_list`, `memory_script_get`,
//! `memory_script_delete`, `memory_scripts_delete_all`), listed a page at a time with
//! `harness-memory scripts list --json`, read and deleted through the installed
//! `harness-memory mcp` (`script_get`, `script_delete`), all deleted with
//! `harness-memory scripts delete --all`, and approved with `harness-memory trust`
//! (`memory_scripts_trust`, each script with the hash of the version the user reviewed). The app never opens the memory database.
//!
//! A `project` must be the active or a recent project; its canonical root goes to the server as
//! `project_path`. Null asks for the global scripts only.

use std::path::PathBuf;

use harness_business::ecosystem::{ListWindow, ScriptsQuery, trust_scripts};
use harness_business::model::{
    MemoryDeleteAll, MemoryScriptsPage, ScriptDetail, ScriptGroup, ScriptSort, ScriptTrustItem, SortOrder,
};
use tauri::State;

use super::authorized_dir;
use crate::error::CommandResult;
use crate::state::AppState;

/// One page of the project's scripts and the global ones (`harness-memory scripts list --json`).
/// Never fails for a missing or old server: see the page's `availability` (`scripts` is empty
/// unless `available`). `invalidInput` for a limit of 0 or above 200, or a query above 256
/// characters, `forbidden` for a project that is not open or recent.
#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "IPC arguments arrive one per name; grouping them would change the command's contract"
)]
pub async fn memory_scripts_list(
    state: State<'_, AppState>,
    project: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
    group: Option<ScriptGroup>,
    sort: Option<ScriptSort>,
    order: Option<SortOrder>,
) -> CommandResult<MemoryScriptsPage> {
    let query = ScriptsQuery {
        window: ListWindow::new(offset, limit, query, order)?,
        group,
        sort,
    };

    list(&state, project, &query).await
}

/// One script with its body. `notFound` when it is gone or the server is not installed,
/// `unsupported` when the server predates scripts.
#[tauri::command]
pub async fn memory_script_get(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
) -> CommandResult<ScriptDetail> {
    let project = project_root(&state, project).await?;

    Ok(state.memory_scripts.get(&state.env, id, project.as_deref()).await?)
}

/// Deletes one script (the UI confirms first). `notFound` when it is gone or the server is not
/// installed, `unsupported` when the server predates scripts.
#[tauri::command]
pub async fn memory_script_delete(state: State<'_, AppState>, id: i64, project: Option<String>) -> CommandResult<()> {
    let project = project_root(&state, project).await?;
    state.memory_scripts.delete(&state.env, id, project.as_deref()).await?;

    tracing::info!(id, "memory script deleted");
    Ok(())
}

/// Deletes every script the list shows for the project without filters (the UI confirms first):
/// how many, and how many of them were in the team database. `notFound` when the server is not
/// installed, `unsupported` when it predates deleting them all, `forbidden` for a project that is
/// not open or recent.
#[tauri::command]
pub async fn memory_scripts_delete_all(
    state: State<'_, AppState>,
    project: Option<String>,
) -> CommandResult<MemoryDeleteAll> {
    delete_all(&state, project).await
}

async fn delete_all(state: &AppState, project: Option<String>) -> CommandResult<MemoryDeleteAll> {
    let project = project_root(state, project).await?;

    Ok(state
        .memory_scripts
        .delete_all(&state.env, &state.runner, project.as_deref())
        .await?)
}

/// Approves the reviewed versions of scripts `items`
/// (`harness-memory trust <id>=<sha256>… [--project <root>]`, 20 s and 1 s per script, 120 s at most), so agents run them without
/// asking. `invalidInput` for no items, more than 500, or a bad id or hash, `conflict` when a
/// script changed since it was reviewed (the others are still approved), `notFound` when Harness
/// Memory is not installed, `process` with the server's (redacted) message when it fails.
#[tauri::command]
pub async fn memory_scripts_trust(
    state: State<'_, AppState>,
    items: Vec<ScriptTrustItem>,
    project: Option<String>,
) -> CommandResult<()> {
    trust(&state, items, project).await
}

async fn trust(state: &AppState, items: Vec<ScriptTrustItem>, project: Option<String>) -> CommandResult<()> {
    let project = project_root(state, project).await?;

    Ok(trust_scripts(&state.env, &state.runner, &items, project.as_deref()).await?)
}

async fn list(state: &AppState, project: Option<String>, query: &ScriptsQuery) -> CommandResult<MemoryScriptsPage> {
    let project = project_root(state, project).await?;

    Ok(state
        .memory_scripts
        .list(&state.env, &state.runner, project.as_deref(), query)
        .await)
}

/// The canonical root of `project` (`forbidden` unless active or recent); `None` for none.
pub(super) async fn project_root(state: &AppState, project: Option<String>) -> CommandResult<Option<PathBuf>> {
    match project {
        Some(project) => Ok(Some(authorized_dir(state, project).await?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use harness_business::model::{IpcErrorCode, MemoryScriptsAvailability};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{ProjectHome, path_arg, place_memory_server};

    fn by_name() -> ScriptsQuery {
        ScriptsQuery {
            window: ListWindow::new(None, None, None, None).expect("the first page"),
            group: None,
            sort: Some(ScriptSort::Name),
        }
    }

    #[test]
    fn a_project_that_is_not_open_or_recent_is_refused() {
        let setup = ProjectHome::new("scripts-stranger");

        let error = block_on(list(&setup.state, path_arg(&setup.stranger), &by_name())).expect_err("unknown project");
        assert_eq!(error.code(), IpcErrorCode::Forbidden);

        let error = block_on(delete_all(&setup.state, path_arg(&setup.stranger))).expect_err("unknown project");
        assert_eq!(error.code(), IpcErrorCode::Forbidden);
    }

    #[test]
    fn trusting_checks_the_project_and_needs_the_server() {
        let setup = ProjectHome::new("scripts-trust");
        let state = &setup.state;

        let item = || {
            vec![ScriptTrustItem {
                id: 12,
                sha256: "ab".repeat(32),
            }]
        };

        let error = block_on(trust(state, item(), path_arg(&setup.stranger))).expect_err("unknown project");
        assert_eq!(error.code(), IpcErrorCode::Forbidden);

        let error = block_on(trust(state, item(), path_arg(&setup.project))).expect_err("not installed");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        let error = block_on(trust(state, Vec::new(), None)).expect_err("no ids");
        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
    }

    #[test]
    fn the_installed_server_lists_the_project_scripts_and_deletes_them_all() {
        let setup = ProjectHome::new("scripts-list");
        let state = &setup.state;

        let page = block_on(list(state, None, &by_name())).expect("listed");
        assert_eq!(page.availability, MemoryScriptsAvailability::NotInstalled);

        if place_memory_server(&setup.home).is_none() {
            return;
        }

        let page = block_on(list(state, path_arg(&setup.project), &by_name())).expect("listed");
        assert_eq!(page.availability, MemoryScriptsAvailability::Available);

        let names: Vec<&str> = page.scripts.iter().map(|script| script.name.as_str()).collect();
        assert_eq!(names, ["open-prs", "release-notes"]);

        let deleted = block_on(delete_all(state, path_arg(&setup.project))).expect("deleted them all");
        assert_eq!(
            deleted,
            MemoryDeleteAll {
                deleted: 3,
                team: Some(0)
            }
        );

        block_on(state.memory_scripts.shutdown());
    }
}
