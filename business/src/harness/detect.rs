//! Default detection shared by every harness: locate the binary, ask it for its version, report
//! the configuration directory and, when missing, how to install it.

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use super::Harness;
use crate::env::{Env, OsKind};
use crate::model::{Detection, VersionSupport};
use crate::process::{ExecSpec, ProcessRunner, ResolvedBinary, launcher_search_dirs, resolve_binary};
use crate::security::is_safe_arg;

/// Time limit for the `--version` probe.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Output cap for the `--version` probe.
const VERSION_OUTPUT_BYTES: usize = 64 * 1024;

/// Executable extensions accepted for an override on Windows.
const WINDOWS_EXTENSIONS: &[&str] = &["exe", "cmd", "bat"];

static SEMVER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?").expect("static regex is valid"));

/// The first `major.minor.patch[-pre]` in `output` (`omp/18.6.1` → `18.6.1`, `1.1.0` → `1.1.0`).
pub fn first_semver(output: &str) -> Option<String> {
    SEMVER.find(output).map(|found| found.as_str().to_owned())
}

/// True when a user-chosen override may stand in for `binary_name`: its file name must be exactly
/// `binary_name`; on Windows the comparison ignores case and `.exe`/`.cmd`/`.bat` are allowed.
pub fn override_name_matches(os: OsKind, binary_name: &str, path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if os != OsKind::Windows {
        return file_name == binary_name;
    }

    let lower = file_name.to_ascii_lowercase();
    let stem = WINDOWS_EXTENSIONS
        .iter()
        .find_map(|extension| lower.strip_suffix(&format!(".{extension}")))
        .unwrap_or(&lower);

    stem == binary_name.to_ascii_lowercase()
}

/// The body of [`Harness::detect`].
pub(super) async fn detect<H: Harness + ?Sized>(
    harness: &H,
    env: &Env,
    runner: &dyn ProcessRunner,
    override_path: Option<&Path>,
) -> Detection {
    let name = harness.binary_name();
    let config_root = harness.paths(env, None).config_root;
    let mut problems = Vec::new();

    let override_path = match override_path {
        Some(path) if !override_name_matches(env.os, name, path) => {
            problems.push(format!("Ignored the custom binary: its file name must be `{name}`."));
            None
        }
        other => other,
    };

    if override_path.is_some_and(|path| !(path.is_absolute() && path.is_file())) {
        problems.push("The custom binary does not exist; searched PATH instead.".to_owned());
    }

    let mut detection = Detection {
        installed: false,
        binary_path: None,
        is_batch_script: false,
        version: None,
        version_support: VersionSupport::Unknown,
        config_dir: Some(config_root.to_string_lossy().into_owned()),
        config_dir_exists: config_root.is_dir(),
        install_hint: None,
        error: None,
    };

    match resolve_binary(env, name, override_path) {
        Some(binary) => {
            detection.installed = true;
            detection.binary_path = Some(binary.path.to_string_lossy().into_owned());
            detection.is_batch_script = binary.is_batch_script;

            match probe_version(harness, env, runner, &binary).await {
                Ok(version) => {
                    detection.version_support = harness.version_support(&version);
                    detection.version = Some(version);
                }
                Err(problem) => problems.push(problem),
            }
        }
        None => detection.install_hint = Some(harness.install_hint()),
    }

    if !problems.is_empty() {
        detection.error = Some(problems.join(" "));
    }

    detection
}

/// Runs `<binary> <version args>` (never through a shell) and parses the version.
async fn probe_version<H: Harness + ?Sized>(
    harness: &H,
    env: &Env,
    runner: &dyn ProcessRunner,
    binary: &ResolvedBinary,
) -> Result<String, String> {
    let args = harness.version_args();
    let command = format!("`{} {}`", harness.binary_name(), args.join(" "));

    // `cmd.exe` re-parses a batch script's arguments; flags are accepted only when the text after
    // their leading dashes passes the same charset check as every other batch argument.
    let batch_safe = args.iter().all(|arg| is_safe_arg(arg.trim_start_matches('-')));
    if binary.is_batch_script && !batch_safe {
        return Err(format!("Skipped {command}: unsafe argument for a batch script."));
    }

    let spec = ExecSpec::new(&binary.path)
        .args(args.iter().copied())
        .search_dirs(launcher_search_dirs(env, &binary.path))
        .timeout(VERSION_TIMEOUT)
        .max_output_bytes(VERSION_OUTPUT_BYTES);

    let output = runner
        .run(&spec)
        .await
        .map_err(|error| format!("Could not run {command}: {error}."))?;

    if output.timed_out {
        return Err(format!(
            "{command} did not answer within {} s.",
            VERSION_TIMEOUT.as_secs()
        ));
    }

    if !output.success() {
        let code = output
            .status
            .map_or_else(|| "a signal".to_owned(), |code| format!("exit code {code}"));
        return Err(format!("{command} failed with {code}."));
    }

    harness
        .parse_version(&output.stdout)
        .or_else(|| harness.parse_version(&output.stderr))
        .ok_or_else(|| format!("Could not read a version from {command}."))
}
