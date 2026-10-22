//! Extension actions through the harness CLI (`extension_action`).
//!
//! An adapter maps an [`ExtensionAction`] onto its CLI arguments ([`Harness::cli_action`]);
//! [`run_cli_action`] validates them, runs the resolved binary without a shell and turns the output
//! into a [`CliResult`]: ANSI escapes stripped, secrets redacted, each stream capped at
//! [`MAX_CLI_OUTPUT_BYTES`], and `ok` decided by the exit code plus the adapter's own failure check
//! ([`Harness::cli_failure_line`]), because some CLIs exit with 0 when the action failed.
//!
//! A dry run uses the CLI's own preview flag. When the CLI has none for an action, nothing is run:
//! the result only carries the command line, so the UI can show it before the user confirms.
//! Either way [`CliResult::command`] is the command line that applies the action (without the
//! preview flag), while [`CliResult::argv`] is the one that ran.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use super::Harness;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{CliResult, ExtensionAction, MAX_CLI_OUTPUT_BYTES};
use crate::process::{ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary, launcher_search_dirs};
use crate::security::paths::has_credentials;
use crate::security::{is_safe_arg, redact_text};
use crate::sync::is_npm_registry_spec;

/// Time limit of one CLI action (installs may download packages).
pub const CLI_ACTION_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Longest accepted target (plugin name, package source, marketplace source).
pub const MAX_CLI_TARGET_CHARS: usize = 512;

/// Longest [`CliResult::summary`], in characters.
const MAX_SUMMARY_CHARS: usize = 300;

/// CSI sequences (colors, cursor moves) and OSC sequences (titles, hyperlinks).
static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-_]").expect("static regex is valid")
});

/// The CLI arguments of one action (after the program name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliInvocation {
    /// Arguments, already validated by the adapter (see [`cli_target`]).
    pub args: Vec<String>,
    /// The flag that turns the action into a preview, when the CLI has one for it.
    pub dry_run_flag: Option<&'static str>,
    /// Runs in the home directory even with a project: an action on the user's own scope, which
    /// must not pick up the project's settings.
    pub in_home_dir: bool,
}

impl CliInvocation {
    /// An invocation without a preview flag.
    pub fn new<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            args: args.into_iter().map(Into::into).collect(),
            dry_run_flag: None,
            in_home_dir: false,
        }
    }

    /// Makes the action run in the home directory (see [`CliInvocation::in_home_dir`]).
    pub fn in_home(mut self) -> Self {
        self.in_home_dir = true;
        self
    }

    /// Sets the preview flag.
    pub fn with_dry_run(mut self, flag: &'static str) -> Self {
        self.dry_run_flag = Some(flag);
        self
    }

    /// Appends `arg` when it is `Some`.
    pub fn push_optional(mut self, arg: Option<String>) -> Self {
        self.args.extend(arg);
        self
    }
}

/// What kind of value a CLI target is, for error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// A plugin to install (`name@marketplace`, a package spec).
    Plugin,
    /// An installed plugin's name or id.
    PluginName,
    /// A marketplace source (`owner/repo`, a URL).
    MarketplaceSource,
    /// A registered marketplace's name.
    MarketplaceName,
    /// A package source (`npm:…`, `git:…`, a URL or a local path).
    PackageSource,
    /// An npm registry module (`name`, `@scope/name`, optionally `@version` or `@tag`), never a
    /// path, URL or other scheme (OpenCode's `opencode plugin <module>`).
    NpmModule,
}

impl TargetKind {
    fn label(self) -> &'static str {
        match self {
            Self::Plugin => "plugin target",
            Self::PluginName => "plugin name",
            Self::MarketplaceSource => "marketplace source",
            Self::MarketplaceName => "marketplace name",
            Self::PackageSource => "package source",
            Self::NpmModule => "npm module",
        }
    }
}

/// Checks a target the webview sent and returns it unchanged.
///
/// Refused (`invalidInput`, without echoing the value): empty or longer than
/// [`MAX_CLI_TARGET_CHARS`], starting with `-` (it would read as a flag), characters outside
/// `A-Z a-z 0-9 @ . _ / : + = , -` (see [`is_safe_arg`]; this also keeps batch-script shims safe),
/// and URLs carrying credentials (`https://user:token@host/…`, `git:token@host/…`). An
/// [`TargetKind::NpmModule`] must also be an npm registry spec (a name of at most 214 characters
/// with an optional version or tag): paths, URLs and `file:`, `github:` or `git+` specs are refused.
pub fn cli_target(kind: TargetKind, value: &str) -> Result<String> {
    let label = kind.label();

    if value.chars().count() > MAX_CLI_TARGET_CHARS {
        return Err(BusinessError::InvalidInput(format!(
            "the {label} is longer than {MAX_CLI_TARGET_CHARS} characters"
        )));
    }

    if !is_safe_arg(value) {
        return Err(BusinessError::InvalidInput(format!(
            "the {label} must be non-empty, must not start with `-` and may only use letters, digits and @ . _ / : + = , -"
        )));
    }

    if has_credentials(value) {
        return Err(BusinessError::InvalidInput(format!(
            "the {label} must not contain credentials"
        )));
    }

    if kind == TargetKind::NpmModule && !is_npm_registry_spec(value) {
        return Err(BusinessError::InvalidInput(format!(
            "the {label} must be an npm package name (`name` or `@scope/name`), optionally with `@version`"
        )));
    }

    Ok(value.to_owned())
}

