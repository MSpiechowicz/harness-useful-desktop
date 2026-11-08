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

use harness_business::ecosystem::{
    ListWindow, Repos, ScriptsQuery, discard_local, hide_entry, list_repos, trust_scripts,
};
use harness_business::model::{
    AuthorFilter, LocalNoteIds, MemoryCopyKind, MemoryDeleteAll, MemoryRepos, MemoryReposPage, MemorySavePlace,
    MemoryScriptsPage, ScriptDetail, ScriptDraft, ScriptGroup, ScriptPlace, ScriptReview, ScriptSaved, ScriptScope,
    ScriptSort, ScriptTrustItem, SortOrder,
};
use tauri::State;

use super::authorized_dir;
use crate::error::CommandResult;
use crate::state::AppState;

/// One page of the project's scripts and the global ones (`harness-memory scripts list --json`).
/// One `group`, or one `scope` and one `review` combined, and one `place`. Never fails for a
/// missing or old server: see the page's `availability` (`scripts` is empty unless `available`);
/// a server that does not filter by place drops `place` and answers `places: null`, one that does
/// not combine filters drops `scope` and `review` too and answers `combinedFilters: false`. `hidden`
/// lists the team scripts hidden on this machine instead. `author` (a name, or the scripts without
/// one) and the `addedBy` sort need `notes-author-filter`: a server without it ignores them, and
/// its page has `authors: null`. `invalidInput` for a limit of 0 or above 200, a query above 256
/// characters, a `group` with a `scope` or `review`, or an author name the servers would refuse
/// (empty, over 80 characters, edge spaces, control or invisible characters), `forbidden` for a
/// project that is not open or recent.
#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "IPC arguments arrive one per name; grouping them would change the command's contract"
)]
pub async fn memory_scripts_list(
    state: State<'_, AppState>,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    offset: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
    group: Option<ScriptGroup>,
    scope: Option<ScriptScope>,
    review: Option<ScriptReview>,
    place: Option<ScriptPlace>,
    author: Option<AuthorFilter>,
    sort: Option<ScriptSort>,
    order: Option<SortOrder>,
    hidden: Option<bool>,
) -> CommandResult<MemoryScriptsPage> {
    let window = ListWindow::new(offset, limit, query, order)?;
    let query = ScriptsQuery {
        place,
        author,
        hidden: hidden.unwrap_or(false),
        ..ScriptsQuery::new(window, group, scope, review, sort)?
    };

    list(&state, project, repos, &query).await
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
    repos: Option<MemoryRepos>,
    hidden: Option<bool>,
) -> CommandResult<ScriptDetail> {
    let project = project_root(&state, project).await?;
    let (env, scripts) = (&state.env, &state.memory_scripts);
    let repos = Repos::of(project.as_deref(), repos.as_ref())?;

    if hidden.unwrap_or(false) {
        return Ok(scripts.get_hidden(env, &state.runner, id, repos).await?);
    }

    Ok(scripts.get(env, &state.runner, id, repos).await?)
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
    repos: Option<MemoryRepos>,
    hidden: bool,
) -> CommandResult<()> {
    hide(&state, id, project, repos, hidden).await
}

