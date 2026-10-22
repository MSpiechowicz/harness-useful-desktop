//! Extension actions through `claude plugin …`, checked against the `--help` of Claude Code
//! 2.1.304.
//!
//! - Plugins: `install <plugin> --scope user|project`, `uninstall`, `enable`, `disable` and
//!   `update <plugin> --scope user`, each with `--json` (one result line on stdout, "same exit
//!   codes", so the exit code decides). `update` takes one plugin, so updating every plugin is not
//!   offered.
//! - Marketplaces: `marketplace add <source> --scope user --json`, `marketplace remove <name>
//!   --scope user --json` (without `--scope` it would remove the declaration from every scope) and
//!   `marketplace update [name]`, with `--json` only for a named one (help: "With a marketplace
//!   name"). A source is a GitHub `owner/repo`, an `https://` URL or an absolute path.
//! - No preview flag: a dry run runs nothing and reports the command line.
//! - `-y` / `--yes` is never sent: on `install` and `update` it accepts a marketplace-declared
//!   command (a command-source plugin, or a `headersHelper`) without showing it to a person.
//!   Without a TTY such an install fails, and the user installs it from a terminal.
//! - Packages are not Claude actions.
//! - Only a project-scope install runs in the project; every other action runs in the home
//!   directory.

use std::path::Path;

use serde_json::Value;

use crate::error::{BusinessError, Result};
use crate::harness::{CliInvocation, TargetKind, cli_target};
use crate::model::{ExtensionAction, InstallScope};

/// Asks for the machine-readable result line.
const JSON: &str = "--json";

/// The scope the app manages plugins and marketplaces in.
const USER: &str = "user";

/// The `claude` arguments of `action`. `project` must be set for a project-scope install, the only
/// action that runs in the project; every other one acts on the user's scope and runs in the home
/// directory, so no project settings (hooks, environment, helpers) apply to it.
pub fn claude_cli_action(action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
    let invocation = invocation(action, project)?;

    let in_project = matches!(
        action,
        ExtensionAction::PluginInstall {
            scope: InstallScope::Project,
            ..
        }
    );

    if in_project {
        Ok(invocation)
    } else {
        Ok(invocation.in_home())
    }
}

fn invocation(action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
    let plugin = |verb: &str, name: &str| -> Result<CliInvocation> {
        let name = cli_target(TargetKind::PluginName, name)?;

        Ok(CliInvocation::new([
            "plugin",
            verb,
            name.as_str(),
            "--scope",
            USER,
            JSON,
        ]))
    };

    match action {
        ExtensionAction::PluginInstall { target, scope } => {
            let scope = install_scope(*scope, project)?;
            let target = cli_target(TargetKind::Plugin, target)?;

            Ok(CliInvocation::new([
                "plugin",
                "install",
                target.as_str(),
                "--scope",
                scope,
                JSON,
            ]))
        }
        ExtensionAction::PluginUninstall { name } => plugin("uninstall", name),
        ExtensionAction::PluginEnable { name } => plugin("enable", name),
        ExtensionAction::PluginDisable { name } => plugin("disable", name),
        ExtensionAction::PluginUpgrade { name: Some(name) } => plugin("update", name),
        ExtensionAction::PluginUpgrade { name: None } => Err(BusinessError::Unsupported(
            "Claude Code updates one plugin at a time".to_owned(),
        )),
        ExtensionAction::MarketplaceAdd { source } => {
            let source = marketplace_source(source)?;

            Ok(CliInvocation::new([
                "plugin",
                "marketplace",
                "add",
                source.as_str(),
                "--scope",
                USER,
                JSON,
            ]))
        }
        ExtensionAction::MarketplaceRemove { name } => {
            let name = cli_target(TargetKind::MarketplaceName, name)?;

            Ok(CliInvocation::new([
                "plugin",
                "marketplace",
                "remove",
                name.as_str(),
                "--scope",
                USER,
                JSON,
            ]))
        }
        ExtensionAction::MarketplaceUpdate { name: Some(name) } => {
            let name = cli_target(TargetKind::MarketplaceName, name)?;

            Ok(CliInvocation::new([
                "plugin",
                "marketplace",
                "update",
                name.as_str(),
                JSON,
            ]))
        }
        ExtensionAction::MarketplaceUpdate { name: None } => {
            Ok(CliInvocation::new(["plugin", "marketplace", "update"]))
        }
        ExtensionAction::PackageInstall { .. }
        | ExtensionAction::PackageRemove { .. }
        | ExtensionAction::PackageUpdate { .. } => Err(BusinessError::Unsupported(
            "Claude Code has no packages; use a plugin action".to_owned(),
        )),
    }
}

/// The `--scope` of an install into `scope`; a project-scope install needs `project`.
fn install_scope(scope: InstallScope, project: Option<&Path>) -> Result<&'static str> {
    match (scope, project) {
        (InstallScope::User, _) => Ok(USER),
        (InstallScope::Project, Some(_)) => Ok("project"),
        (InstallScope::Project, None) => Err(BusinessError::InvalidInput(
            "a project-scope install needs a project".to_owned(),
        )),
    }
}

/// The `message` of the JSON result line `--json` prints last on stdout (its first line), when
/// there is one.
pub fn claude_summary(stdout: &str) -> Option<String> {
    let last = stdout.lines().map(str::trim).rfind(|line| !line.is_empty())?;
    let result: Value = serde_json::from_str(last).ok()?;
    let message = result.get("message")?.as_str()?;

    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

/// A marketplace source the app passes on: a GitHub `owner/repo`, an `https://` URL or an absolute
/// path, after [`cli_target`]'s checks. A network path (`//host/share`, `\\host\share`) is
/// refused: Claude Code would reach out to that host.
fn marketplace_source(source: &str) -> Result<String> {
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
