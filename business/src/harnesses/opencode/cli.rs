//! Extension actions through `opencode plugin <module> -g`, following `opencode plugin --help`
//! (1.19.3: "install plugin and update config", `-g` writes the global config file).
//!
//! - Only user-scope installs of npm registry modules (`name`, `@scope/name`, with an optional
//!   `@version` or `@tag`): a project install, enable, disable, upgrade (`-f`), marketplace and
//!   package actions are unsupported.
//! - OpenCode has no uninstall command: removal edits the global config file instead (see
//!   [`super::plugin_remove`], reached through [`crate::harness::Harness::config_action`]).
//! - OpenCode has no preview flag, so a dry run never starts the CLI (see
//!   [`crate::harness::run_cli_action`]).
//! - Every run switches off OpenCode's self-updater ([`crate::harness::Harness::spawn_env`]).

use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::harness::{CliInvocation, TargetKind, cli_target};
use crate::model::{ExtensionAction, InstallScope};

/// OpenCode's global-config flag.
const GLOBAL: &str = "-g";

/// The `opencode` arguments of `action`.
pub fn opencode_cli_action(action: &ExtensionAction, _project: Option<&Path>) -> Result<CliInvocation> {
    match action {
        ExtensionAction::PluginInstall {
            target,
            scope: InstallScope::User,
        } => {
            let module = cli_target(TargetKind::NpmModule, target)?;

            Ok(CliInvocation::new(["plugin", module.as_str(), GLOBAL]))
        }
        ExtensionAction::PluginInstall {
            scope: InstallScope::Project,
            ..
        } => Err(BusinessError::Unsupported(
            "OpenCode plugins are installed for the user only".to_owned(),
        )),
        ExtensionAction::PluginUninstall { .. } => Err(BusinessError::Unsupported(
            "OpenCode has no uninstall command, so the app edits its config file instead".to_owned(),
        )),
        _ => Err(BusinessError::Unsupported(
            "OpenCode can only install plugins from npm and remove them".to_owned(),
        )),
    }
}
