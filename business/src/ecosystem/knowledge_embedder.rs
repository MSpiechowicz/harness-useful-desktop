//! The local embedder behind vector search of Harness Knowledge: `harness-knowledge embedder run`
//! is its own process per computer (its own socket and model), and the app keeps it running (see
//! [`super::sidecar_embedder_supervisor`], which takes [`KnowledgeFeatures`] as the features of the
//! server it keeps).
//!
//! What the app understands of it (the events, the state they add up to, the short commands) is
//! [`sidecar_embedder`](super::sidecar_embedder)'s, run for [`KNOWLEDGE_SIDECAR`]. This file adds
//! the commands that name the knowledge server, and the one that embeds what an import left
//! without a vector (`docs reindex`).

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_cli::{CliCall, count_of, run_json, words};
use super::sidecar_cli::LIST_OUTPUT_BYTES;
use super::sidecar_embedder::{
    EmbedderStatus, remove_sidecar_embedder_model, set_sidecar_embedder_enabled, sidecar_embedder_status,
};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::KnowledgeReindexed;
use crate::process::ProcessRunner;

/// How long a reindex may take (30 minutes: a large library embeds at a few chunks a second).
pub const REINDEX_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// `embedder enable` or `embedder disable` (see [`set_sidecar_embedder_enabled`]).
pub async fn set_knowledge_embedder_enabled(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    enabled: bool,
) -> Result<()> {
    set_sidecar_embedder_enabled(&KNOWLEDGE_SIDECAR, env, runner, server, enabled).await
}

/// `embedder status --json` (see [`sidecar_embedder_status`]).
pub async fn knowledge_embedder_status(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<EmbedderStatus> {
    sidecar_embedder_status(&KNOWLEDGE_SIDECAR, env, runner, server).await
}

/// `embedder remove-model` (see [`remove_sidecar_embedder_model`]).
pub async fn remove_knowledge_embedder_model(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<()> {
    remove_sidecar_embedder_model(&KNOWLEDGE_SIDECAR, env, runner, server).await
}

/// `docs reindex --json (ID... | --all)`: gives the chunks that have no vector one (a file imported
/// while no embedder was ready). `ids` empty means every document. Needs the embedder to be
/// running: the server fails (`process`) otherwise.
pub async fn reindex_knowledge(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    ids: &[i64],
) -> Result<KnowledgeReindexed> {
    if ids.len() > 1000 || ids.iter().any(|id| *id < 1) {
        return Err(BusinessError::InvalidInput(
            "reindex up to 1000 documents at a time".to_owned(),
        ));
    }

    let mut args = words(&["docs", "reindex", "--json"]);

    if ids.is_empty() {
        args.push("--all".into());
    } else {
        args.extend(ids.iter().map(|id| id.to_string().into()));
    }

    let call = CliCall {
        name: "docs reindex",
        args,
        timeout: REINDEX_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    };

    let data = run_json(env, runner, server, call).await?;

    Ok(KnowledgeReindexed {
        documents: count_of(data.get("documents")),
        embedded: count_of(data.get("embedded")),
        failed: data
            .get("failed")
            .and_then(Value::as_array)
            .map_or(0, |failed| u32::try_from(failed.len()).unwrap_or(u32::MAX)),
        cancelled: data.get("cancelled").and_then(Value::as_bool).unwrap_or(false),
        model: data.get("model").and_then(Value::as_str).map(str::to_owned),
    })
}
