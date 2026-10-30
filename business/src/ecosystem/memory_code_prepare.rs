//! Building the code index of a repository before an agent asks: `code build --project P --json`
//! ([`prepare_code_index`], with the `code-prepare` feature). The server keeps the
//! `code-builds` status file while it works, which [`CodeBuildWatcher`](super::memory_code::CodeBuildWatcher)
//! turns into a toast.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::memory_cli::{CliCall, LIST_OUTPUT_BYTES, clean, run_json};
use super::memory_code::{clip, one_line};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::process::ProcessRunner;

/// How long a preparation may run before the process group is killed (12 min). The server's own cap
/// bounds a build (about 10 min, one worker grace and a final graph check), so this is only a
/// backstop above it for a server that hangs.
pub const PREPARE_TIMEOUT: Duration = Duration::from_secs(12 * 60);

/// How a `code build` ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodePrepared {
    /// The server built the index of `files` files.
    Built {
        /// The files indexed.
        files: u64,
    },
    /// The server did not build: `reason` is `indexed`, `none`, `tooLarge`, `failed`, `refused`,
    /// `off` or `error`, with the server's line for `error`.
    Skipped {
        /// Why nothing was built.
        reason: String,
        /// The server's line, for an `error`.
        error: Option<String>,
    },
}

/// `code build --project <project> --json`: has the server index the repository now, so the first
/// `code_*` call of an agent finds it ready. The server keeps the `code-builds` status file the
/// watcher turns into a toast. It can take [`PREPARE_TIMEOUT`] on a huge repository. `protocol`
/// when the answer is not what the server prints.
pub async fn prepare_code_index(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: &Path,
) -> Result<CodePrepared> {
    let call = CliCall {
        name: "code build",
        args: vec![
            OsString::from("code"),
            OsString::from("build"),
            OsString::from("--project"),
            project.as_os_str().to_owned(),
            OsString::from("--json"),
        ],
        timeout: PREPARE_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    };

    let data = run_json(env, runner, server, call).await.map_err(clip_error)?;
    let prepared = read_prepared(&data)?;

    match &prepared {
        CodePrepared::Built { files } => tracing::info!(files, "code index built"),
        CodePrepared::Skipped { reason, error } if reason == "error" => {
            tracing::warn!(error = error.as_deref().unwrap_or_default(), "code index build failed");
        }
        CodePrepared::Skipped { reason, .. } => tracing::debug!(reason, "code index not built"),
    }

    Ok(prepared)
}

/// `error` with the server's line stripped of control characters, because it is logged and a line
/// from a child process may carry terminal escapes.
fn clip_error(error: BusinessError) -> BusinessError {
    match error {
        BusinessError::Process(line) => BusinessError::Process(clip(&line)),
        BusinessError::Protocol(line) => BusinessError::Protocol(clip(&line)),
        BusinessError::Timeout(line) => BusinessError::Timeout(clip(&line)),
        other => other,
    }
}

fn read_prepared(data: &Value) -> Result<CodePrepared> {
    let Some(built) = data.get("built").and_then(Value::as_bool) else {
        return Err(BusinessError::Protocol(
            "harness-memory code build answered without `built`".to_owned(),
        ));
    };

    if built {
        let Some(files) = data.get("files").and_then(Value::as_u64) else {
            return Err(BusinessError::Protocol(
                "harness-memory code build answered without `files`".to_owned(),
            ));
        };

        return Ok(CodePrepared::Built { files });
    }

    let Some(reason) = data.get("reason").and_then(Value::as_str) else {
        return Err(BusinessError::Protocol(
            "harness-memory code build answered without `reason`".to_owned(),
        ));
    };

    // The server's line can quote a path or a token, so it is cleaned before it is logged.
    let error = data
        .get("error")
        .and_then(Value::as_str)
        .map(|line| clean(&one_line(line)));

    Ok(CodePrepared::Skipped {
        reason: clip(reason),
        error,
    })
}
