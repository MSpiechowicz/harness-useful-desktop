//! Codex (`codex`), following the Codex CLI 0.154 ([plan](../../../../docs/plans/codex.md)).
//!
//! Everything lives in one home directory (`~/.codex` or `CODEX_HOME`, see [`paths`]): the
//! `config.toml` settings, skills, plugins and the sessions of every project. The harness is
//! detected and listed for now: the config and extension views are empty, sessions and chat are
//! not available yet, and the catalog lists every command as unsupported. The login store
//! (`auth.json`) is never read.

mod catalog;
mod paths;

pub use catalog::CodexCatalog;
pub use paths::{PROJECT_DIR_NAME, codex_home};

use crate::error::BusinessError;
use crate::harness::prelude::*;
use crate::harnesses::pi::PiDialect;
use crate::model::{SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use crate::pi_family::sessions::SessionCache;

/// The Codex adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct Codex;

impl Harness for Codex {
    fn id(&self) -> HarnessId {
        HarnessId::Codex
    }

    fn info(&self) -> HarnessInfo {
        HarnessInfo {
            id: HarnessId::Codex,
            name: "Codex".to_owned(),
            short_name: "CX".to_owned(),
            binary_name: self.binary_name().to_owned(),
            description:
                "OpenAI's terminal coding agent with skills, plugins, MCP servers and hooks, on your own ChatGPT or API login."
                    .to_owned(),
            homepage: "https://developers.openai.com/codex".to_owned(),
        }
    }

    /// Nothing yet: the views, sessions and chat arrive in later steps of the plan.
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths {
        paths::paths(env, project)
    }

    fn config_view(&self, _env: &Env, _project: Option<&Path>) -> Result<ConfigView> {
        Ok(ConfigView {
            harness: HarnessId::Codex,
            sections: Vec::new(),
            notices: Vec::new(),
        })
    }

    fn extensions_view(&self, _env: &Env, _project: Option<&Path>) -> Result<ExtensionsView> {
        Ok(ExtensionsView {
            harness: HarnessId::Codex,
            plugins: Vec::new(),
            marketplaces: Vec::new(),
            skills: Vec::new(),
            prompt_templates: Vec::new(),
            agents: Vec::new(),
            mcp_servers: Vec::new(),
            themes: Vec::new(),
            packages: Vec::new(),
            extension_modules: Vec::new(),
            notices: Vec::new(),
        })
    }

    fn catalog(&self) -> &dyn CommandCatalog {
        &catalog::CATALOG
    }

    /// Chat with Codex is not available yet.
    fn launch_spec(&self, _request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        Err(chat_not_available())
    }

    /// Never reached: chat is off (`capabilities.chat` is false).
    fn dialect(&self) -> &'static dyn RpcDialect {
        &PiDialect
    }

    /// `<home>/sessions`, shared by every project (see [`paths`]).
    fn sessions_dir(&self, env: &Env, _project: &Path) -> PathBuf {
        paths::sessions_dir(env)
    }

    fn list_sessions(&self, _env: &Env, _project: &Path, _cache: &SessionCache) -> Result<Vec<SessionSummary>> {
        Err(sessions_not_available())
    }

    fn resolve_session(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<PathBuf> {
        Err(sessions_not_available())
    }

    fn delete_session(&self, _env: &Env, _project: &Path, _session: &SessionRef, _empty_copy_only: bool) -> Result<()> {
        Err(sessions_not_available())
    }

    fn session_tree(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<SessionTree> {
        Err(sessions_not_available())
    }

    fn session_messages(
        &self,
        _env: &Env,
        _project: &Path,
        _session: &SessionRef,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> Result<SessionMessagesPage> {
        Err(sessions_not_available())
    }

    fn session_files_touched(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<Vec<String>> {
        Err(sessions_not_available())
    }

    /// The npm package, on every OS (never run by the app).
    fn install_hint(&self) -> InstallHint {
        InstallHint::single(
            "npm install -g @openai/codex",
            "https://developers.openai.com/codex/cli",
        )
    }

    /// Nothing yet: the MCP registration through `codex mcp` comes later, and Harness Skills has
    /// no Codex adapter.
    fn companion_setup(&self) -> CompanionSetup {
        CompanionSetup {
            skills: SkillsStrategy::Unsupported {
                reason: "Harness Skills for Codex are not available yet.",
            },
            mcp: McpTarget::Unsupported,
        }
    }

    /// `codex --version` prints `codex-cli 0.154.0`; the first `x.y.z` is the version.
    fn supported_versions(&self) -> &'static [&'static str] {
        &["0.154.", "0.155.", "0.156."]
    }
}

fn chat_not_available() -> BusinessError {
    BusinessError::Unsupported("Codex chat is not available yet".to_owned())
}

fn sessions_not_available() -> BusinessError {
    BusinessError::Unsupported("Codex sessions are not available yet".to_owned())
}