async fn hide(
    state: &AppState,
    id: i64,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    hidden: bool,
) -> CommandResult<()> {
    let project = project_root(state, project).await?;
    let scripts = MemoryCopyKind::Scripts;

    Ok(hide_entry(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        scripts,
        id,
        Repos::of(project.as_deref(), repos.as_ref())?,
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
pub async fn memory_script_delete(
    state: State<'_, AppState>,
    id: i64,
    project: Option<String>,
    repos: Option<MemoryRepos>,
) -> CommandResult<()> {
    let project = project_root(&state, project).await?;
    let repos = Repos::of(project.as_deref(), repos.as_ref())?;
    state
        .memory_scripts
        .delete(&state.env, &state.runner, id, repos)
        .await?;

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
/// app, `forbidden` for a project that is not open or recent. A new script may pass `place`
/// (`remote` or `local`) while its scope is routed to "Remote + this computer": the answer says
/// where it went (`place`, null when the server does not say). A script saved to the team database
/// answers `script: null` (it is read-only here). `invalidInput` for a `place` with `replace`,
/// `unsupported` for one when the server lacks `save-place` (see
/// `MemoryRemote.savePlaceSupported`), `process` with the server's redacted line when the team
/// database cannot be reached: nothing is saved on either side.
#[tauri::command]
pub async fn memory_script_save(
    state: State<'_, AppState>,
    project: Option<String>,
    draft: ScriptDraft,
    replace: bool,
    base: Option<String>,
    place: Option<MemorySavePlace>,
) -> CommandResult<ScriptSaved> {
    save(&state, project, &draft, replace, base.as_deref(), place).await
}

async fn save(
    state: &AppState,
    project: Option<String>,
    draft: &ScriptDraft,
    replace: bool,
    base: Option<&str>,
    place: Option<MemorySavePlace>,
) -> CommandResult<ScriptSaved> {
    let project = project_root(state, project).await?;
    let route = state.save_route().await;

    Ok(state
        .memory_scripts
        .save(
            &state.env,
            &state.runner,
            route,
            project.as_deref(),
            draft,
            replace,
            base,
            place,
        )
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
    repos: Option<MemoryRepos>,
    scope: Option<ScriptScope>,
) -> CommandResult<MemoryDeleteAll> {
    delete_all(&state, project, repos, scope).await
}

async fn delete_all(
    state: &AppState,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    scope: Option<ScriptScope>,
) -> CommandResult<MemoryDeleteAll> {
    let project = project_root(state, project).await?;
    let repos = Repos::of(project.as_deref(), repos.as_ref())?;

    Ok(state
        .memory_scripts
        .delete_all(&state.env, &state.runner, repos, scope)
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
    repos: Option<MemoryRepos>,
) -> CommandResult<()> {
    trust(&state, items, project, repos).await
}

async fn trust(
    state: &AppState,
    items: Vec<ScriptTrustItem>,
    project: Option<String>,
    repos: Option<MemoryRepos>,
) -> CommandResult<()> {
    let project = project_root(state, project).await?;
    let repos = Repos::of(project.as_deref(), repos.as_ref())?;

    Ok(trust_scripts(&state.env, &state.runner, &items, repos).await?)
}

async fn list(
    state: &AppState,
    project: Option<String>,
    repos: Option<MemoryRepos>,
    query: &ScriptsQuery,
) -> CommandResult<MemoryScriptsPage> {
    query.author.as_ref().map(AuthorFilter::checked).transpose()?;

    let project = project_root(state, project).await?;

    // A server without `notes-author-filter` would refuse the author options and the `addedBy`
    // sort, as usage errors.
    let query = if query.needs_features() {
        query.for_server(state.memory_features.support(&state.env, &state.runner).await)
    } else {
        query.clone()
    };

    Ok(harness_business::ecosystem::memory_list_cache::list_scripts_cached(
        &state.config_dir,
        &state.env,
        &state.memory_scripts,
        &state.runner,
        Repos::of(project.as_deref(), repos.as_ref())?,
        &query,
    )
    .await)
}

/// The repositories that have notes (`kind` notes) or scripts, by name, with their counts and how
/// many global ones there are (`harness-memory notes|scripts repos --json`). Never fails for a
/// missing or old server: see the page's `availability`.
#[tauri::command]
pub async fn memory_repos_list(state: State<'_, AppState>, kind: MemoryCopyKind) -> CommandResult<MemoryReposPage> {
    Ok(list_repos(&state.env, &state.memory_scripts, &state.runner, kind).await)
}

/// The canonical root of `project` (`forbidden` unless active or recent); `None` for none.
pub(super) async fn project_root(state: &AppState, project: Option<String>) -> CommandResult<Option<PathBuf>> {
    match project {
        Some(project) => Ok(Some(authorized_dir(state, project).await?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
