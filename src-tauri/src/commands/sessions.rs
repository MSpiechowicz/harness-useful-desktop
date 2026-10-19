//! Saved-session commands: a project's sessions and their folder, one session's branch tree,
//! active-path transcript and edited files, and deleting a session.
//!
//! The project must be the active project or a recent one (or the routines workspace, where
//! global routines keep their sessions). A session a routine run started carries its routine
//! (`routine`, the "Routine" tag). A session is named only by its
//! harness and id; the harness looks the id up in that project's session folder, so the webview
//! can never name another file. The files are read in a blocking task.
//!
//! Each global routine runs in its own folder of the routines workspace. Named by the workspace
//! itself, the session commands cover all of them: the list holds the sessions of the workspace
//! and of every routine's folder, newest first, and a session is looked up in the folder that
//! holds it ([`session_dir`]).
//!
//! A harness without session files (OpenCode) lists, reads and deletes its sessions through its
//! CLI ([`Harness::cli_sessions`]): its detected binary runs in the blocking task, and a harness
//! that is not installed is `notFound`. Such sessions have no branch tree.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::{CliSessionCtx, CliSessions, Harness};
use harness_business::model::{HarnessId, SessionFolder, SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use harness_business::process::StdRunner;
use tauri::State;

use super::harness::installed_binary;
use super::{authorized_dir, blocking};
use crate::error::CommandResult;
use crate::routines::RoutineRuntime;
use crate::state::AppState;

/// The saved sessions of `harness` in `project`, newest first.
#[tauri::command]
pub async fn sessions_list(
    state: State<'_, AppState>,
    harness: HarnessId,
    project: String,
) -> CommandResult<Vec<SessionSummary>> {
    list(&state, harness, project).await
}

/// The folder `harness` keeps the sessions of `project` in; `configured` when the project's own
/// settings chose it.
#[tauri::command]
pub async fn sessions_folder(
    state: State<'_, AppState>,
    harness: HarnessId,
    project: String,
) -> CommandResult<SessionFolder> {
    folder(&state, harness, project).await
}

/// The branch tree of `session`, with its active path marked.
#[tauri::command]
pub async fn session_tree(
    state: State<'_, AppState>,
    session: SessionRef,
    project: String,
) -> CommandResult<SessionTree> {
    tree(&state, session, project).await
}

/// One page of the active-path transcript of `session`, oldest first: `cursor` null starts at
/// the root, `nextCursor` continues; at most 200 messages per page.
#[tauri::command]
pub async fn session_messages(
    state: State<'_, AppState>,
    session: SessionRef,
    project: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<SessionMessagesPage> {
    messages(&state, session, project, cursor, limit).await
}

/// The project-relative files `session` edited, sorted.
#[tauri::command]
pub async fn session_files_touched(
    state: State<'_, AppState>,
    session: SessionRef,
    project: String,
) -> CommandResult<Vec<String>> {
    files_touched(&state, session, project).await
}

/// Deletes `session` with its companion folder and backups. With `emptyCopyOnly` only an empty
/// copy is deleted (`invalidInput` otherwise). A session an agent holds (open in a chat, starting
/// or stopping) is `forbidden`: close the chat first.
#[tauri::command]
pub async fn session_delete(
    state: State<'_, AppState>,
    session: SessionRef,
    project: String,
    empty_copy_only: bool,
) -> CommandResult<()> {
    delete(&state, session, project, empty_copy_only).await
}

pub(crate) async fn list(state: &AppState, harness: HarnessId, project: String) -> CommandResult<Vec<SessionSummary>> {
    let dir = authorized_dir(state, project).await?;
    let folders = folders_of(state, harness, dir).await?;
    let several = folders.len() > 1;
    let mut summaries = Vec::new();

    for folder in folders {
        let cache = state.sessions.clone();

        summaries.extend(
            with_sessions(state, harness, folder, move |sessions| match sessions {
                Sessions::Files(harness, env, project) => harness.list_sessions(env, project, &cache),
                Sessions::Cli(cli, ctx) => cli.list(&ctx),
            })
            .await?,
        );
    }

    if several {
        summaries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    }

    let routines = state.routines.clone();

    blocking(move || {
        tag_routines(&routines, &mut summaries);
        Ok(summaries)
    })
    .await
}

/// The folders whose sessions of `harness` `dir` (authorized) stands for: the routines workspace
/// and each global routine's folder in it that holds a session folder of `harness`, or `dir`
/// alone. Routines run only harnesses with session files, so a harness listed through its CLI
/// gets the workspace alone, and a folder without sessions costs one look, not a listing.
async fn folders_of(state: &AppState, harness: HarnessId, dir: PathBuf) -> CommandResult<Vec<PathBuf>> {
    let routines = state.routines.clone();
    let registry = state.registry.clone();
    let env = state.env.clone();

    blocking(move || {
        let found = registry.get(harness).filter(|found| found.cli_sessions().is_none());

        let Some(found) = found.filter(|_| routines.workspace().as_ref() == Some(&dir)) else {
            return Ok(vec![dir]);
        };

        let holding = routines
            .routine_workspaces()
            .into_iter()
            .filter(|folder| found.sessions_dir(&env, folder).is_dir());

        Ok(std::iter::once(dir).chain(holding).collect())
    })
    .await
}

/// The authorized folder `project` names, for `session`: in the routines workspace, the global
/// routine's folder that holds the session (the workspace itself when none does).
pub(crate) async fn session_dir(state: &AppState, project: String, session: &SessionRef) -> CommandResult<PathBuf> {
    let dir = authorized_dir(state, project).await?;
    let folders = folders_of(state, session.harness, dir.clone()).await?;

    if folders.len() == 1 {
        return Ok(dir);
    }

    let env = state.env.clone();
    let registry = state.registry.clone();
    let session = session.clone();

    blocking(move || {
        let Some(harness) = registry.get(session.harness) else {
            return Ok(dir);
        };

        let holder = folders
            .into_iter()
            .find(|folder| harness.resolve_session(&env, folder, &session).is_ok());

        Ok(holder.unwrap_or(dir))
    })
    .await
}

/// Fills in the routine of each session a routine run started.
fn tag_routines(routines: &RoutineRuntime, summaries: &mut [SessionSummary]) {
    let tags = routines.session_tags();

    if tags.is_empty() {
        return;
    }

    for summary in summaries {
        summary.routine = tags.get(summary.session.harness, &summary.session.id).cloned();
    }
}

async fn folder(state: &AppState, harness: HarnessId, project: String) -> CommandResult<SessionFolder> {
    let project = authorized_dir(state, project).await?;

    with_store(state, harness, project, |harness, env, project| {
        Ok(harness.session_folder(env, project))
    })
    .await
}

async fn tree(state: &AppState, session: SessionRef, project: String) -> CommandResult<SessionTree> {
    let project = session_dir(state, project, &session).await?;

    with_store(state, session.harness, project, move |harness, env, project| {
        harness.session_tree(env, project, &session)
    })
    .await
}

async fn messages(
    state: &AppState,
    session: SessionRef,
    project: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<SessionMessagesPage> {
    let project = session_dir(state, project, &session).await?;

    with_sessions(state, session.harness, project, move |sessions| match sessions {
        Sessions::Files(harness, env, project) => {
            harness.session_messages(env, project, &session, cursor.as_deref(), limit)
        }
        Sessions::Cli(cli, ctx) => cli.messages(&ctx, &session, cursor.as_deref(), limit),
    })
    .await
}

async fn files_touched(state: &AppState, session: SessionRef, project: String) -> CommandResult<Vec<String>> {
    let project = session_dir(state, project, &session).await?;

    with_sessions(state, session.harness, project, move |sessions| match sessions {
        Sessions::Files(harness, env, project) => harness.session_files_touched(env, project, &session),
        Sessions::Cli(cli, ctx) => cli.files_touched(&ctx, &session),
    })
    .await
}

async fn delete(state: &AppState, session: SessionRef, project: String, empty_copy_only: bool) -> CommandResult<()> {
    let agents = state.agents.clone();

    // Held until the files are gone, so no chat can resume the session meanwhile.
    let Some(_claim) = agents.manager().claim_session(&session.id) else {
        return Err(BusinessError::Forbidden("close the chat first".to_owned()).into());
    };

    let project = session_dir(state, project, &session).await?;

    with_sessions(state, session.harness, project, move |sessions| match sessions {
        Sessions::Files(harness, env, project) => harness.delete_session(env, project, &session, empty_copy_only),
        // CLI sessions are never copies.
        Sessions::Cli(..) if empty_copy_only => Err(BusinessError::InvalidInput(format!(
            "session {} is not an empty copy",
            session.id
        ))),
        Sessions::Cli(cli, ctx) => cli.delete(&ctx, &session),
    })
    .await
}

/// Runs `read` with `harness` and the authorized canonical project directory `project` in a
/// blocking task.
async fn with_store<T, F>(state: &AppState, harness: HarnessId, project: PathBuf, read: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&dyn Harness, &Env, &Path) -> harness_business::Result<T> + Send + 'static,
{
    let env = state.env.clone();
    let registry = state.registry.clone();

    blocking(move || {
        let found = registry
            .get(harness)
            .ok_or_else(|| BusinessError::NotFound(format!("harness {harness:?}")))?;

        Ok(read(found, &env, &project)?)
    })
    .await
}

/// Where [`with_sessions`] finds a harness's saved sessions.
enum Sessions<'a> {
    /// Session files the harness reads: the harness, environment and canonical project.
    Files(&'a dyn Harness, &'a Env, &'a Path),
    /// The harness CLI, run for the project.
    Cli(&'a dyn CliSessions, CliSessionCtx<'a>),
}

/// Like [`with_store`], with the CLI sessions of a harness that has them. Their binary is detected
/// first (`notFound` when the harness is not installed).
async fn with_sessions<T, F>(state: &AppState, harness: HarnessId, project: PathBuf, read: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(Sessions<'_>) -> harness_business::Result<T> + Send + 'static,
{
    let env = state.env.clone();
    let registry = state.registry.clone();

    let uses_cli = registry
        .get(harness)
        .ok_or_else(|| BusinessError::NotFound(format!("harness {harness:?}")))?
        .cli_sessions()
        .is_some();

    // Detecting is async, so it happens before the blocking task.
    let binary = if uses_cli {
        Some(installed_binary(state, harness).await?)
    } else {
        None
    };

    blocking(move || {
        let found = registry
            .get(harness)
            .ok_or_else(|| BusinessError::NotFound(format!("harness {harness:?}")))?;

        let sessions = match (found.cli_sessions(), &binary) {
            (Some(cli), Some(binary)) => Sessions::Cli(
                cli,
                CliSessionCtx {
                    env: &env,
                    project: &project,
                    binary,
                    runner: &StdRunner,
                },
            ),
            _ => Sessions::Files(found, &env, &project),
        };

        Ok(read(sessions)?)
    })
    .await
}

#[cfg(test)]
mod tests;
