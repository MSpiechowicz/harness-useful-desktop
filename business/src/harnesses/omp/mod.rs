//! Oh My Pi (`omp`, `@oh-my-pi/pi-coding-agent`), verified against 18.6.1.
//!
//! Paths, formats and precedence rules mirror the OMP sources; each module names the files it
//! mirrors. Shared PI-family parsing comes from [`crate::pi_family`].

mod catalog;
mod config;
mod extensions;
mod marketplaces;
mod paths;
mod plugins;
mod themes;

pub use catalog::OmpCatalog;
pub use config::omp_config_view;
pub use extensions::omp_extensions_view;
pub use paths::{active_profile, omp_paths};

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
            plugins: true,
            marketplaces: true,
            mcp: true,
            skills: true,
            extension_modules: true,
            themes: true,
            profiles: true,
            approval_modes: true,
            slash_commands_via_prompt: true,
            session_tree: true,
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

    fn install_hint(&self) -> InstallHint {
        InstallHint {
            command: "bun install -g @oh-my-pi/pi-coding-agent".to_owned(),
            docs_url: "https://github.com/can1357/oh-my-pi".to_owned(),
        }
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["18."]
    }
}
