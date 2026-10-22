//! Extension actions through `codex plugin …`, checked against the `--help` of Codex CLI 0.154.
//!
//! - Plugins: `plugin add <name@marketplace>` and `plugin remove <name@marketplace>`. Codex installs
//!   plugins for the user only and has no command to switch one on or off or to update one (a
//!   marketplace upgrade refreshes what its plugins install), so those actions are refused.
//! - Marketplaces: `plugin marketplace add <source>` (a GitHub `owner/repo`, an `https://` URL or
//!   an absolute path), `plugin marketplace remove <name>` and `plugin marketplace upgrade [name]`.
//! - No `--json`: its result shape is not documented, so the CLI's last line is the summary.
//! - No preview flag: a dry run runs nothing and reports the command line.
//! - Packages are not Codex actions. Every action acts on the user's settings and runs in the home
//!   directory, so no project settings apply to it.

use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::harness::{CliInvocation, TargetKind, cli_target, marketplace_source};
use crate::harnesses::plugin_registry::split_id;
use crate::model::{ExtensionAction, InstallScope};

/// The `codex` arguments of `action` (see the module docs).
pub fn codex_cli_action(action: &ExtensionAction, _project: Option<&Path>) -> Result<CliInvocation> {
    invocation(action).map(CliInvocation::in_home)
}

fn invocation(action: &ExtensionAction) -> Result<CliInvocation> {
    match action {
        ExtensionAction::PluginInstall { target, scope } => {
            if *scope == InstallScope::Project {
                return Err(BusinessError::InvalidInput(
                    "Codex installs plugins for the user only".to_owned(),
                ));
            }

            let target = plugin_id(target)?;

            Ok(CliInvocation::new(["plugin", "add", target.as_str()]))
        }
        ExtensionAction::PluginUninstall { name } => {
            let name = plugin_id(name)?;

            Ok(CliInvocation::new(["plugin", "remove", name.as_str()]))
        }
        ExtensionAction::PluginEnable { .. } | ExtensionAction::PluginDisable { .. } => Err(
            BusinessError::Unsupported("Codex has no command to switch a plugin on or off".to_owned()),
        ),
        ExtensionAction::PluginUpgrade { .. } => Err(BusinessError::Unsupported(
            "Codex updates plugins through their marketplace".to_owned(),
        )),
        ExtensionAction::MarketplaceAdd { source } => {
            let source = marketplace_source(source)?;

            Ok(CliInvocation::new(["plugin", "marketplace", "add", source.as_str()]))
        }
        ExtensionAction::MarketplaceRemove { name } => {
            let name = cli_target(TargetKind::MarketplaceName, name)?;

            Ok(CliInvocation::new(["plugin", "marketplace", "remove", name.as_str()]))
        }
        ExtensionAction::MarketplaceUpdate { name } => {
            let mut invocation = CliInvocation::new(["plugin", "marketplace", "upgrade"]);

            if let Some(name) = name {
                invocation.args.push(cli_target(TargetKind::MarketplaceName, name)?);
            }

            Ok(invocation)
        }
        ExtensionAction::PackageInstall { .. }
        | ExtensionAction::PackageRemove { .. }
        | ExtensionAction::PackageUpdate { .. } => Err(BusinessError::Unsupported(
            "Codex has no packages; use a plugin action".to_owned(),
        )),
    }
}

/// A plugin as Codex names it, `name@marketplace` (a bare name would need `--marketplace`).
fn plugin_id(value: &str) -> Result<String> {
    let value = cli_target(TargetKind::Plugin, value)?;

    match split_id(&value) {
        (name, Some(marketplace)) if !name.is_empty() && !marketplace.is_empty() => Ok(value),
        _ => Err(BusinessError::InvalidInput(
            "the plugin must be named as name@marketplace".to_owned(),
        )),
    }
}
