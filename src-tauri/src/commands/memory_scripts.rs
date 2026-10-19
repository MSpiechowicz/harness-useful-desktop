//! The Memory view's scripts (`memory_scripts_list`, `memory_script_get`,
//! `memory_script_delete`, `memory_scripts_delete_all`, `memory_script_hide`,
//! `memory_scripts_discard_local`), listed a page at a time with
//! `harness-memory scripts list --json`, read and deleted through the installed
//! `harness-memory mcp` (`script_get`, `script_delete`; a hidden one read with `harness-memory
//! scripts get --hidden`), hidden on this machine with `harness-memory scripts hide|unhide`, local
//! copies discarded with `harness-memory scripts discard-local`, created and edited there
//! (`memory_script_save`: `script_save`, after `script_get` by name unless replacing), all deleted with
//! `harness-memory scripts delete --all`, and approved with `harness-memory trust`
//! (`memory_scripts_trust`, each script with the hash of the version the user reviewed). The app never opens the memory database.
//!
//! A `project` must be the active or a recent project; its canonical root goes to the server as
//! `project_path`. Null asks for the global scripts only.

use std::path::PathBuf;

use harness_business::ecosystem::{ListWindow, ScriptsQuery, discard_local, hide_entry, trust_scripts};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, MemoryDeleteAll, MemoryScriptsPage, ScriptDetail, ScriptDraft, ScriptGroup,
    ScriptReview, ScriptSaved, ScriptScope, ScriptSort, ScriptTrustItem, SortOrder,
};
use tauri::State;

use super::authorized_dir;
use crate::error::CommandResult;
use crate::state::AppState;

/// One page of the project's scripts and the global ones (`harness-memory scripts list --json`).
/// One `group`, or one `scope` and one `review` combined. Never fails for a missing or old server:
/// see the page's `availability` (`scripts` is empty unless `available`); a server that does not
/// combine filters drops `scope` and `review` and answers `combinedFilters: false`. `hidden`
/// lists the team scripts hidden on this machine instead. `invalidInput` for a limit of 0 or above 200, a query above 256 characters, or a `group` with
/// a `scope` or `review`, `forbidden` for a project that is not open or recent.
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
    scope: Option<ScriptScope>,
    review: Option<ScriptReview>,
    sort: Option<ScriptSort>,
    order: Option<SortOrder>,
    hidden: Option<bool>,
) -> CommandResult<MemoryScriptsPage> {
    let window = ListWindow::new(offset, limit, query, order)?;
    let query = ScriptsQuery {
        hidden: hidden.unwrap_or(false),
        ..ScriptsQuery::new(window, group, scope, review, sort)?
    };

    list(&state, project, &query).await
}

/// One script with its body; with `hidden` a team script hidden on this machine (only, read with
/// `harness-memory scripts get --hidden`). `notFound` when it is gone (or hidden, without
/// `hidden`) or the server is not installed, `unsupported` when the server predates scripts (or
/// hiding them).
#[tauri::command]
pub async fn memory_script_get(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
    hidden: Option<bool>,
) -> CommandResult<ScriptDetail> {
    let project = project_root(&state, project).await?;
    let (env, scripts) = (&state.env, &state.memory_scripts);

    if hidden.unwrap_or(false) {
        return Ok(scripts.get_hidden(env, &state.runner, id, project.as_deref()).await?);
    }

    Ok(scripts.get(env, id, project.as_deref()).await?)
}

/// Hides a team script on this machine (`hidden`), or shows it again; the team database is not
/// changed. `notFound` when it is gone or the server is not installed, `unsupported` when the
/// server predates hiding scripts, `process` with the server's line when it refuses (a script
/// saved on this machine, no team database), `forbidden` for a project that is not open or recent.
#[tauri::command]
pub async fn memory_script_hide(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
    hidden: bool,
) -> CommandResult<()> {
    hide(&state, id, project, hidden).await
}

async fn hide(state: &AppState, id: i64, project: Option<String>, hidden: bool) -> CommandResult<()> {
    let project = project_root(state, project).await?;
    let scripts = MemoryCopyKind::Scripts;

    Ok(hide_entry(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        scripts,
        id,
        project.as_deref(),
        hidden,
    )
    .await?)
}

