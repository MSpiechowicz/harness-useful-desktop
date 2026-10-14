//! Saved-session commands: a project's sessions and their folder, one session's branch tree,
//! active-path transcript and edited files, and deleting a session.
//!
//! The project must be the active project or a recent one. A session is named only by its
//! harness and id; the harness looks the id up in that project's session folder, so the webview
//! can never name another file. The files are read in a blocking task.

use std::path::Path;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::model::{HarnessId, SessionFolder, SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
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

async fn list(state: &AppState, harness: HarnessId, project: String) -> CommandResult<Vec<SessionSummary>> {
    let cache = state.sessions.clone();

    with_store(state, harness, project, move |harness, env, project| {
        harness.list_sessions(env, project, &cache)
    })
    .await
}

async fn folder(state: &AppState, harness: HarnessId, project: String) -> CommandResult<SessionFolder> {
    with_store(state, harness, project, |harness, env, project| {
        Ok(harness.session_folder(env, project))
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

async fn delete(state: &AppState, session: SessionRef, project: String, empty_copy_only: bool) -> CommandResult<()> {
    let agents = state.agents.clone();

    // Held until the files are gone, so no chat can resume the session meanwhile.
    let Some(_claim) = agents.manager().claim_session(&session.id) else {
        return Err(BusinessError::Forbidden("close the chat first".to_owned()).into());
    };

    with_store(state, session.harness, project, move |harness, env, project| {
        harness.delete_session(env, project, &session, empty_copy_only)
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
    use std::sync::Arc;

    use harness_business::agent::{AgentDeps, AgentManager, CollectingSink};
    use harness_business::model::{AgentStartRequest, IpcErrorCode};
    use harness_business::process::ResolvedBinary;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::agents::Agents;
    use crate::test_support::{ScenarioSpawner, TempDir, fake_agent, state_with_project};

    /// The main OMP fixture session: two branch points, edits inside and outside the project.
    const MAIN_ID: &str = "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";
    const MAIN_FILE: &str = "2026-10-09T10-00-00-000Z_0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57.jsonl";

    /// The session of the unknown project next to it.
    const STRANGER_ID: &str = "01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3";
    const STRANGER_FILE: &str = "2026-10-07T15-30-12-000Z_01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3.jsonl";

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
        copy_fixture(&state, &stranger, STRANGER_FILE);

        Setup {
            state,
            project,
            stranger,
            _dir: dir,
        }
    }

    fn sessions_dir(state: &AppState, project: &Path) -> PathBuf {
        let omp = state.registry.get(HarnessId::Omp).expect("OMP is registered");
        omp.sessions_dir(&state.env, project)
    }

    fn copy_fixture(state: &AppState, project: &Path, file: &str) {
        let sessions = sessions_dir(state, project);
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
        let id = STRANGER_ID;

        let errors = [
            block_on(list(state, HarnessId::Omp, stranger.clone())).map(drop),
            block_on(folder(state, HarnessId::Pi, stranger.clone())).map(drop),
            block_on(tree(state, omp(id), stranger.clone())).map(drop),
            block_on(messages(state, omp(id), stranger.clone(), None, 10)).map(drop),
            block_on(files_touched(state, omp(id), stranger.clone())).map(drop),
            block_on(delete(state, omp(id), stranger, false)),
        ];

        for error in errors {
            assert_eq!(error.expect_err("unknown project").code(), IpcErrorCode::Forbidden);
        }

        let kept = sessions_dir(state, &setup.stranger).join(STRANGER_FILE);
        assert!(kept.is_file(), "the refused delete removed nothing");
    }

    #[test]
    fn the_folder_is_configured_only_by_the_projects_own_setting() {
        let setup = setup();
        let (state, project) = (&setup.state, text(&setup.project));

        let omp = block_on(folder(state, HarnessId::Omp, project.clone())).expect("OMP folder");
        assert_eq!(omp.path, text(&sessions_dir(state, &setup.project)));
        assert!(!omp.configured);

        std::fs::create_dir_all(setup.project.join(".pi")).expect("create .pi");
        std::fs::write(setup.project.join(".pi/settings.json"), r#"{ "sessionDir": "chats" }"#)
            .expect("write settings");

        let pi = block_on(folder(state, HarnessId::Pi, project)).expect("PI folder");
        assert_eq!(pi.path, text(&setup.project.join("chats")));
        assert!(pi.configured);
    }

    #[test]
    fn a_deleted_session_leaves_the_list() {
        let setup = setup();
        let (state, project) = (&setup.state, text(&setup.project));
        let file = sessions_dir(state, &setup.project).join(MAIN_FILE);

        // Not an empty copy: refused, nothing removed.
        let refused = block_on(delete(state, omp(MAIN_ID), project.clone(), true)).expect_err("not a copy");
        assert_eq!(refused.code(), IpcErrorCode::InvalidInput);
        assert!(file.is_file());

        block_on(delete(state, omp(MAIN_ID), project.clone(), false)).expect("delete");
        assert!(!file.exists());

        let listed = block_on(list(state, HarnessId::Omp, project.clone())).expect("list");
        assert!(listed.is_empty(), "{listed:?}");

        let again = block_on(delete(state, omp(MAIN_ID), project, false)).expect_err("already deleted");
        assert_eq!(again.code(), IpcErrorCode::NotFound);
    }

    #[test]
    fn a_session_open_in_a_chat_is_not_deleted() {
        let Some(binary) = fake_agent() else {
            return;
        };
        let mut setup = setup();
        let spawner = ScenarioSpawner::new(&binary, "omp_basic.jsonl");
        setup.state.agents = Arc::new(Agents::with(AgentManager::new(), spawner.clone()));

        let (state, project) = (&setup.state, text(&setup.project));
        let file = sessions_dir(state, &setup.project).join(MAIN_FILE);

        let request = AgentStartRequest {
            harness: HarnessId::Omp,
            project: project.clone(),
            resume: Some(omp(MAIN_ID)),
            approval_override: None,
            trust_once: false,
            utility: false,
        };
        let deps = AgentDeps {
            harness: state.registry.get(HarnessId::Omp).expect("OMP is registered"),
            env: &state.env,
            spawner,
            binary: ResolvedBinary {
                path: binary.clone(),
                is_batch_script: false,
            },
        };
        let manager = state.agents.manager();
        let info = block_on(manager.start(request, deps, CollectingSink::new())).expect("the chat starts");

        let refused = block_on(delete(state, omp(MAIN_ID), project.clone(), false)).expect_err("open in a chat");
        assert_eq!(refused.code(), IpcErrorCode::Forbidden);
        assert!(file.is_file());

        block_on(manager.stop(&info.key)).expect("stop");

        block_on(delete(state, omp(MAIN_ID), project, false)).expect("delete once the chat closed");
        assert!(!file.exists());
    }

    #[test]
    fn sessions_are_looked_up_only_in_the_projects_folder() {
        let setup = setup();
        let (state, project) = (&setup.state, text(&setup.project));

        // The stranger's session exists, but not in this project's folder.
        let foreign = block_on(tree(state, omp(STRANGER_ID), project.clone()));
        assert_eq!(foreign.expect_err("foreign session").code(), IpcErrorCode::NotFound);

        let foreign_delete = block_on(delete(state, omp(STRANGER_ID), project.clone(), false));
        assert_eq!(
            foreign_delete.expect_err("foreign session").code(),
            IpcErrorCode::NotFound
        );

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
