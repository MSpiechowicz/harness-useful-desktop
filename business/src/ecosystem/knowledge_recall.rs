//! The recall the app runs for the `knowledge-recall` family hook (`harness-knowledge recall
//! --hook [--project P]`, see [`crate::hooks::recall`]).
//!
//! Needs `recall` in the server's features. The prompt goes to the server's stdin (at most
//! [`MAX_RECALL_PROMPT_BYTES`], cut at a character), no shell is involved, and the server gets
//! [`RECALL_TIMEOUT`]: past it the process group is killed. It prints a block of at most three
//! pointers or nothing and exits 0. Anything else (a failure, a timeout, a server that is missing
//! or predates `recall`, output that is not exactly one recall block) gives `None`. The block is
//! redacted like a hook command's output. Neither the prompt nor the block is logged.

use std::path::Path;
use std::time::Duration;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_cli::words;
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::sidecar_cli::{CliAnswer, CliCall, project_args, run_sidecar_cli};
use crate::env::Env;
use crate::process::ProcessRunner;
use crate::security::redact_text;

/// How long a recall may take before it is killed.
pub const RECALL_TIMEOUT: Duration = Duration::from_secs(3);

/// The output kept of a recall (the server prints at most 1 KiB).
pub const MAX_RECALL_OUTPUT_BYTES: usize = 2 * 1024;

/// The most prompt bytes sent to the server (the most it reads).
pub const MAX_RECALL_PROMPT_BYTES: usize = 64 * 1024;

/// The tag the server wraps its pointers in.
const OPEN: &str = "<harness-knowledge-recall>";
const CLOSE: &str = "</harness-knowledge-recall>";

/// The pointers Harness Knowledge recalls for `prompt` in `project` (the chat's project root, none
/// without one), as the server's block; `None` when there are none or the recall did not work.
pub async fn knowledge_recall(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    prompt: &str,
    project: Option<&str>,
) -> Option<String> {
    let server = features
        .feature_server(env, runner, KnowledgeSupport::recall, "recall")
        .await
        .ok()?;

    let mut args = words(&["recall", "--hook"]);
    args.extend(project_args(project.map(Path::new)));

    let call = CliCall {
        name: "recall",
        args,
        timeout: RECALL_TIMEOUT,
        output_bytes: MAX_RECALL_OUTPUT_BYTES,
        stdin: Some(clip(prompt).as_bytes().to_vec()),
    };

    match run_sidecar_cli(&KNOWLEDGE_SIDECAR, env, runner, &server, call).await {
        Ok(CliAnswer::Done(stdout)) => block_of(&stdout),
        Ok(CliAnswer::Failed { .. }) | Err(_) => None,
    }
}

/// `prompt` cut to [`MAX_RECALL_PROMPT_BYTES`] at a character.
fn clip(prompt: &str) -> &str {
    let mut end = prompt.len().min(MAX_RECALL_PROMPT_BYTES);

    while !prompt.is_char_boundary(end) {
        end -= 1;
    }

    &prompt[..end]
}

/// The redacted recall block in `stdout`: it must be exactly one block, nothing around it.
fn block_of(stdout: &str) -> Option<String> {
    let text = redact_text(stdout.trim());

    let inner = text.strip_prefix(OPEN)?.strip_suffix(CLOSE)?;

    if inner.contains(OPEN) || inner.contains(CLOSE) {
        return None;
    }

    Some(text)
}
