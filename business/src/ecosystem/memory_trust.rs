//! `harness-memory trust <id>=<sha256>… [--project <root>]` (`memory_scripts_trust`): the user
//! approves the versions of scripts they reviewed, so agents may run them without asking.
//!
//! Each script goes with the SHA-256 of the version the user saw: the server approves a script only
//! while its current version has that hash, so one changed after the review is not approved (it
//! says the script "changed since you reviewed it", which comes back as `conflict` for the UI to
//! load the script again and ask). The server keeps approvals in its local file and decides which
//! scripts the ids name (those visible from the project, or the global ones without one). It runs
//! as [`memory_cli`](super::memory_cli) runs the server's commands, with a [`trust_timeout`] limit;
//! its error line comes back redacted.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use super::catalog::MEMORY_BINARY;
use super::memory_cli::{CliAnswer, CliCall, run_memory_cli};
use crate::ecosystem::memory_binary::installed_memory;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::ScriptTrustItem;
use crate::process::ProcessRunner;

/// How long `harness-memory trust` may take before it counts the scripts (20 s).
pub const TRUST_TIMEOUT: Duration = Duration::from_secs(20);

/// What each script adds to [`TRUST_TIMEOUT`] (1 s: the server checks each one against its store).
const TRUST_TIMEOUT_PER_ITEM: Duration = Duration::from_secs(1);

/// The longest `harness-memory trust` may take, however many scripts (120 s).
const TRUST_TIMEOUT_MAX: Duration = Duration::from_secs(120);

/// How long `harness-memory trust` may take for `items` scripts: [`TRUST_TIMEOUT`] and a second
/// each, at most 120 s.
pub fn trust_timeout(items: usize) -> Duration {
    let count = u32::try_from(items).unwrap_or(u32::MAX);

    TRUST_TIMEOUT
        .saturating_add(TRUST_TIMEOUT_PER_ITEM.saturating_mul(count))
        .min(TRUST_TIMEOUT_MAX)
}

/// The most scripts approved in one call.
pub const MAX_TRUSTED_IDS: usize = 500;

/// What the server says about a script whose version is not the one reviewed.
const CHANGED_MARKER: &str = "changed since you reviewed it";

/// The output kept of each stream (16 KiB).
const TRUST_OUTPUT_BYTES: usize = 16 * 1024;

/// The arguments after the binary: `trust <id>=<sha256>…`, then `--project <root>` when there is
/// one. `invalidInput` for no items, more than [`MAX_TRUSTED_IDS`], an id below 1, or a hash that
/// is not 64 lowercase hex characters.
pub fn trust_args(items: &[ScriptTrustItem], project: Option<&Path>) -> Result<Vec<OsString>> {
    if items.is_empty() {
        return Err(BusinessError::InvalidInput("pick the scripts to trust".to_owned()));
    }

    if items.len() > MAX_TRUSTED_IDS {
        return Err(BusinessError::InvalidInput(format!(
            "at most {MAX_TRUSTED_IDS} scripts can be trusted at once"
        )));
    }

    if items.iter().any(|item| item.id < 1) {
        return Err(BusinessError::InvalidInput(
            "a script id is a positive number".to_owned(),
        ));
    }

    if !items.iter().all(|item| is_sha256(&item.sha256)) {
        return Err(BusinessError::InvalidInput(
            "a script hash is 64 lowercase hex characters".to_owned(),
        ));
    }

    let mut args = vec![OsString::from("trust")];
    args.extend(
        items
            .iter()
            .map(|item| OsString::from(format!("{}={}", item.id, item.sha256))),
    );

    if let Some(project) = project {
        args.push(OsString::from("--project"));
        args.push(project.as_os_str().to_owned());
    }

    Ok(args)
}

/// Whether `text` is a SHA-256 as the server writes it: 64 lowercase hex characters.
fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Runs `harness-memory trust` for `items` (see the module docs). `notFound` when Harness Memory
/// is not installed, `conflict` with the server's line (redacted) when a script changed since it
/// was reviewed (the others are still approved), `process` with the server's error line
/// (redacted) when it fails otherwise, does not start or does not answer in time.
pub async fn trust_scripts(
    env: &Env,
    runner: &dyn ProcessRunner,
    items: &[ScriptTrustItem],
    project: Option<&Path>,
) -> Result<()> {
    let args = trust_args(items, project)?;

    let Some(server) = installed_memory(env, runner).await else {
        return Err(BusinessError::NotFound(format!("executable {MEMORY_BINARY}")));
    };

    let call = CliCall {
        name: "trust",
        args,
        timeout: trust_timeout(items.len()),
        output_bytes: TRUST_OUTPUT_BYTES,
        stdin: None,
    };

    match run_memory_cli(env, runner, &server.path, call).await? {
        CliAnswer::Done(_) => {
            tracing::info!(scripts = items.len(), "memory scripts trusted");
            Ok(())
        }
        CliAnswer::Failed { message, .. } if message.contains(CHANGED_MARKER) => {
            tracing::info!("a memory script changed since it was reviewed");
            Err(BusinessError::Conflict(message))
        }
        CliAnswer::Failed { message, .. } => Err(BusinessError::Process(message)),
    }
}
