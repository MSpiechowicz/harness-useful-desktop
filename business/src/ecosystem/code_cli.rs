//! One run of a `harness-code` command for the app (`status`, `enable`, `disable`, `build`),
//! shared by [`code`](super::code) and [`code_prepare`](super::code_prepare).
//!
//! The run, its limits and its error lines are [`sidecar_cli`](super::sidecar_cli)'s, for
//! [`CODE_SIDECAR`].

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use super::catalog::CODE_SIDECAR;
use super::sidecar_cli::{LIST_TIMEOUT, run_sidecar_json};
use crate::env::Env;
use crate::error::Result;
use crate::process::ProcessRunner;

pub(crate) use super::sidecar_cli::{CliCall, LIST_OUTPUT_BYTES};

/// A `harness-code` command with a closed stdin, the limits of a read (15 s, 1 MiB of output) and
/// `--json` after `args`.
pub(crate) fn json_call<'a>(name: &'a str, args: &[&str]) -> CliCall<'a> {
    let mut words: Vec<OsString> = args.iter().map(OsString::from).collect();
    words.push(OsString::from("--json"));

    CliCall {
        name,
        args: words,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    }
}

/// Runs `call` on the code server at `server` and reads its stdout as JSON (see
/// [`run_sidecar_json`]).
pub(crate) async fn run_json(env: &Env, runner: &dyn ProcessRunner, server: &Path, call: CliCall<'_>) -> Result<Value> {
    run_sidecar_json(&CODE_SIDECAR, env, runner, server, call).await
}
