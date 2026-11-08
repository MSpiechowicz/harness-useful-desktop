//! Running a harness's own updater (`claude update`, `pi update --self`).
//!
//! The command line is fixed per harness ([`update_args`]): nothing from the webview reaches it. The
//! binary is the one detection found (the user's override included), run by its absolute path
//! without a shell, in the home directory, with a closed stdin. The app's own switches that stop a
//! CLI from updating itself ([`Harness::spawn_env`]) are left off this one run, because the CLI
//! may refuse to update with them. An update counts only when the updater exited with 0 and
//! detection afterwards reads another version than before: a package-manager install, where
//! `claude update` only prints advice, exits with 0 and changes nothing.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use super::{has_update_command, update_args, update_command};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{Harness, OpenCodeGeneration, clean};
use crate::model::{HarnessId, HarnessUpdateResult};
use crate::process::{ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary, launcher_search_dirs};
use crate::security::{is_safe_arg, redact_text};

/// Time limit of one update (it downloads and installs a package).
pub const UPDATE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Output cap of one update, per stream; the end is kept, because the last line is what is shown.
const UPDATE_OUTPUT_BYTES: usize = 256 * 1024;

/// Why a CLI without the updater command is not run (see [`has_update_command`]).
const TOO_OLD_MESSAGE: &str =
    "This version is too old to update from the app. The command below installs the latest one.";

/// Longest [`HarnessUpdateResult::message`], in characters.
const MAX_MESSAGE_CHARS: usize = 200;

/// An escape sequence with intermediates (`ESC ( B`), or a control or format character other than
/// the line ends and tabs the output is split on.
static UNPRINTABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b[ -/]*[0-~]?|[\p{Cc}\p{Cf}&&[^\n\r\t]]").expect("static regex is valid"));

/// Runs the updater of `harness` and reports whether its version changed. `override_path` is the
/// user's chosen binary (see [`Harness::detect`]). `notFound` when the harness is not installed,
/// `forbidden` when an argument is unsafe for a batch-script binary, an error when the process
/// could not start. A CLI too old to have the updater (or of an unknown version) is not run: the result
/// says so and gives the install command. The caller holds the lock that allows one update at a time.
pub async fn run_update(
    harness: &dyn Harness,
    env: &Env,
    runner: &dyn ProcessRunner,
    override_path: Option<&Path>,
) -> Result<HarnessUpdateResult> {
    let before = harness.summary(env, runner, override_path).await;

    // The binary detection found, the override included.
    let detection = &before.detection;
    let binary = match detection.binary_path.as_deref() {
        Some(path) if detection.installed => ResolvedBinary {
            path: path.into(),
            is_batch_script: detection.is_batch_script,
        },
        _ => {
            let name = harness.info().short_name;
            return Err(BusinessError::NotFound(format!("{name} is not installed")));
        }
    };

    // Before anything is spawned: an old CLI may take `update` as a prompt and start an agent.
    if !has_update_command(harness.id(), detection.version.as_deref()) {
        let command = reinstall_command(harness, detection.version.as_deref());

        return Ok(HarnessUpdateResult {
            updated: false,
            summary: before,
            message: Some(TOO_OLD_MESSAGE.to_owned()),
            command,
        });
    }

    let spec = update_spec(harness, env, &binary)?;
    let output = runner.run(&spec).await?;

    let summary = harness.summary(env, runner, override_path).await;
    let changed = summary.detection.version.is_some() && summary.detection.version != before.detection.version;
    let updated = output.success() && changed;

    let message = if updated {
        None
    } else {
        Some(not_updated_message(&output, changed))
    };

    tracing::info!(
        harness = ?harness.id(),
        exit_code = ?output.status,
        timed_out = output.timed_out,
        updated,
        "harness update finished"
    );

    Ok(HarnessUpdateResult {
        updated,
        summary,
        message,
        command: update_command(harness.id()),
    })
}

