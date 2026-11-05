//! The team database of Harness Knowledge (`knowledge_remote_get`, `knowledge_remote_set`,
//! `knowledge_remote_test`), kept in the server's own `config.toml` as `[database] url` and
//! `token`. PostgreSQL, MongoDB and libSQL are supported (Amazon DocumentDB is not). The test runs
//! `harness-knowledge check --json` and says what the database lacks (`no-mongot`, `no-pgvector`,
//! `pgvector-no-privilege`, `pgvector-too-old`, `libsql-no-vector`, `no-fts5`, `index-building`,
//! `unreachable`, `auth`).
//!
//! The URL and the token the webview sends are secrets: they are validated, written to that file or
//! handed to `harness-knowledge check` through its environment, and never logged or returned. The
//! app itself does not connect to the database.

use harness_business::ecosystem::{
    KnowledgeRemoteChange, knowledge_remote_get as read_remote, knowledge_remote_set as write_remote,
    knowledge_remote_test as run_test,
};
use harness_business::model::{KnowledgeCheck, KnowledgeRemote, MemoryNotesRouting, MemoryRemoteKind};
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// What the config file holds: the redacted URL, whether a token is saved, whether the server has
/// an embedder, and what is wrong with the file, if anything.
#[tauri::command]
pub async fn knowledge_remote_get(state: State<'_, AppState>) -> CommandResult<KnowledgeRemote> {
    let env = state.env.clone();
    let support = state.knowledge_features.support(&state.env, &state.runner).await;

    blocking(move || Ok(read_remote(&env, support))).await
}

/// Saves the team database (`enabled`), or removes it from the config file. A null `url` or
/// `token` keeps the saved one, an empty `token` removes it. `invalidInput` for a URL or token
/// Harness Knowledge would refuse, and for a file the app must not overwrite (see `problem` in the
/// result of `knowledge_remote_get`). `unsupported` for Amazon DocumentDB, also as a MongoDB URL
/// at its hosts. `repository_documents` and `global_documents` (null keeps the saved ones) say where
/// documents are kept (`remote`, `local`, `remoteLocal`), `unsupported` while the installed server
/// lacks `routing`.
#[tauri::command]
pub async fn knowledge_remote_set(
    state: State<'_, AppState>,
    enabled: bool,
    kind: Option<MemoryRemoteKind>,
    url: Option<String>,
    token: Option<String>,
    repository_documents: Option<MemoryNotesRouting>,
    global_documents: Option<MemoryNotesRouting>,
) -> CommandResult<KnowledgeRemote> {
    let env = state.env.clone();
    let support = state.knowledge_features.support(&state.env, &state.runner).await;
    let change = KnowledgeRemoteChange {
        enabled,
        kind,
        url,
        token,
        repository_documents,
        global_documents,
    };

    blocking(move || Ok(write_remote(&env, change, support)?)).await
}

/// Runs `harness-knowledge check --json` (20 s limit) against the given URL and token, or the
/// saved ones where they are null. `ok` false with `message` when the server is not installed, the
/// input is not valid, or the check could not run. Otherwise `problem` names what the database
/// lacks, and `capabilities` what works.
#[tauri::command]
pub async fn knowledge_remote_test(
    state: State<'_, AppState>,
    kind: MemoryRemoteKind,
    url: Option<String>,
    token: Option<String>,
) -> CommandResult<KnowledgeCheck> {
    let check = run_test(&state.env, &state.runner, kind, url, token).await;

    // The server took note of what the test found, so a banner on screen follows it at once.
    state.knowledge_pulse.wake();

    Ok(check)
}
