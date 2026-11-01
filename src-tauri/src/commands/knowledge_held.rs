//! The documents agents proposed through `knowledge_add` (`knowledge_held_list`,
//! `knowledge_held_get`, `knowledge_held_decide`) and the live counts of the view
//! (`knowledge_changes_listen`), through the installed `harness-knowledge held …` and `stats`.
//! See `harness_business::ecosystem::knowledge_held`.
//!
//! The review queue needs `held` in the server's features: `notFound` when Harness Knowledge is not
//! installed, `unsupported` when it predates it. A list never fails for that: see the page's
//! `availability`.

use harness_business::ecosystem::{
    HeldSort, ListWindow, approval_place, decide_knowledge_held, get_knowledge_held, knowledge_held_server,
    list_knowledge_held,
};
use harness_business::model::{
    KnowledgeDecided, KnowledgeDecision, KnowledgeHeldItem, KnowledgeHeldPage, KnowledgePlace, KnowledgePulse,
    KnowledgeScope, SortOrder,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::error::CommandResult;
use crate::state::AppState;

/// One page of the review queue, newest first, filtered by the search `query` (in the title or file
/// name) and starting at `offset`. `invalidInput` for a limit of 0 or above 200, or a query above 256 characters. Never
/// fails for a missing or old server: see the page's `availability`.
#[tauri::command]
pub async fn knowledge_held_list(
    state: State<'_, AppState>,
    offset: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
    scope: Option<KnowledgeScope>,
    sort: Option<String>,
    order: Option<SortOrder>,
) -> CommandResult<KnowledgeHeldPage> {
    let window = ListWindow::new(offset, limit, query, order)?;
    let scope = scope.unwrap_or(KnowledgeScope::All);

    Ok(list_knowledge_held(
        &state.env,
        &state.runner,
        &state.knowledge_features,
        &window,
        HeldSort::of_key(sort.as_deref()),
        &scope,
    )
    .await)
}

/// One proposal with all of its text, for the review. `notFound` when there is no such proposal.
#[tauri::command]
pub async fn knowledge_held_get(state: State<'_, AppState>, id: String) -> CommandResult<KnowledgeHeldItem> {
    let server = knowledge_held_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(get_knowledge_held(&state.env, &state.runner, &server, &id).await?)
}

/// Approves (imports) or rejects (throws away) a proposal, only the version whose `revision` is
/// given (what the review showed). With `place` an approval is kept there (when both places are in
/// use, the routing's default otherwise); the server refuses a place the routing does not allow. `invalidInput` for an id or revision that is not one. A
/// refusal is an answer (`notFound`, `changed`, `refused`), not an error.
#[tauri::command]
pub async fn knowledge_held_decide(
    state: State<'_, AppState>,
    id: String,
    decision: KnowledgeDecision,
    revision: Option<String>,
    place: Option<KnowledgePlace>,
) -> CommandResult<KnowledgeDecided> {
    let server = knowledge_held_server(&state.env, &state.runner, &state.knowledge_features).await?;
    let place = approval_place(&state.env, &state.runner, &state.knowledge_features, place).await;

    Ok(decide_knowledge_held(
        &state.env,
        &state.runner,
        &server,
        &id,
        decision,
        revision.as_deref(),
        place,
    )
    .await?)
}

/// Sends the database's counts (`documents`, `generation`, `held`) to `on_event` now and whenever
/// one changes, replacing the previous listener: the view reloads its list when the generation
/// moves and shows `held` on the review badge. Nothing is sent while project knowledge is off or
/// the server does not answer.
#[tauri::command]
pub async fn knowledge_changes_listen(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<KnowledgePulse>,
) -> CommandResult<()> {
    state.knowledge_pulse.listen(app, on_event);

    Ok(())
}

/// Tells that the Knowledge view is open (`watching`) or closed: the counts of
/// `knowledge_changes_listen` are read every 4 s while it is open and every 30 s otherwise.
#[tauri::command]
pub fn knowledge_changes_watch(state: State<'_, AppState>, watching: bool) -> CommandResult<()> {
    state.knowledge_pulse.set_watching(watching);

    Ok(())
}
