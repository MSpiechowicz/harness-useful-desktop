//! Vector search of Harness Knowledge (`knowledge_set_vector_search`, `knowledge_embedder_watch`,
//! `knowledge_embedder_remove_model`, `knowledge_reindex`), through the installed
//! `harness-knowledge embedder …` and `docs reindex`. Each needs `embedder` in the server's
//! features: `notFound` when Harness Knowledge is not installed, `unsupported` when it predates it.
//! See `harness_business::ecosystem::knowledge_embedder` and `crate::sidecar_embedder`.

use harness_business::ecosystem::{KnowledgeSupport, reindex_knowledge};
use harness_business::model::{KnowledgeId, KnowledgeReindexed, MemoryEmbedderState, MemoryVectorSwitchResult};
use tauri::State;
use tauri::ipc::Channel;

use crate::error::CommandResult;
use crate::product::Knowledge;
use crate::sidecar_embedder;
use crate::state::AppState;

/// Turns vector search on or off: runs `embedder enable|disable`, stores the `knowledgeVectorSearch`
/// pref, then starts or stops the embedder (it starts only while project knowledge is on too). The
/// answer holds the stored pref and the embedder's state right then (follow it with
/// `knowledge_embedder_watch`). Nothing changes when the server refuses: `unsupported` for a
/// Harness Knowledge without `embedder`, `notFound` when it is not installed, `process` with its
/// line. A pref that cannot be stored undoes the server's switch and answers the error. Turning it
/// off also works on a server without `embedder`.
#[tauri::command]
pub async fn knowledge_set_vector_search(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<MemoryVectorSwitchResult> {
    sidecar_embedder::set_vector_search::<Knowledge>(&state, enabled).await
}

/// Sends the embedder's state (`phase`, `progress`, `model`, `error`) to `on_event` now and on
/// every change, replacing the previous listener.
#[tauri::command]
pub async fn knowledge_embedder_watch(
    state: State<'_, AppState>,
    on_event: Channel<MemoryEmbedderState>,
) -> CommandResult<()> {
    sidecar_embedder::watch::<Knowledge>(&state, on_event);

    Ok(())
}

/// Stops the embedder if it runs, then deletes the model files and the vectors (60 s). Answers the
/// embedder's state (`off`, `modelPresent` false). Turn vector search off first to keep the model
/// from being fetched again at the next launch.
#[tauri::command]
pub async fn knowledge_embedder_remove_model(state: State<'_, AppState>) -> CommandResult<MemoryEmbedderState> {
    sidecar_embedder::remove_model::<Knowledge>(&state).await
}

/// Gives the chunks that have no vector one (files imported while the embedder was not ready), for
/// the documents `ids` or every document when none are given (30 minutes at most). `process` with
/// the server's line while no embedder runs.
#[tauri::command]
pub async fn knowledge_reindex(
    state: State<'_, AppState>,
    ids: Option<Vec<KnowledgeId>>,
) -> CommandResult<KnowledgeReindexed> {
    let server = state
        .knowledge_features
        .feature_server(&state.env, &state.runner, KnowledgeSupport::embedder, "vector search")
        .await?;

    Ok(reindex_knowledge(&state.env, &state.runner, &server, &ids.unwrap_or_default()).await?)
}
