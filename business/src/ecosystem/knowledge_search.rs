//! A search of Harness Knowledge for the user (`knowledge_search`): `harness-knowledge search
//! --json --query Q --scope repo|all --limit N [--project P]`, the same ranking the agents'
//! `knowledge_search` tool uses (keyword and vector, boosted for the repository).
//!
//! Needs `search` in the server's features. `--project` names the repository the search is for
//! (the server has none without it and then sees the global documents), `--scope all` sees every
//! repository's documents. The server prints a JSON array, best first, and `[]` for no match or no
//! database; each item is `{docId, chunkId, id, title, section, score, snippet, block}`. The
//! array is read strictly: an item without those fields fails the search (`protocol`). `block`
//! is the server's untrusted-text wrapping for agents and is not read. The query is logged
//! nowhere.

use std::path::{Path, PathBuf};

use super::knowledge_cli::{call, run_json, words};
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::sidecar_cli::{MAX_QUERY_CHARS, project_args};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeHit, KnowledgeSearchScope};
use crate::process::ProcessRunner;

/// The most passages one search returns.
pub const MAX_HITS: u32 = 20;

/// The passages a search returns when the caller does not say.
const DEFAULT_HITS: u32 = 10;

/// The installed server when it lists `search`.
pub async fn search_server(env: &Env, runner: &dyn ProcessRunner, features: &KnowledgeFeatures) -> Result<PathBuf> {
    features
        .feature_server(env, runner, KnowledgeSupport::search, "search")
        .await
}

/// The passages that answer `query` for `project`, best first, at most `limit` (1 to [`MAX_HITS`],
/// 10 when not given). `invalidInput` for a blank query or one above 256 characters.
pub async fn search_knowledge(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    query: &str,
    project: Option<&Path>,
    scope: KnowledgeSearchScope,
    limit: Option<u32>,
) -> Result<Vec<KnowledgeHit>> {
    let query = query.trim();

    if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS {
        return Err(BusinessError::InvalidInput(format!(
            "a search has 1 to {MAX_QUERY_CHARS} characters"
        )));
    }

    let limit = limit.unwrap_or(DEFAULT_HITS);

    if limit == 0 || limit > MAX_HITS {
        return Err(BusinessError::InvalidInput(format!(
            "a search returns 1 to {MAX_HITS} passages"
        )));
    }

    let server = search_server(env, runner, features).await?;

    let scope = match scope {
        KnowledgeSearchScope::Repo => "repo",
        KnowledgeSearchScope::All => "all",
    };

    let mut args = words(&["search", "--json", "--query"]);
    args.push(query.into());
    args.extend(["--scope".into(), scope.into()]);
    args.extend(["--limit".into(), limit.to_string().into()]);
    args.extend(project_args(project));

    let data = run_json(env, runner, &server, call("search", args)).await?;

    serde_json::from_value(data).map_err(|_| {
        BusinessError::Protocol("harness-knowledge search answered passages this app cannot read".to_owned())
    })
}
