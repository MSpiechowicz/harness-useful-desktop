//! Extension actions through `omp plugin …`, verified against `cli/plugin-cli.ts` (18.6.1).
//!
//! - `install <target> --scope user|project` and `uninstall <name>` honour `--dry-run`.
//! - `enable`, `disable`, `upgrade` and `marketplace add|remove|update` have no preview: OMP
//!   parses `--dry-run` for them but ignores it (`upgrade --dry-run` really upgrades), so the flag
//!   is never sent with them.
//! - OMP prints failures as `✘ Failed to …: Error: …` (or `Failed to upgrade: …`) and exits with
//!   code 0 for some of them, so [`omp_failure_line`] decides success together with the exit code.
//!
//! PI-style packages are not OMP actions: OMP installs npm and git packages as plugins.

use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::harness::{CliInvocation, TargetKind, cli_target, optional_cli_target};
use crate::model::{ExtensionAction, InstallScope};

/// OMP's preview flag.
const DRY_RUN: &str = "--dry-run";

/// The `omp` arguments of `action`. `project` must be set for a project-scope install.
pub fn omp_cli_action(action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
    let plugin = |verb: &str, name: &str| -> Result<CliInvocation> {
        Ok(CliInvocation::new(["plugin", verb]).push_optional(Some(cli_target(TargetKind::PluginName, name)?)))
    };
    let marketplace =
        |verb: &str, target: Option<String>| CliInvocation::new(["plugin", "marketplace", verb]).push_optional(target);

    match action {
        ExtensionAction::PluginInstall { target, scope } => {
            if *scope == InstallScope::Project && project.is_none() {
                return Err(BusinessError::InvalidInput(
                    "a project-scope install needs a project".to_owned(),
                ));
            }

            let target = cli_target(TargetKind::Plugin, target)?;
            let scope = match scope {
                InstallScope::User => "user",
                InstallScope::Project => "project",
            };

            Ok(CliInvocation::new(["plugin", "install", target.as_str(), "--scope", scope]).with_dry_run(DRY_RUN))
        }
        ExtensionAction::PluginUninstall { name } => Ok(plugin("uninstall", name)?.with_dry_run(DRY_RUN)),
        ExtensionAction::PluginEnable { name } => plugin("enable", name),
        ExtensionAction::PluginDisable { name } => plugin("disable", name),
        ExtensionAction::PluginUpgrade { name } => {
            let name = optional_cli_target(TargetKind::PluginName, name.as_deref())?;

            Ok(CliInvocation::new(["plugin", "upgrade"]).push_optional(name))
        }
        ExtensionAction::MarketplaceAdd { source } => Ok(marketplace(
            "add",
            Some(cli_target(TargetKind::MarketplaceSource, source)?),
        )),
        ExtensionAction::MarketplaceRemove { name } => Ok(marketplace(
            "remove",
            Some(cli_target(TargetKind::MarketplaceName, name)?),
        )),
        ExtensionAction::MarketplaceUpdate { name } => Ok(marketplace(
            "update",
            optional_cli_target(TargetKind::MarketplaceName, name.as_deref())?,
        )),
        ExtensionAction::PackageInstall { .. }
        | ExtensionAction::PackageRemove { .. }
        | ExtensionAction::PackageUpdate { .. } => Err(BusinessError::Unsupported(
            "OMP installs packages as plugins; use a plugin action".to_owned(),
        )),
    }
}

/// The first line reporting a failure: one starting with `✘` or containing `Failed to`
/// (stderr first, where OMP prints errors).
pub fn omp_failure_line(stdout: &str, stderr: &str) -> Option<String> {
    stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|line| line.starts_with('✘') || line.contains("Failed to"))
        .map(str::to_owned)
}
