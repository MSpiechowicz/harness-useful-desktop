//! Extension actions through OpenCode's plugin command, following `opencode plugin --help`
//! (1.19.3: `opencode plugin <module> -g`, "install plugin and update config", `-g` writes the
//! global config file) and `cli/src/commands/handlers/plugin/add.ts` (2.1.2: `opencode plugin add
//! <module>` installs the package and adds it to `plugins` of the global config file, without the
//! background service).
//!
//! - Only user-scope installs of npm registry modules (`name`, `@scope/name`, with an optional
//!   `@version` or `@tag`): a project install, enable, disable, upgrade (`-f`), marketplace and
//!   package actions are unsupported.
//! - Removal edits the global config file instead (see [`super::plugin_remove`], reached through
//!   [`crate::harness::Harness::config_action`]): OpenCode 1 has no uninstall command, and the
//!   edit keeps a preview on both generations.
//! - OpenCode has no preview flag, so a dry run never starts the CLI (see
//!   [`crate::harness::run_cli_action`]).
//! - Every run switches off OpenCode's self-updater ([`crate::harness::Harness::spawn_env`]).

use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::harness::{CliInvocation, OpenCodeGeneration, TargetKind, cli_target};
use crate::model::{ExtensionAction, InstallScope};

/// OpenCode 1's global-config flag.
const GLOBAL: &str = "-g";

/// The `opencode` arguments of `action` for `generation`.
pub fn opencode_cli_action(
    action: &ExtensionAction,
    _project: Option<&Path>,
    generation: OpenCodeGeneration,
) -> Result<CliInvocation> {
    match action {
        ExtensionAction::PluginInstall {
            target,
            scope: InstallScope::User,
        } => {
            let module = cli_target(TargetKind::NpmModule, target)?;

            match generation {
                OpenCodeGeneration::V1 => Ok(CliInvocation::new(["plugin", module.as_str(), GLOBAL])),
                OpenCodeGeneration::V2 => Ok(CliInvocation::new(["plugin", "add", module.as_str()])),
            }
        }
        ExtensionAction::PluginInstall {
            scope: InstallScope::Project,
            ..
        } => Err(BusinessError::Unsupported(
            "OpenCode plugins are installed for the user only".to_owned(),
        )),
        ExtensionAction::PluginUninstall { .. } => Err(BusinessError::Unsupported(
            "The app removes OpenCode plugins by editing its config file".to_owned(),
        )),
        _ => Err(BusinessError::Unsupported(
            "OpenCode can only install plugins from npm and remove them".to_owned(),
        )),
    }
}