/// [`cli_target`] for an optional target.
pub fn optional_cli_target(kind: TargetKind, value: Option<&str>) -> Result<Option<String>> {
    value.map(|value| cli_target(kind, value)).transpose()
}

/// A marketplace source the app passes on: a GitHub `owner/repo`, an `https://` URL or an absolute
/// path, after [`cli_target`]'s checks. A network path (`//host/share`, `\\host\share`) is
/// refused: the harness would reach out to that host.
pub fn marketplace_source(source: &str) -> Result<String> {
    let source = cli_target(TargetKind::MarketplaceSource, source)?;

    if source.starts_with("//") || source.starts_with("\\\\") {
        return Err(BusinessError::InvalidInput(
            "the marketplace source must not be a network path".to_owned(),
        ));
    }

    let url = source
        .strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'));

    if url || is_github_repo(&source) || Path::new(&source).is_absolute() {
        return Ok(source);
    }

    Err(BusinessError::InvalidInput(
        "the marketplace source must be a GitHub owner/repo, an https:// URL or an absolute path".to_owned(),
    ))
}

/// `owner/repo`: an owner of letters, digits and `-`, and a repository name of letters, digits,
/// `.`, `_` and `-` that is not `.` or `..`.
fn is_github_repo(source: &str) -> bool {
    let Some((owner, repo)) = source.split_once('/') else {
        return false;
    };

    let owner_ok = !owner.is_empty()
        && owner
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    let repo_ok = !matches!(repo, "" | "." | "..")
        && repo
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character));

    owner_ok && repo_ok
}

/// One extension action to run.
#[derive(Clone, Copy)]
pub struct CliRun<'a> {
    /// The harness whose CLI runs.
    pub harness: &'a dyn Harness,
    /// The environment (the CLI runs in the home directory without a project).
    pub env: &'a Env,
    /// Runs the process.
    pub runner: &'a dyn ProcessRunner,
    /// The harness binary (as detected).
    pub binary: &'a ResolvedBinary,
    /// The action.
    pub action: &'a ExtensionAction,
    /// Preview instead of applying.
    pub dry_run: bool,
    /// The authorized project directory, for project-scoped actions; the CLI runs in it unless the
    /// action runs in the home directory ([`CliInvocation::in_home_dir`]).
    pub project: Option<&'a Path>,
}

/// Runs `run` (see the module docs). An action that runs in the project runs only in a folder the
/// harness trusts ([`Harness::check_project_trust`], no trust once). Errors: the action is invalid
/// or unsupported by the harness, the project is not trusted (`untrusted`), an argument is unsafe
/// for a batch-script shim (`forbidden`), or the process could not start.
pub async fn run_cli_action(run: CliRun<'_>) -> Result<CliResult> {
    let invocation = run.harness.cli_action(run.action, run.project)?;
    let command = display_argv(run.harness, &invocation.args);
    let mut args = invocation.args;

    if run.dry_run {
        match invocation.dry_run_flag {
            Some(flag) => args.push(flag.to_owned()),
            None => return Ok(preview_only(run.harness, command)),
        }
    }

    let cwd = match run.project {
        Some(project) if !invocation.in_home_dir => {
            // The CLI applies the project's own settings there, as a chat would.
            // No trust once here, so the refusal says how to trust the folder instead.
            run.harness
                .check_project_trust(run.env, project)
                .map_err(|error| match error {
                    BusinessError::Untrusted(_) => BusinessError::Untrusted(format!(
                        "{} does not trust this folder yet. Run it there once from a terminal, then try again.",
                        run.harness.info().name
                    )),
                    other => other,
                })?;
            project.to_path_buf()
        }
        _ => run.env.home.clone(),
    };
    let cli = HarnessCli {
        harness: run.harness,
        env: run.env,
        runner: run.runner,
        binary: run.binary,
    };

    cli.run(args, command, cwd, CLI_ACTION_TIMEOUT).await
}

