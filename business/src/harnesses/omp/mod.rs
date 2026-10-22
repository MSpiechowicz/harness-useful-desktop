//! Oh My Pi (`omp`, `@oh-my-pi/pi-coding-agent`), verified against 18.6.1.
//!
//! Paths, formats and precedence rules mirror the OMP sources; each module names the files it
//! mirrors. Shared PI-family parsing comes from [`crate::pi_family`].

mod catalog;
mod cli;
mod config;
mod dialect;
mod extensions;
mod launch;
mod marketplaces;
mod paths;
mod plugins;
mod sessions;
mod subagents;
mod themes;

pub use catalog::OmpCatalog;
pub use cli::{omp_cli_action, omp_failure_line};
pub use config::omp_config_view;
pub use dialect::OmpDialect;
pub use extensions::omp_extensions_view;
pub use launch::{
    DEFAULT_APPROVAL_MODE, cli_approval_mode, configured_approval_mode, omp_launch_spec, parse_approval_mode,
};
pub(crate) use marketplaces::{GITHUB_REPO, MarketplaceRecord, is_word_like, marketplace_records};
pub use marketplaces::{mark_updates, marketplaces};
pub use paths::{active_profile, omp_paths};
pub use plugins::{InstalledPlugins, installed_plugins};
pub(crate) use plugins::{PluginOrigin, PluginRecord, user_plugin_records};
pub use sessions::{omp_session_store, omp_sessions_dir, omp_sessions_root};

use crate::ecosystem::catalog::{SKILLS_MARKETPLACE, SKILLS_TARGET};
use crate::harness::prelude::*;

/// The OMP adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct Omp;

impl Harness for Omp {
    fn id(&self) -> HarnessId {
        HarnessId::Omp
    }

    fn info(&self) -> HarnessInfo {
        HarnessInfo {
            id: HarnessId::Omp,
            name: "Oh My Pi".to_owned(),
            short_name: "OMP".to_owned(),
            binary_name: self.binary_name().to_owned(),
            description: "Batteries-included coding agent with plugins, marketplaces and profiles.".to_owned(),
            homepage: "https://omp.sh".to_owned(),
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            chat: true,
            plugins: true,
            marketplaces: true,
            mcp: true,
            skills: true,
            extension_modules: true,
            themes: true,
            cli_actions: true,
            profiles: true,
            approval_modes: true,
            slash_commands_via_prompt: true,
            session_tree: true,
            move_session: true,
            remove_queued: true,
            compact_instructions: true,
            memory_chat_guards: true,
            ..Capabilities::default()
        }
    }

    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths {
        omp_paths(env, project)
    }

    fn config_view(&self, env: &Env, project: Option<&Path>) -> Result<ConfigView> {
        Ok(omp_config_view(env, project))
    }

    fn extensions_view(&self, env: &Env, project: Option<&Path>) -> Result<ExtensionsView> {
        Ok(omp_extensions_view(env, project))
    }

    fn catalog(&self) -> &dyn CommandCatalog {
        &OmpCatalog
    }

    fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        omp_launch_spec(request)
    }

    fn dialect(&self) -> &'static dyn RpcDialect {
        &OmpDialect
    }

    fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf {
        omp_sessions_dir(env, project)
    }

    fn session_store(&self, env: &Env) -> Result<SessionStore> {
        omp_session_store(env)
    }

    fn configured_approval_mode(&self, env: &Env, project: Option<&Path>) -> Option<ApprovalMode> {
        Some(configured_approval_mode(env, project))
    }

    fn cli_action(&self, action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
        omp_cli_action(action, project)
    }

    fn cli_failure_line(&self, stdout: &str, stderr: &str) -> Option<String> {
        omp_failure_line(stdout, stderr)
    }

    fn install_hint(&self) -> InstallHint {
        InstallHint::single(
            "bun install -g @oh-my-pi/pi-coding-agent",
            "https://github.com/can1357/oh-my-pi",
        )
    }

    fn companion_setup(&self) -> CompanionSetup {
        CompanionSetup {
            skills: SkillsStrategy::Marketplace {
                source: SKILLS_MARKETPLACE,
                plugin: SKILLS_TARGET,
            },
            mcp: McpTarget::UserMcpJson {
                also_read: &[".mcp.json"],
            },
        }
    }

    fn share_support(&self) -> ShareSupport {
        ShareSupport {
            marketplaces: true,
            plugins: true,
            packages: false,
            mcp_servers: true,
        }
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["18."]
    }
}
