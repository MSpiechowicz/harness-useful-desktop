//! The recall the app runs for the Harness Memory step (the `memory-first` family hook):
//! `harness-memory recall --hook [--project P]`, see [`crate::hooks::recall`].
//!
//! The same run as [`super::knowledge_recall`] against the memory server (limits and results are
//! [`super::recall_lookup`]'s), gated on `recall` in `harness-memory features`: a server without it
//! is [`Lookup::NotRun`]. The block it prints is the `<harness-memory-recall>` block of
//! [`crate::pi_family::sessions::recall_block`]: output of another shape (text around it, two
//! blocks, a cut block, a section that is no note or script) is [`Lookup::Failed`].

use super::catalog::MEMORY_SIDECAR;
use super::memory_features::MemoryFeatures;
use super::recall_lookup::{RecallRun, within_timeout};
use super::sidecar_embedder::sidecar_feature_server;
use crate::env::Env;
use crate::hooks::Lookup;
use crate::pi_family::sessions::recall_block::strip_recall;
use crate::process::ProcessRunner;

/// The output kept of a recall (the server prints at most 2304 bytes).
pub const MAX_MEMORY_RECALL_OUTPUT_BYTES: usize = 4 * 1024;

/// The notes (and saved scripts) Harness Memory recalls for `prompt` in `project` (the chat's
/// project root, none without one). The whole of it, the features probe included, gets
/// `RECALL_TIMEOUT`; a recall that did not finish in time is [`Lookup::Failed`].
pub async fn memory_recall(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &MemoryFeatures,
    prompt: &str,
    project: Option<&str>,
) -> Lookup {
    within_timeout(recall(env, runner, features, prompt, project)).await
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

    let run = RecallRun {
        sidecar: &MEMORY_SIDECAR,
        output_bytes: MAX_MEMORY_RECALL_OUTPUT_BYTES,
        is_block,
    };

    run.run(env, runner, &server, prompt, project).await
}

/// True when `text` is exactly one well-formed recall block within [`RECALL_BUDGET`] (so the step
/// never cuts it): [`strip_recall`] takes all of it.
fn is_block(text: &str) -> bool {
    strip_recall(text).is_empty()
}
