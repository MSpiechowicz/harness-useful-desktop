//! Extension actions through `pi install|remove|update`, following the PI documentation
//! (`docs/cli.md` "Package commands", `docs/packages.md`; v1.1.0).
//!
//! - Sources are `npm:…`, `git:…`, URLs (treated as git) or local paths.
//! - With a project, `install` and `remove` add `--local`, which writes the declaration to the
//!   project's `.pi/settings.json` (PI documents the flag for these two only); the CLI runs in the
//!   project either way.
//! - `update` always names what it updates: `pi update` without a target updates PI itself, which
//!   the app never does, so "update every package" is `pi update --extensions`.
//! - PI has no preview flag, so a dry run never starts the CLI (see [`crate::harness::run_cli_action`]).
//! - The project-trust flags are never passed.

use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::harness::{CliInvocation, TargetKind, cli_target};
use crate::model::ExtensionAction;

/// PI's project-settings flag.
const LOCAL: &str = "--local";

/// The `pi` arguments of `action`.
pub fn pi_cli_action(action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
    let local = project.map(|_| LOCAL.to_owned());

    match action {
        ExtensionAction::PackageInstall { source } => {
            let source = cli_target(TargetKind::PackageSource, source)?;

            Ok(CliInvocation::new(["install", source.as_str()]).push_optional(local))
        }
        ExtensionAction::PackageRemove { source } => {
            let source = cli_target(TargetKind::PackageSource, source)?;

            Ok(CliInvocation::new(["remove", source.as_str()]).push_optional(local))
        }
        ExtensionAction::PackageUpdate { source: Some(source) } => {
            let source = cli_target(TargetKind::PackageSource, source)?;

            Ok(CliInvocation::new(["update", source.as_str()]))
        }
        ExtensionAction::PackageUpdate { source: None } => Ok(CliInvocation::new(["update", "--extensions"])),
        _ => Err(BusinessError::Unsupported(
            "PI has no plugins or marketplaces; use a package action".to_owned(),
        )),
    }
}
