//! Claude Code (`claude`), following Claude Code 2.1.304 and its documentation
//! ([plan](../../../../docs/plans/claude-code.md)).
//!
//! Everything lives in one config directory (`~/.claude` or `CLAUDE_CONFIG_DIR`, see [`paths`]),
//! next to a global file (`~/.claude.json`) the CLI rewrites while it runs. The harness is detected,
//! its settings are shown ([`config`]) and its plugins, marketplaces, skills, agents, commands and
//! MCP servers are listed ([`extensions`]). Saved sessions are read from Claude's session files
//! ([`sessions`]). Plugin and marketplace actions run `claude plugin …` ([`cli`]). Recommended
//! setup installs the Harness Skills plugin from its marketplace ([`setup`] reads the registries)
//! and registers the Harness family MCP servers in the user scope through `claude mcp`
//! ([`McpTarget::ClaudeCli`]). Chat is not available yet, and the catalog lists every command as
//! unsupported. Every spawn switches off Claude's self-updater.

mod catalog;
mod cli;
mod config;
mod extensions;
mod paths;
mod sessions;
mod setup;

pub use catalog::ClaudeCatalog;
pub use cli::{claude_cli_action, claude_summary};
pub use paths::{PROJECT_DIR_NAME, config_dir, global_config_file, project_slug};
pub use setup::{KnownMarketplace, UserPlugin, known_marketplaces, user_plugins};

use crate::ecosystem::catalog::{CLAUDE_SKILLS_PLUGIN, SKILLS_MARKETPLACE};
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

    /// What the views list and the extension actions the CLI runs. Chat arrives in a later step of
    /// the plan.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            plugins: true,
            marketplaces: true,
            mcp: true,
            skills: true,
            prompt_templates: true,
            agents: true,
            cli_actions: true,
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

    /// The project's `<uuid>.jsonl` session files (see [`sessions`]).
    fn list_sessions(&self, env: &Env, project: &Path, _cache: &SessionCache) -> Result<Vec<SessionSummary>> {
        Ok(sessions::list_sessions(&self.sessions_dir(env, project), project))
    }

    fn resolve_session(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<PathBuf> {
        let id = sessions::checked_id(session)?;

        sessions::find_session(&self.sessions_dir(env, project), id)
    }

    /// Deletes the session file with the session's folders, refused while the session runs in
    /// Claude Code (see [`sessions`]).
    fn delete_session(&self, env: &Env, project: &Path, session: &SessionRef, empty_copy_only: bool) -> Result<()> {
        let id = sessions::checked_id(session)?;

        sessions::delete_session(env, project, id, empty_copy_only)
    }

    fn session_tree(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<SessionTree> {
        sessions::session_tree(&self.resolve_session(env, project, session)?, session)
    }

    fn session_messages(
        &self,
        env: &Env,
        project: &Path,
        session: &SessionRef,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionMessagesPage> {
        let path = self.resolve_session(env, project, session)?;

        sessions::session_messages(&path, cursor, limit)
    }

    fn session_files_touched(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<Vec<String>> {
        sessions::session_files_touched(&self.resolve_session(env, project, session)?, project)
    }

    fn spawn_env(&self) -> &'static [(&'static str, &'static str)] {
        SPAWN_ENV
    }

    fn cli_action(&self, action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
        claude_cli_action(action, project)
    }

    /// The `message` of the `--json` result line.
    fn cli_summary(&self, stdout: &str) -> Option<String> {
        claude_summary(stdout)
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

    /// The Harness Skills plugin from its marketplace (harness-useful-skills README), and the MCP
    /// servers in the user scope through `claude mcp`.
    fn companion_setup(&self) -> CompanionSetup {
        CompanionSetup {
            skills: SkillsStrategy::Marketplace {
                source: SKILLS_MARKETPLACE,
                plugin: CLAUDE_SKILLS_PLUGIN,
            },
            mcp: McpTarget::ClaudeCli,
        }
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["2.1."]
    }
}

fn chat_not_available() -> BusinessError {
    BusinessError::Unsupported("Claude Code chat is not available yet".to_owned())
}
