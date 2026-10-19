//! `harness-memory trust <id>=<sha256>… [--project <root>]` (`memory_scripts_trust`): the user
//! approves the versions of scripts they reviewed, so agents may run them without asking.
//!
//! Each script goes with the SHA-256 of the version the user saw: the server approves a script only
//! while its current version has that hash, so one changed after the review is not approved (it
//! says the script "changed since you reviewed it", which comes back as `conflict` for the UI to
//! load the script again and ask). The server keeps approvals in its local file and decides which
//! scripts the ids name (those visible from the project, or the global ones without one). It runs
//! as [`memory_cli`](super::memory_cli) runs the server's commands, with a [`trust_timeout`] limit;
//! its error line comes back redacted. [`script_sha256`] is that hash for a version the app has.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::catalog::MEMORY_BINARY;
use super::memory_cli::{CliAnswer, CliCall, run_memory_cli};
use crate::ecosystem::memory_binary::installed_memory;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{ScriptLanguage, ScriptTrustItem};
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

/// What Harness Memory's script hash starts with (its `SHA_TAG`).
const SCRIPT_SHA_TAG: &str = "harness-memory-script-v1";

/// The hash that names a script version for trust, as Harness Memory's `script_sha256`: the
/// lowercase hex SHA-256 of `harness-memory-script-v1 NUL <language> NUL <body>`.
pub fn script_sha256(language: ScriptLanguage, body: &str) -> String {
    let language = match language {
        ScriptLanguage::Sh => "sh",
        ScriptLanguage::Bash => "bash",
        ScriptLanguage::Python => "python",
        ScriptLanguage::Node => "node",
        ScriptLanguage::Bun => "bun",
    };

    let mut hash = Sha256::new();
    hash.update(SCRIPT_SHA_TAG.as_bytes());
    hash.update([0]);
    hash.update(language.as_bytes());
    hash.update([0]);
    hash.update(body.as_bytes());

    format!("{:x}", hash.finalize())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trust_hash_is_the_servers() {
        // Harness Memory's own vector: printf 'harness-memory-script-v1\0bash\0abc' | sha256sum
        assert_eq!(
            script_sha256(ScriptLanguage::Bash, "abc"),
            "b9b56e4b9f812e7f72cdd0dca3526f2e0d7e71ed4825bd5464eefb817dc89c1e"
        );
        assert_ne!(
            script_sha256(ScriptLanguage::Sh, "abc"),
            script_sha256(ScriptLanguage::Bash, "abc")
        );
        assert!(is_sha256(&script_sha256(ScriptLanguage::Python, "print(1)\n")));
    }
}
