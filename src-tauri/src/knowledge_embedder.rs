//! The embedder behind vector search of Harness Knowledge: its own process, socket and model, kept
//! running while project knowledge and the `knowledgeVectorSearch` pref are on and the installed
//! Harness Knowledge lists `embedder` (see [`crate::sidecar_embedder`], run for [`Knowledge`]), from
//! launch, after the server's install, after each switch of either pref and before an import
//! ([`ready_for_import`]).

use std::time::Duration;

use harness_business::model::EmbedderPhase;

use crate::product::Knowledge;
use crate::sidecar_embedder;
use crate::state::AppState;

/// The most an import waits for the embedder to load its model after it was started.
const READY_WAIT: Duration = Duration::from_secs(20);

/// Makes sure the embedder runs before an import when vector search is on, and waits for it to
/// load its model (at most [`READY_WAIT`]), so the chunks of the files get their vectors at once.
/// A model that still downloads is not waited for: those chunks wait for a reindex
/// (`knowledge_reindex`). Nothing happens while vector search is off.
pub(crate) async fn ready_for_import(state: &AppState) {
    if !sidecar_embedder::wanted::<Knowledge>(state).await {
        return;
    }

    sidecar_embedder::reconcile::<Knowledge>(state).await;

    let mut updates = state.knowledge_embedder.subscribe();
    let loading = |phase: EmbedderPhase| {
        matches!(
            phase,
            EmbedderPhase::Starting | EmbedderPhase::Verifying | EmbedderPhase::Loading
        )
    };

    let settled = async {
        loop {
            if !loading(updates.borrow_and_update().phase) || updates.changed().await.is_err() {
                break;
            }
        }
    };

    if tokio::time::timeout(READY_WAIT, settled).await.is_err() {
        tracing::info!("the knowledge embedder is still loading, the import starts without waiting");
    }
}
