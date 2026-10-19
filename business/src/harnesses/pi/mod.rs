//! Pi (`pi`, `@earendil-works/pi-coding-agent`), following the PI documentation (v1.1.0).
//!
//! The agent directory is `PI_CODING_AGENT_DIR` (absolute, `~` expanded) or `~/.pi/agent`; the
//! project layer is `<project>/.pi`, loaded by PI only for trusted projects. No PI release has been
//! verified live yet, so every detected version reports as untested.

mod catalog;
mod cli;
mod config;
mod dialect;
mod extensions;
mod launch;
mod layers;
mod packages;
mod sessions;
mod sources;

pub use catalog::PiCatalog;
pub use cli::pi_cli_action;
pub use config::pi_config_view;
pub use dialect::PiDialect;
pub use extensions::pi_extensions_view;
pub use launch::pi_launch_spec;
pub(crate) use packages::{PackageRecord, user_has_package, user_package_records};
pub use sessions::{pi_session_folder, pi_sessions_dir};

use crate::harness::prelude::*;

/// The PI adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pi;

/// Resolves the PI directories (see the module docs). PI has no profiles; its data lives in the
/// agent directory.
pub fn pi_paths(env: &Env, project: Option<&Path>) -> HarnessPaths {
    let custom = env
        .var("PI_CODING_AGENT_DIR")
        .map(|raw| env.expand_home(raw))
        .filter(|dir| dir.is_absolute());

    let (config_root, agent_dir) = match custom {
        Some(dir) => (dir.clone(), dir),
        None => (env.home_join(".pi"), env.home_join(".pi/agent")),
    };

    HarnessPaths {
        config_root,
        data_root: agent_dir.clone(),
        agent_dir,
        profile: None,
        project_dir: project.map(|project| project.join(".pi")),
    }
}

impl Harness for Pi {
    fn id(&self) -> HarnessId {
        HarnessId::Pi
    }

    fn info(&self) -> HarnessInfo {
        HarnessInfo {
            id: HarnessId::Pi,
            name: "Pi".to_owned(),
            short_name: "PI".to_owned(),
            binary_name: self.binary_name().to_owned(),
            description:
                "Minimal, extensible terminal coding agent. It grows through packages from npm or git, not plugins."
                    .to_owned(),
            homepage: "https://github.com/earendil-works/pi".to_owned(),
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            chat: true,
            packages: true,
            mcp: true,
            skills: true,
            prompt_templates: true,
            extension_modules: true,
            themes: true,
            cli_actions: true,
            project_trust: true,
            session_tree: true,
            compact_instructions: true,
            ..Capabilities::default()
        }
    }

    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths {
        pi_paths(env, project)
    }

    fn config_view(&self, env: &Env, project: Option<&Path>) -> Result<ConfigView> {
        Ok(pi_config_view(env, project))
    }

    fn extensions_view(&self, env: &Env, project: Option<&Path>) -> Result<ExtensionsView> {
        Ok(pi_extensions_view(env, project))
    }

    fn catalog(&self) -> &dyn CommandCatalog {
        &PiCatalog
    }

    fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        pi_launch_spec(request)
    }

    fn dialect(&self) -> &'static dyn RpcDialect {
        &PiDialect
    }

    fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf {
        pi_sessions_dir(env, project)
    }

    fn session_folder(&self, env: &Env, project: &Path) -> SessionFolder {
        pi_session_folder(env, project)
    }

    fn cli_action(&self, action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
        pi_cli_action(action, project)
    }

    /// 1.1 is the release the adapter follows and was checked against live.
    fn supported_versions(&self) -> &'static [&'static str] {
        &["1.1."]
    }

    fn install_hint(&self) -> InstallHint {
        InstallHint {
            command: "curl -fsSL https://pi.dev/install.sh | sh".to_owned(),
            docs_url: "https://github.com/earendil-works/pi".to_owned(),
        }
    }

    fn companion_setup(&self) -> CompanionSetup {
        CompanionSetup {
            skills: SkillsStrategy::Package {
                source: crate::ecosystem::catalog::PI_SKILLS_SOURCE,
            },
            mcp: McpTarget::UserMcpJson { also_read: &[] },
        }
    }

    fn share_support(&self) -> ShareSupport {
        ShareSupport {
            marketplaces: false,
            plugins: false,
            packages: true,
            mcp_servers: true,
        }
    }
}
