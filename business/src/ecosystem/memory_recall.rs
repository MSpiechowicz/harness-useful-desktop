//! The recall the app runs for the Harness Memory step (the `memory-first` family hook):
//! `harness-memory recall --hook [--project P]`, see [`crate::hooks::recall`].
//!
//! The same run as [`super::knowledge_recall`] against the memory server (limits and results are
//! [`super::recall_lookup`]'s), gated on `recall` in `harness-memory features`: a server without it
//! is [`Lookup::NotRun`]. The block it prints is the `<harness-memory-recall>` block of
//! [`crate::pi_family::sessions::recall_block`]: output of another shape (text around it, two
//! blocks, a cut block, a section that is no note or script) is [`Lookup::Failed`].

use std::path::Path;

use super::catalog::MEMORY_SIDECAR;
use super::knowledge_cli::words;
use super::memory_features::MemoryFeatures;
use super::recall_lookup::{RECALL_TIMEOUT, clip, lookup_of};
use super::sidecar_cli::{CliCall, project_args, run_sidecar_cli};
use super::sidecar_embedder::sidecar_feature_server;
use crate::env::Env;
use crate::hooks::Lookup;
use crate::pi_family::sessions::recall_block::strip_recall;
use crate::process::ProcessRunner;

/// The output kept of a recall (the server prints at most 2304 bytes).
pub const MAX_MEMORY_RECALL_OUTPUT_BYTES: usize = 4 * 1024;

/// The notes (and saved scripts) Harness Memory recalls for `prompt` in `project` (the chat's
/// project root, none without one). The whole of it, the features probe included, gets
/// [`RECALL_TIMEOUT`]; a recall that did not finish in time is [`Lookup::Failed`].
pub async fn memory_recall(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    prompt: &str,
    project: Option<&str>,
) -> Lookup {
    match tokio::time::timeout(RECALL_TIMEOUT, recall(env, runner, features, prompt, project)).await {
        Ok(lookup) => lookup,
        Err(_elapsed) => Lookup::Failed,
    }
}

async fn recall(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    prompt: &str,
    project: Option<&str>,
) -> Lookup {
    let listed = features.lists_recall(env, runner);

    let Ok(server) = sidecar_feature_server(&MEMORY_SIDECAR, env, listed, "recall").await else {
        return Lookup::NotRun;
    };

    let mut args = words(&["recall", "--hook"]);
    args.extend(project_args(project.map(Path::new)));

    let call = CliCall {
        name: "recall",
        args,
        timeout: RECALL_TIMEOUT,
        output_bytes: MAX_MEMORY_RECALL_OUTPUT_BYTES,
        stdin: Some(clip(prompt).as_bytes().to_vec()),
    };

    lookup_of(
        run_sidecar_cli(&MEMORY_SIDECAR, env, runner, &server, call).await,
        is_block,
    )
}

/// True when `text` is exactly one well-formed recall block within [`RECALL_BUDGET`] (so the step
/// never cuts it): [`strip_recall`] takes all of it.
fn is_block(text: &str) -> bool {
    strip_recall(text).is_empty()
}