/// Deletes scripts from this machine's file while scripts are routed to the team database alone,
/// where the server no longer reads them (the UI confirms first): `"all"` or those local ids, of
/// `scope` only when given. How many were deleted. `invalidInput` for `"none"` or no ids,
/// `notFound` when the server is not installed, `unsupported` when it predates discarding scripts,
/// `process` with the server's line when it refuses (scripts not routed to the team database
/// alone).
#[tauri::command]
pub async fn memory_scripts_discard_local(
    state: State<'_, AppState>,
    ids: LocalNoteIds,
    scope: Option<ScriptScope>,
) -> CommandResult<u32> {
    let scripts = MemoryCopyKind::Scripts;

    Ok(discard_local(&state.env, &state.memory_scripts, &state.runner, scripts, &ids, scope).await?)
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

/// Creates a script, or with `replace` replaces the one of that name in that scope (an edit, or a
/// confirmed replace). An edit also passes `base`, the `sha256` of the version the editor opened:
/// `conflict` (nothing saved) when that script was deleted or changed since. Saved here, so
/// trusted on this machine: when the server answers `needed` (the code of a script waiting for
/// approval, or a copy of one) the saved version is approved (`harness-memory trust`) only when
/// the user put the code in and the stored script is the draft. An edit counts when it changed
/// the code it opened (line endings and trailing whitespace aside), so an edit of the
/// description, timeout or language alone stays `needed`; a confirmed replace counts unless it
/// keeps the code of the untrusted script it replaces; a new name always counts. `exists`
/// (nothing saved) with that script's id when one has the name and `replace` is false.
/// `invalidInput` for a draft over a limit or a repository script without a project,
/// `notFound` when the server is not installed, `unsupported` when it predates saving from the
/// app, `forbidden` for a project that is not open or recent.
#[tauri::command]
pub async fn memory_script_save(
    state: State<'_, AppState>,
    project: Option<String>,
    draft: ScriptDraft,
    replace: bool,
    base: Option<String>,
) -> CommandResult<ScriptSaved> {
    save(&state, project, &draft, replace, base.as_deref()).await
}

async fn save(
    state: &AppState,
    project: Option<String>,
    draft: &ScriptDraft,
    replace: bool,
    base: Option<&str>,
) -> CommandResult<ScriptSaved> {
    let project = project_root(state, project).await?;

    Ok(state
        .memory_scripts
        .save(&state.env, &state.runner, project.as_deref(), draft, replace, base)
        .await?)
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
    use harness_business::model::{
        IpcErrorCode, MemoryScriptsAvailability, ScriptApproval, ScriptLanguage, ScriptSaveOutcome,
    };
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{ProjectHome, path_arg, place_memory_server};

    fn draft(name: &str, scope: ScriptScope) -> ScriptDraft {
        ScriptDraft {
            name: name.to_owned(),
            description: "Written in the app".to_owned(),
            language: ScriptLanguage::Bash,
            body: "echo app\n".to_owned(),
            timeout_secs: 30,
            scope,
        }
    }

    fn by_name() -> ScriptsQuery {
        let window = ListWindow::new(None, None, None, None).expect("the first page");

        ScriptsQuery::new(window, None, None, None, Some(ScriptSort::Name)).expect("no filters")
    }

    #[test]
    fn a_project_that_is_not_open_or_recent_is_refused() {
        let setup = ProjectHome::new("scripts-stranger");

        let error = block_on(list(&setup.state, path_arg(&setup.stranger), &by_name())).expect_err("unknown project");
        assert_eq!(error.code(), IpcErrorCode::Forbidden);

        let error = block_on(delete_all(&setup.state, path_arg(&setup.stranger))).expect_err("unknown project");
        assert_eq!(error.code(), IpcErrorCode::Forbidden);

        let error = block_on(hide(&setup.state, 30, path_arg(&setup.stranger), true)).expect_err("unknown project");
        assert_eq!(error.code(), IpcErrorCode::Forbidden);

        let error = block_on(save(
            &setup.state,
            path_arg(&setup.stranger),
            &draft("open-prs", ScriptScope::Repo),
            false,
            None,
        ))
        .expect_err("unknown project");
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
                team: Some(0),
                kept: None,
            }
        );

        block_on(state.memory_scripts.shutdown());
    }

    #[test]
    fn the_installed_server_saves_a_script_and_replaces_one_only_when_asked() {
        let setup = ProjectHome::new("scripts-save");
        let (state, project) = (&setup.state, &setup.project);

        let error = block_on(save(
            state,
            path_arg(project),
            &draft("app-script", ScriptScope::Repo),
            false,
            None,
        ))
        .expect_err("not installed");
        assert_eq!(error.code(), IpcErrorCode::NotFound);

        if place_memory_server(&setup.home).is_none() {
            return;
        }

        let created = block_on(save(
            state,
            path_arg(project),
            &draft("app-script", ScriptScope::Repo),
            false,
            None,
        ))
        .expect("created");
        assert_eq!(created.outcome, ScriptSaveOutcome::Created);
        let script = created.script.expect("the saved script");
        assert_eq!((script.id, script.approval), (created.id, ScriptApproval::Local));

        let exists = block_on(save(
            state,
            path_arg(project),
            &draft("open-prs", ScriptScope::Repo),
            false,
            None,
        ))
        .expect("refused");
        assert_eq!(
            (exists.outcome, exists.id, exists.script),
            (ScriptSaveOutcome::Exists, 12, None)
        );

        let global = block_on(save(
            state,
            path_arg(project),
            &draft("open-prs", ScriptScope::Global),
            false,
            None,
        ))
        .expect("another scope");
        assert_eq!(
            global.outcome,
            ScriptSaveOutcome::Created,
            "the name is taken in the repository only"
        );

        let replaced = block_on(save(
            state,
            path_arg(project),
            &draft("open-prs", ScriptScope::Repo),
            true,
            None,
        ))
        .expect("replaced");
        assert_eq!((replaced.outcome, replaced.id), (ScriptSaveOutcome::Replaced, 12));
        assert_eq!(replaced.script.expect("the saved script").timeout_secs, 30);

        let waiting = block_on(save(
            state,
            path_arg(project),
            &draft("release-notes", ScriptScope::Global),
            true,
            None,
        ))
        .expect("replaced");
        assert_eq!(
            (waiting.id, waiting.script.expect("the saved script").approval),
            (7, ScriptApproval::Local),
            "other code over a script waiting for approval is the user's, and the server trusts it as saved here"
        );

        let error = block_on(save(state, None, &draft("app-script", ScriptScope::Repo), false, None))
            .expect_err("a repository script needs a project");
        assert_eq!(error.code(), IpcErrorCode::InvalidInput);

        block_on(state.memory_scripts.shutdown());
    }
}