/// The first install command of `harness`, for OpenCode the one of the generation of `installed`.
/// An old version lacks `update`, so reinstalling is the way to the latest one.
fn reinstall_command(harness: &dyn Harness, installed: Option<&str>) -> String {
    let commands = harness.install_hint().commands;

    let label = match (harness.id(), installed.map(OpenCodeGeneration::from_version)) {
        (HarnessId::OpenCode, Some(OpenCodeGeneration::V2)) => Some("OpenCode 2"),
        (HarnessId::OpenCode, _) => Some("OpenCode 1"),
        _ => None,
    };

    let labelled = label.and_then(|label| commands.iter().find(|entry| entry.label.as_deref() == Some(label)));

    labelled
        .or(commands.first())
        .map(|entry| entry.command.clone())
        .unwrap_or_default()
}

/// The shell-free command line of the updater of `harness`.
fn update_spec(harness: &dyn Harness, env: &Env, binary: &ResolvedBinary) -> Result<ExecSpec> {
    let args = update_args(harness.id());

    // `cmd.exe` re-parses a batch script's arguments: flags are checked without their dashes.
    let batch_safe = args.iter().all(|arg| is_safe_arg(arg.trim_start_matches('-')));
    if binary.is_batch_script && !batch_safe {
        return Err(BusinessError::Forbidden(
            "an argument is unsafe for a batch-script binary".to_owned(),
        ));
    }

    let spec = ExecSpec::new(&binary.path)
        .args(args.iter().copied())
        .cwd(env.home.clone())
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0");

    Ok(without_injected_env(harness, harness.spawn_spec(env, spec))
        .search_dirs(launcher_search_dirs(env, &binary.path))
        .timeout(UPDATE_TIMEOUT)
        .max_output_bytes(UPDATE_OUTPUT_BYTES)
        .keep_output_tail())
}

/// `spec` without the variables [`Harness::spawn_env`] set. A variable the user exported under
/// the same name is not in the spec, so the child still inherits it.
fn without_injected_env(harness: &dyn Harness, mut spec: ExecSpec) -> ExecSpec {
    let injected = harness.spawn_env();

    spec.env.retain(|(name, value)| {
        !injected.iter().any(|(injected_name, injected_value)| {
            name == OsStr::new(injected_name) && value == OsStr::new(injected_value)
        })
    });

    spec
}

/// Why a run did not update: the updater's last line (stderr after a failure, stdout after an exit
/// with 0), else a plain sentence. The output is cleaned of terminal escapes and redacted first.
fn not_updated_message(output: &ExecOutput, version_changed: bool) -> String {
    if output.timed_out {
        return format!(
            "The update did not finish within {} minutes.",
            UPDATE_TIMEOUT.as_secs() / 60
        );
    }

    let (stdout, stderr) = (printable(&output.stdout), printable(&output.stderr));
    let line = if output.success() {
        last_line(&stdout).or_else(|| last_line(&stderr))
    } else {
        last_line(&stderr).or_else(|| last_line(&stdout))
    };

    if let Some(line) = line {
        return clip(&line);
    }

    match output.status {
        Some(0) if !version_changed => "The version did not change.".to_owned(),
        Some(0) => "The update ran but the new version could not be read.".to_owned(),
        Some(code) => format!("The update failed with exit code {code}."),
        None => "The update was stopped before it finished.".to_owned(),
    }
}

/// `text` cleaned and redacted, without what could spoof the toast it is shown in: the escape
/// sequences [`clean`] leaves, control characters and invisible format characters (bidi overrides).
/// Redacted again last: a character removed here may have split a secret [`clean`] did not see.
fn printable(text: &str) -> String {
    redact_text(&UNPRINTABLE.replace_all(&clean(text), ""))
}

/// The last non-empty line of `text`; a carriage return ends a line too, so a progress display that
/// redraws one line leaves its final state.
fn last_line(text: &str) -> Option<String> {
    text.split(['\n', '\r'])
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
}

fn clip(line: &str) -> String {
    if line.chars().count() <= MAX_MESSAGE_CHARS {
        return line.to_owned();
    }

    let mut clipped: String = line.chars().take(MAX_MESSAGE_CHARS - 1).collect();
    clipped.push('…');
    clipped
}
