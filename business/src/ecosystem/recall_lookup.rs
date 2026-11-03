//! What the two app-run recalls (`harness-knowledge recall --hook`, `harness-memory recall --hook`)
//! share: the limits of one run and how its end becomes a [`Lookup`].
//!
//! The prompt goes to the server's stdin (at most [`MAX_RECALL_PROMPT_BYTES`], cut at a character),
//! no shell is involved, and the server gets [`RECALL_TIMEOUT`]: past it the process group is
//! killed. A server that exits 0 prints one block or nothing. Neither the prompt nor the block is
//! logged.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use super::knowledge_cli::words;
use super::sidecar::Sidecar;
use super::sidecar_cli::{CliAnswer, CliCall, project_args, run_sidecar_cli};
use crate::env::Env;
use crate::error::Result;
use crate::hooks::Lookup;
use crate::process::ProcessRunner;
use crate::security::redact_text;

/// How long a recall may take before it is killed.
pub const RECALL_TIMEOUT: Duration = Duration::from_secs(3);

/// The most prompt bytes sent to a server (the most it reads).
pub const MAX_RECALL_PROMPT_BYTES: usize = 64 * 1024;

/// `recall` held to [`RECALL_TIMEOUT`]: one that did not finish in time is [`Lookup::Failed`].
pub(super) async fn within_timeout(recall: impl Future<Output = Lookup>) -> Lookup {
    match tokio::time::timeout(RECALL_TIMEOUT, recall).await {
        Ok(lookup) => lookup,
        Err(_elapsed) => Lookup::Failed,
    }
}

/// What one recall run needs besides the process plumbing: the server's `sidecar`, how much of its
/// output is kept and what shape a block must have.
pub(super) struct RecallRun<'a> {
    pub sidecar: &'a Sidecar,
    pub output_bytes: usize,
    pub is_block: fn(&str) -> bool,
}

impl RecallRun<'_> {
    /// Runs `recall --hook [--project P]` on `server` with `prompt` on stdin.
    pub(super) async fn run(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        server: &Path,
        prompt: &str,
        project: Option<&str>,
    ) -> Lookup {
        let mut args = words(&["recall", "--hook"]);
        args.extend(project_args(project.map(Path::new)));

        let call = CliCall {
            name: "recall",
            args,
            timeout: RECALL_TIMEOUT,
            output_bytes: self.output_bytes,
            stdin: Some(clip(prompt).as_bytes().to_vec()),
        };

        lookup_of(
            run_sidecar_cli(self.sidecar, env, runner, server, call).await,
            self.is_block,
        )
    }
}

/// `prompt` cut to [`MAX_RECALL_PROMPT_BYTES`] at a character.
pub(super) fn clip(prompt: &str) -> &str {
    let mut end = prompt.len().min(MAX_RECALL_PROMPT_BYTES);

    while !prompt.is_char_boundary(end) {
        end -= 1;
    }

    &prompt[..end]
}

/// How a recall that ran ended: nothing printed is [`Lookup::Empty`], a block `well_formed` accepts
/// is [`Lookup::Found`] (redacted like a hook command's output, and judged after the redaction), and anything else (a non-zero
/// exit, a timeout, a server that did not start, other output) is [`Lookup::Failed`].
pub(super) fn lookup_of(answer: Result<CliAnswer>, well_formed: impl Fn(&str) -> bool) -> Lookup {
    let stdout = match answer {
        Ok(CliAnswer::Done(stdout)) => stdout,
        Ok(CliAnswer::Failed { .. }) | Err(_) => return Lookup::Failed,
    };

    let text = stdout.trim();

    if text.is_empty() {
        return Lookup::Empty;
    }

    // Redacted first: the shape and the size are judged on the text that goes in the step, so
    // redaction never leaves a block that was not checked (or that a later cut would clip).
    let redacted = redact_text(text);

    if well_formed(&redacted) {
        Lookup::Found(redacted)
    } else {
        Lookup::Failed
    }
}
