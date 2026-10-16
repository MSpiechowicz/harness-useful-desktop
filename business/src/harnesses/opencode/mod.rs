//! OpenCode (`opencode`, `opencode-ai`), following the OpenCode sources (1.19.3) and documentation.
//!
//! Config and data follow the XDG directories (see [`paths`]). OpenCode keeps its sessions in a
//! SQLite database that also holds credentials, so the app never opens it: sessions are listed,
//! exported and deleted through the CLI ([`Harness::cli_sessions`]). Chat (OpenCode speaks ACP,
//! not the PI-family RPC) is not available yet, and extension actions are not offered. Every spawn
//! switches off OpenCode's self-updater.

mod catalog;
mod config;
mod extensions;
mod paths;
mod sessions;
mod setup;

pub use catalog::OpenCodeCatalog;
pub use paths::{PROJECT_DIR_NAME, config_dir, custom_config_dir, data_dir, home_config_dir};
pub use sessions::OpenCodeSessions;

use crate::error::BusinessError;
use crate::harness::prelude::*;
use crate::harnesses::pi::PiDialect;
use crate::model::{SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use crate::pi_family::sessions::SessionCache;

/// The OpenCode adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCode;

impl Harness for OpenCode {
    fn id(&self) -> HarnessId {
        HarnessId::OpenCode
    }

    fn info(&self) -> HarnessInfo {
        HarnessInfo {
            id: HarnessId::OpenCode,
            name: "OpenCode".to_owned(),
            short_name: "OpenCode".to_owned(),
            binary_name: self.binary_name().to_owned(),
            description:
                "Open-source terminal coding agent with custom agents, commands and plugins, for any model provider."
                    .to_owned(),
            homepage: "https://opencode.ai".to_owned(),
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            plugins: true,
            mcp: true,
            skills: true,
            prompt_templates: true,
            agents: true,
            themes: true,
            ..Capabilities::default()
        }
    }

    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths {
        paths::paths(env, project)
    }

    fn config_view(&self, env: &Env, project: Option<&Path>) -> Result<ConfigView> {
        Ok(config::config_view(env, project))
    }

    fn extensions_view(&self, env: &Env, project: Option<&Path>) -> Result<ExtensionsView> {
        Ok(extensions::extensions_view(env, project))
    }

    fn catalog(&self) -> &dyn CommandCatalog {
        &catalog::CATALOG
    }

    fn launch_spec(&self, _request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        Err(BusinessError::Unsupported(
            "chat with OpenCode is coming soon".to_owned(),
        ))
    }

    /// Never reached: [`OpenCode::launch_spec`] refuses every launch.
    fn dialect(&self) -> &'static dyn RpcDialect {
        &PiDialect
    }

    /// The data directory, where the session database lives (it is never opened).
    fn sessions_dir(&self, env: &Env, _project: &Path) -> PathBuf {
        paths::data_dir(env)
    }

    fn list_sessions(&self, _env: &Env, _project: &Path, _cache: &SessionCache) -> Result<Vec<SessionSummary>> {
        Err(no_session_files())
    }

    fn resolve_session(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<PathBuf> {
        Err(no_session_files())
    }

    fn delete_session(&self, _env: &Env, _project: &Path, _session: &SessionRef, _empty_copy_only: bool) -> Result<()> {
        Err(no_session_files())
    }

    fn session_tree(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<SessionTree> {
        Err(no_session_files())
    }

    fn session_messages(
        &self,
        _env: &Env,
        _project: &Path,
        _session: &SessionRef,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> Result<SessionMessagesPage> {
        Err(no_session_files())
    }

    fn session_files_touched(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<Vec<String>> {
        Err(no_session_files())
    }

    fn cli_sessions(&self) -> Option<&dyn CliSessions> {
        Some(sessions::cli_sessions())
    }

    fn spawn_env(&self) -> &'static [(&'static str, &'static str)] {
        &[("OPENCODE_DISABLE_AUTOUPDATE", "1")]
    }

    fn install_hint(&self) -> InstallHint {
        let command = if cfg!(windows) {
            "npm install -g opencode-ai"
        } else {
            "curl -fsSL https://opencode.ai/install | bash"
        };

        InstallHint {
            command: command.to_owned(),
            docs_url: "https://opencode.ai/docs/".to_owned(),
        }
    }

    fn companion_setup(&self) -> CompanionSetup {
        setup::companion_setup()
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["1.18.", "1.19."]
    }
}

/// OpenCode has no session files; its sessions go through [`Harness::cli_sessions`].
fn no_session_files() -> BusinessError {
    BusinessError::Unsupported("OpenCode sessions are read through its CLI".to_owned())
}