/// A harness CLI to run fixed commands with, under the rules of [`run_cli_action`] (no shell, the
/// harness's spawn environment, capped and redacted output).
#[derive(Clone, Copy)]
pub struct HarnessCli<'a> {
    /// The harness whose CLI runs.
    pub harness: &'a dyn Harness,
    /// The environment (the CLI runs in the home directory).
    pub env: &'a Env,
    /// Runs the process.
    pub runner: &'a dyn ProcessRunner,
    /// The harness binary (as detected).
    pub binary: &'a ResolvedBinary,
}

impl HarnessCli<'_> {
    /// Runs `args` (after the program name) in the home directory with the time limit `timeout`.
    /// The arguments are the caller's own, not the webview's; `forbidden` when one is unsafe for a
    /// batch-script binary, an error when the process could not start.
    pub async fn run_args(self, args: Vec<String>, timeout: Duration) -> Result<CliResult> {
        let command = display_argv(self.harness, &args);

        self.run(args, command, self.env.home.clone(), timeout).await
    }

    async fn run(self, args: Vec<String>, command: Vec<String>, cwd: PathBuf, timeout: Duration) -> Result<CliResult> {
        // `cmd.exe` re-parses a batch script's arguments: flags are checked without their dashes.
        let batch_safe = args.iter().all(|arg| is_safe_arg(arg.trim_start_matches('-')));
        if self.binary.is_batch_script && !batch_safe {
            return Err(BusinessError::Forbidden(
                "an argument is unsafe for a batch-script binary".to_owned(),
            ));
        }

        let spec = ExecSpec::new(&self.binary.path)
            .args(args.iter().cloned())
            .cwd(cwd)
            .env("NO_COLOR", "1")
            .env("FORCE_COLOR", "0");
        let spec = self
            .harness
            .spawn_spec(self.env, spec)
            .search_dirs(launcher_search_dirs(self.env, &self.binary.path))
            .timeout(timeout)
            .max_output_bytes(MAX_CLI_OUTPUT_BYTES);

        let output = self.runner.run(&spec).await?;
        let argv = display_argv(self.harness, &args);

        Ok(cli_result(self.harness, argv, command, output, timeout))
    }
}

/// `args` after the harness's program name.
fn display_argv(harness: &dyn Harness, args: &[String]) -> Vec<String> {
    let mut argv = vec![harness.binary_name().to_owned()];
    argv.extend_from_slice(args);
    argv
}

/// The result of a dry run the CLI cannot do: nothing ran, only `command` is reported (as both
/// `argv` and `command`).
fn preview_only(harness: &dyn Harness, command: Vec<String>) -> CliResult {
    let summary = format!(
        "{} has no preview for this action; nothing was run.",
        harness.info().short_name
    );

    CliResult {
        ok: true,
        summary: Some(summary),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        truncated: false,
        timed_out: false,
        argv: command.clone(),
        command,
        config_edit: None,
    }
}

/// The result of a run: `argv` ran, `command` applies the action.
fn cli_result(
    harness: &dyn Harness,
    argv: Vec<String>,
    command: Vec<String>,
    output: ExecOutput,
    timeout: Duration,
) -> CliResult {
    let stdout = clean(&output.stdout);
    let stderr = clean(&output.stderr);
    let failure = harness.cli_failure_line(&stdout, &stderr);
    let ok = output.success() && failure.is_none();

    let summary = match failure {
        Some(line) => Some(line),
        None if output.timed_out => Some(stopped_line(timeout)),
        None if !ok => last_line(&stderr).or_else(|| exit_line(output.status)),
        None => harness
            .cli_summary(&stdout)
            .or_else(|| last_line(&stdout))
            .or_else(|| last_line(&stderr)),
    };

    CliResult {
        ok,
        summary: summary.map(|line| clip(&line)),
        exit_code: output.status,
        stdout,
        stderr,
        truncated: output.truncated,
        timed_out: output.timed_out,
        argv,
        command,
        config_edit: None,
    }
}

/// `text` without terminal escapes, then redacted.
pub(crate) fn clean(text: &str) -> String {
    redact_text(&ANSI.replace_all(text, ""))
}

fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
}

/// The summary of a run that hit its time limit.
fn stopped_line(timeout: Duration) -> String {
    let seconds = timeout.as_secs();

    if seconds >= 120 && seconds.is_multiple_of(60) {
        format!("Stopped after {} minutes without finishing.", seconds / 60)
    } else {
        format!("Stopped after {seconds} seconds without finishing.")
    }
}

fn exit_line(status: Option<i32>) -> Option<String> {
    let reason = status.map_or_else(
        || "was ended by a signal".to_owned(),
        |code| format!("exited with code {code}"),
    );

    Some(format!("The command {reason}."))
}

fn clip(line: &str) -> String {
    if line.chars().count() <= MAX_SUMMARY_CHARS {
        return line.to_owned();
    }

    let mut clipped: String = line.chars().take(MAX_SUMMARY_CHARS - 1).collect();
    clipped.push('…');
    clipped
}
