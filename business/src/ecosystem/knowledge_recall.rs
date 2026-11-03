//! The recall the app runs for the Harness Knowledge step (the `knowledge-first` family hook):
//! `harness-knowledge recall --hook [--project P]`, see [`crate::hooks::recall`].
//!
//! Needs `recall` in the server's features, else it is [`Lookup::NotRun`]. The limits and what the
//! end of the run means are [`super::recall_lookup`]'s: it prints a block of at most three pointers
//! or nothing and exits 0 ([`Lookup::Found`], [`Lookup::Empty`]), anything else (a failure, a
//! timeout, output that is not exactly one recall block) is [`Lookup::Failed`].

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::recall_lookup::{RecallRun, within_timeout};
use crate::env::Env;
use crate::hooks::Lookup;
use crate::process::ProcessRunner;

/// The output kept of a recall (the server prints at most 1 KiB).
pub const MAX_RECALL_OUTPUT_BYTES: usize = 2 * 1024;

/// The tag the server wraps its pointers in.
const OPEN: &str = "<harness-knowledge-recall>";
const CLOSE: &str = "</harness-knowledge-recall>";

/// The pointers Harness Knowledge recalls for `prompt` in `project` (the chat's project root, none
/// without one). The whole of it, the features probe included, gets `RECALL_TIMEOUT` (plus the
/// process kill that follows a timeout of the server itself), so a slow probe never holds a message
/// back; a recall that did not finish in time is [`Lookup::Failed`].
pub async fn knowledge_recall(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    prompt: &str,
    project: Option<&str>,
) -> Lookup {
    within_timeout(recall(env, runner, features, prompt, project)).await
}

async fn recall(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    prompt: &str,
    project: Option<&str>,
) -> Lookup {
    let Ok(server) = features
        .feature_server(env, runner, KnowledgeSupport::recall, "recall")
        .await
    else {
        return Lookup::NotRun;
    };

    let run = RecallRun {
        sidecar: &KNOWLEDGE_SIDECAR,
        output_bytes: MAX_RECALL_OUTPUT_BYTES,
        is_block,
    };

    run.run(env, runner, &server, prompt, project).await
}

/// True when `text` is exactly one recall block, nothing around it, within the step's cap (judged
/// after redaction, which can lengthen a line): the step never cuts a block.
fn is_block(text: &str) -> bool {
    if text.len() > crate::hooks::steps::MAX_KNOWLEDGE_LOOKUP_BYTES {
        return false;
    }

    let Some(inner) = text.strip_prefix(OPEN).and_then(|rest| rest.strip_suffix(CLOSE)) else {
        return false;
    };

    !inner.contains(OPEN) && !inner.contains(CLOSE)
}
