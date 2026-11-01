//! The Knowledge view's documents (`knowledge_docs_list`, `knowledge_doc_get`,
//! `knowledge_doc_rename`, `knowledge_doc_links_set`, `knowledge_docs_delete`,
//! `knowledge_repos_list`, `knowledge_stats`) and the user's own search (`knowledge_search`),
//! through the installed `harness-knowledge docs …`, `repos`, `stats` and `search` (15 s each).
//! The app never opens the database. See `harness_business::ecosystem::knowledge_docs`.
//!
//! Each needs the feature of the installed server (`docs`, `search`): `notFound` when Harness
//! Knowledge is not installed, `unsupported` when it predates it. A list never fails for that:
//! see the page's `availability`.

use harness_business::ecosystem::{
    ListWindow, delete_docs, docs_server, get_doc, list_docs, list_knowledge_repos, read_stats, rename_doc,
    search_knowledge, set_links,
};
use harness_business::model::{
    KnowledgeDeleted, KnowledgeDetail, KnowledgeHit, KnowledgeLink, KnowledgeListFilter, KnowledgePage,
    KnowledgeReposPage, KnowledgeSearchScope, KnowledgeStats,
};
use tauri::State;

use super::memory_scripts::project_root;
use crate::error::CommandResult;
use crate::state::AppState;

/// One page of documents, filtered by `filter` (repository scope, format, sort and its direction)
/// and the search `query` (the title contains it or the text has all its words). The page starts at
/// `offset`, a multiple of `limit` (1 to 200, 50 when not given). `invalidInput` for a limit of 0
/// or above 200, an offset that is not a multiple of the limit, a query above 200 characters, a
/// format name or repository key that is not one.
#[tauri::command]
pub async fn knowledge_docs_list(
    state: State<'_, AppState>,
    filter: Option<KnowledgeListFilter>,
    offset: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
) -> CommandResult<KnowledgePage> {
    let filter = filter.unwrap_or_default();
    let window = ListWindow::new(offset, limit, query, filter.order)?;

    Ok(list_docs(&state.env, &state.runner, &state.knowledge_features, &filter, &window).await?)
}

/// A document with its outline, and the chunk `chunk_id` of it with its text when given.
/// `process` with the server's line for an id that does not exist.
#[tauri::command]
pub async fn knowledge_doc_get(
    state: State<'_, AppState>,
    id: i64,
    chunk_id: Option<i64>,
) -> CommandResult<KnowledgeDetail> {
    let server = docs_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(get_doc(&state.env, &state.runner, &server, id, chunk_id).await?)
}

/// Renames a document (1 to 300 characters) and answers the title as stored.
#[tauri::command]
pub async fn knowledge_doc_rename(state: State<'_, AppState>, id: i64, title: String) -> CommandResult<String> {
    let server = docs_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(rename_doc(&state.env, &state.runner, &server, id, &title).await?)
}

/// Replaces the links of a document (at most 32 repositories with at most 32 path globs each, none
/// makes it global) and answers the links as stored. `invalidInput` for a repository key that is
/// not one or a glob the server would refuse.
#[tauri::command]
pub async fn knowledge_doc_links_set(
    state: State<'_, AppState>,
    id: i64,
    links: Vec<KnowledgeLink>,
) -> CommandResult<Vec<KnowledgeLink>> {
    let server = docs_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(set_links(&state.env, &state.runner, &server, id, &links).await?)
}

/// Deletes 1 to 1000 documents with their chunks and index entries (ids that do not exist count
/// nothing).
#[tauri::command]
pub async fn knowledge_docs_delete(state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<KnowledgeDeleted> {
    let server = docs_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(delete_docs(&state.env, &state.runner, &server, &ids).await?)
}

/// The repositories that have documents and how many are global, for the Repository filter. Never
/// fails for a missing or old server: see the page's `availability`.
#[tauri::command]
pub async fn knowledge_repos_list(state: State<'_, AppState>) -> CommandResult<KnowledgeReposPage> {
    Ok(list_knowledge_repos(&state.env, &state.runner, &state.knowledge_features).await)
}

/// What the database holds: documents, chunks, how many have a vector, the size, the generation
/// and the model that embedded them.
#[tauri::command]
pub async fn knowledge_stats(state: State<'_, AppState>) -> CommandResult<KnowledgeStats> {
    let server = docs_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(read_stats(&state.env, &state.runner, &server).await?)
}

/// The passages that answer `query`, best first, at most `limit` (1 to 20, 10 when not given), as
/// the agents' search ranks them for the project `project` (active or recent, else the global
/// documents only), with the documents of this project's repository (`repo`, the default) or of
/// every repository (`all`). `unsupported` for a server without `search`, `invalidInput` for a
/// blank query or one above 256 characters, `forbidden` for a project that is not open or recent.
#[tauri::command]
pub async fn knowledge_search(
    state: State<'_, AppState>,
    query: String,
    project: Option<String>,
    scope: Option<KnowledgeSearchScope>,
    limit: Option<u32>,
) -> CommandResult<Vec<KnowledgeHit>> {
    let project = project_root(&state, project).await?;

    Ok(search_knowledge(
        &state.env,
        &state.runner,
        &state.knowledge_features,
        &query,
        project.as_deref(),
        scope.unwrap_or_default(),
        limit,
    )
    .await?)
}
