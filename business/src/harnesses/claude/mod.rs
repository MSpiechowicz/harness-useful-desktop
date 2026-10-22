//! Claude Code (`claude`), following Claude Code 2.1.304 and its documentation
//! ([plan](../../../../docs/plans/claude-code.md)).
//!
//! Everything lives in one config directory (`~/.claude` or `CLAUDE_CONFIG_DIR`, see [`paths`]),
//! next to a global file (`~/.claude.json`) the CLI rewrites while it runs. The harness is detected
//! and its settings are shown ([`config`]). The extension view is empty, sessions and chat are not
//! available yet, and the catalog lists every command as unsupported. Every spawn switches off
//! Claude's self-updater.

mod catalog;
mod config;
mod paths;

pub use catalog::ClaudeCatalog;
pub use paths::{PROJECT_DIR_NAME, config_dir, global_config_file, project_slug};

use crate::error::BusinessError;
use crate::harness::prelude::*;
use crate::harnesses::pi::PiDialect;
use crate::model::{SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use crate::pi_family::sessions::SessionCache;

/// Environment variables set on every spawn of `claude` (see [`Harness::spawn_env`]): its
/// self-updater stays off.
pub const SPAWN_ENV: &[(&str, &str)] = &[("DISABLE_UPDATES", "1")];

/// The Claude Code adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct Claude;

impl Harness for Claude {
    fn id(&self) -> HarnessId {
        HarnessId::Claude
    }

    fn info(&self) -> HarnessInfo {
        HarnessInfo {
            id: HarnessId::Claude,
            name: "Claude Code".to_owned(),
            short_name: "CC".to_owned(),
            binary_name: self.binary_name().to_owned(),
            description:
                "Anthropic's terminal coding agent with plugins, skills, subagents and hooks, on your own Claude login."
                    .to_owned(),
            homepage: "https://code.claude.com".to_owned(),
        }
    }

    /// Nothing yet: the views, sessions and chat arrive in later steps of the plan.
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths {
        paths::paths(env, project)
    }

    fn config_view(&self, env: &Env, project: Option<&Path>) -> Result<ConfigView> {
        Ok(config::config_view(env, project))
    }

    fn extensions_view(&self, _env: &Env, _project: Option<&Path>) -> Result<ExtensionsView> {
        Ok(ExtensionsView {
            harness: HarnessId::Claude,
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

    /// Chat with Claude Code is not available yet.
    fn launch_spec(&self, _request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        Err(chat_not_available())
    }

    /// Never reached: Claude chats over [`ChatProtocol::ClaudeStream`], not RPC.
    fn dialect(&self) -> &'static dyn RpcDialect {
        &PiDialect
    }

    fn chat_protocol(&self) -> ChatProtocol {
        ChatProtocol::ClaudeStream
    }

    /// `<config>/projects/<slug>` (see [`paths`]).
    fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf {
        paths::sessions_dir(env, project)
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

    fn spawn_env(&self) -> &'static [(&'static str, &'static str)] {
        SPAWN_ENV
    }

    /// The native installer script for the OS (never run by the app).
    fn install_hint(&self) -> InstallHint {
        let command = if cfg!(windows) {
            "irm https://claude.ai/install.ps1 | iex"
        } else {
            "curl -fsSL https://claude.ai/install.sh | bash"
        };

        InstallHint::single(command, "https://code.claude.com/docs/en/setup")
    }

    /// Nothing yet: the Harness Skills plugin and the MCP registration through the CLI come later.
    fn companion_setup(&self) -> CompanionSetup {
        CompanionSetup {
            skills: SkillsStrategy::Unsupported {
                reason: "Harness Skills for Claude Code are not available yet.",
            },
            mcp: McpTarget::Unsupported,
        }
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["2.1."]
    }
}

fn chat_not_available() -> BusinessError {
    BusinessError::Unsupported("Claude Code chat is not available yet".to_owned())
}

fn sessions_not_available() -> BusinessError {
    BusinessError::Unsupported("Claude Code sessions are not available yet".to_owned())
}
