//! Saved-session commands: a project's sessions, and one session's branch tree, active-path
//! transcript and edited files.
//!
//! The project must be the active project or a recent one. A session is named only by its
//! harness and id; the harness looks the id up in that project's session folder, so the webview
//! can never name another file. The files are read in a blocking task.

use std::path::Path;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::model::{HarnessId, SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use tauri::State;

use super::{authorized_dir, blocking};
use crate::error::CommandResult;
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

async fn list(state: &AppState, harness: HarnessId, project: String) -> CommandResult<Vec<SessionSummary>> {
    let cache = state.sessions.clone();

    with_store(state, harness, project, move |harness, env, project| {
        harness.list_sessions(env, project, &cache)
    })
    .await
}

async fn tree(state: &AppState, session: SessionRef, project: String) -> CommandResult<SessionTree> {
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
    with_store(state, session.harness, project, move |harness, env, project| {
        harness.session_messages(env, project, &session, cursor.as_deref(), limit)
    })
    .await
}

async fn files_touched(state: &AppState, session: SessionRef, project: String) -> CommandResult<Vec<String>> {
    with_store(state, session.harness, project, move |harness, env, project| {
        harness.session_files_touched(env, project, &session)
    })
    .await
}

/// Authorizes `project` and runs `read` with `harness` and the canonical project directory in a
/// blocking task.
async fn with_store<T, F>(state: &AppState, harness: HarnessId, project: String, read: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&dyn Harness, &Env, &Path) -> harness_business::Result<T> + Send + 'static,
{
    let project = authorized_dir(state, project).await?;
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use harness_business::model::IpcErrorCode;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, state_with_project};

    /// The main OMP fixture session: two branch points, edits inside and outside the project.
    const MAIN_ID: &str = "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";
    const MAIN_FILE: &str = "2026-10-09T10-00-00-000Z_0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57.jsonl";

    struct Setup {
        state: AppState,
        project: PathBuf,
        stranger: PathBuf,
        _dir: TempDir,
    }

    /// An opened project with the main OMP fixture session, and an unknown project next to it
    /// that has a session of its own.
    fn setup() -> Setup {
        let dir = TempDir::new("sessions");
        let home = dir.path().join("home");
        let project = home.join("demo");
        let stranger = home.join("stranger");
        std::fs::create_dir_all(&stranger).expect("create stranger");

        let state = state_with_project(&dir, Env::for_tests(&home), &project);

        copy_fixture(&state, &project, MAIN_FILE);
        copy_fixture(
            &state,
            &stranger,
            "2026-10-07T15-30-12-000Z_01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3.jsonl",
        );

        Setup {
            state,
            project,
            stranger,
            _dir: dir,
        }
    }

    fn copy_fixture(state: &AppState, project: &Path, file: &str) {
        let omp = state.registry.get(HarnessId::Omp).expect("OMP is registered");
        let sessions = omp.sessions_dir(&state.env, project);
        std::fs::create_dir_all(&sessions).expect("create sessions dir");

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../business/tests/fixtures/sessions/omp")
            .join(file);
        std::fs::copy(fixture, sessions.join(file)).expect("copy fixture");
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn omp(id: &str) -> SessionRef {
        SessionRef {
            harness: HarnessId::Omp,
            id: id.to_owned(),
        }
    }

    #[test]
    fn a_projects_sessions_are_listed_and_read() {
        let setup = setup();
        let (state, project) = (&setup.state, text(&setup.project));

        let listed = block_on(list(state, HarnessId::Omp, project.clone())).expect("list");
        let ids: Vec<&str> = listed.iter().map(|summary| summary.session.id.as_str()).collect();
        assert_eq!(ids, [MAIN_ID]);

        let tree = block_on(tree(state, omp(MAIN_ID), project.clone())).expect("tree");
        assert_eq!(tree.branch_count, 2);
        assert!(tree.nodes.iter().any(|node| node.on_active_path));

        let first = block_on(messages(state, omp(MAIN_ID), project.clone(), None, 2)).expect("first page");
        assert_eq!(first.messages.len(), 2);
        assert!(first.total > 2);

        let cursor = first.next_cursor.clone().expect("more pages");
        let second = block_on(messages(state, omp(MAIN_ID), project.clone(), Some(cursor), 200)).expect("next page");
        assert_eq!(first.messages.len() + second.messages.len(), first.total as usize);
        assert_eq!(second.next_cursor, None);

        let touched = block_on(files_touched(state, omp(MAIN_ID), project)).expect("files touched");
        assert!(!touched.is_empty());
        assert!(
            touched
                .iter()
                .all(|path| !path.starts_with("..") && !path.starts_with('/'))
        );
    }

    #[test]
    fn unknown_projects_are_refused() {
        let setup = setup();
        let (state, stranger) = (&setup.state, text(&setup.stranger));
        let id = "01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3";

        let errors = [
            block_on(list(state, HarnessId::Omp, stranger.clone())).map(drop),
            block_on(tree(state, omp(id), stranger.clone())).map(drop),
            block_on(messages(state, omp(id), stranger.clone(), None, 10)).map(drop),
            block_on(files_touched(state, omp(id), stranger)).map(drop),
        ];

        for error in errors {
            assert_eq!(error.expect_err("unknown project").code(), IpcErrorCode::Forbidden);
        }
    }

    #[test]
    fn sessions_are_looked_up_only_in_the_projects_folder() {
        let setup = setup();
        let (state, project) = (&setup.state, text(&setup.project));

        // The stranger's session exists, but not in this project's folder.
        let foreign = block_on(tree(
            state,
            omp("01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3"),
            project.clone(),
        ));
        assert_eq!(foreign.expect_err("foreign session").code(), IpcErrorCode::NotFound);

        for id in ["../stranger", "/etc/passwd", MAIN_FILE, ""] {
            let error = block_on(tree(state, omp(id), project.clone())).expect_err(id);
            assert!(
                matches!(error.code(), IpcErrorCode::InvalidInput | IpcErrorCode::NotFound),
                "{id}: {:?}",
                error.code()
            );
        }

        let other_harness = SessionRef {
            harness: HarnessId::Pi,
            id: MAIN_ID.to_owned(),
        };
        let error = block_on(tree(state, other_harness, project)).expect_err("PI has no such session");
        assert_eq!(error.code(), IpcErrorCode::NotFound);
    }
}
