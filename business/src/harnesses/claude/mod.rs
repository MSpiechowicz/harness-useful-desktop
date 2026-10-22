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
//! ([`McpTarget::ClaudeCli`]). Share setup shares its marketplaces, user plugins and user MCP
//! servers ([`Harness::share_support`], an imported server is added through `claude mcp add-json`).
//! The chat engine runs `claude` in stream-json mode ([`launch`]); chat is a preview. The catalog maps Claude's commands to the chat's actions and views. Every
//! spawn switches off Claude's self-updater.

mod catalog;
mod cli;
mod config;
mod extensions;
mod launch;
mod modes;
mod paths;
mod sessions;
mod setup;

pub use catalog::ClaudeCatalog;
pub use cli::{claude_cli_action, claude_summary};
pub use modes::WIRE_DEFAULT;
pub(crate) use paths::read_global_file;
pub use paths::{MAX_GLOBAL_FILE_BYTES, PROJECT_DIR_NAME, config_dir, global_config_file, project_slug};
pub(crate) use sessions::chat::{SavedMessages, chat_messages, fork_point, session_file};
pub(crate) use sessions::is_session_id;
pub use setup::{
    KnownMarketplace, PluginRecord, PluginScope, UserPlugin, known_marketplaces, plugin_records, user_plugins,
};

use self::config::ProjectTrust;
use crate::ecosystem::catalog::{CLAUDE_SKILLS_PLUGIN, SKILLS_MARKETPLACE};
use crate::error::BusinessError;
use crate::harness::prelude::*;
use crate::harnesses::pi::PiDialect;
use crate::model::{SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use crate::pi_family::sessions::SessionCache;

/// Environment variables set on every spawn of `claude` (see [`Harness::spawn_env`]): its
/// self-updater stays off.
pub const SPAWN_ENV: &[(&str, &str)] = &[("DISABLE_UPDATES", "1")];

/// Why a chat in a folder Claude Code does not trust yet is refused.
const UNTRUSTED: &str = "Claude Code trusts each folder on its own and does not trust this one yet. Run claude in it once from a terminal, or trust it once here.";

/// Why a chat is refused when Claude Code's global file exists but cannot be read.
const UNREADABLE: &str = "Claude Code's settings file could not be read, so this folder counts as untrusted.";

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

    /// What the views list, the extension actions the CLI runs and the chat (a preview). Its
    /// approval modes are Always ask and Write, never Yolo (see [`modes`]). A chat starts only in a
    /// folder Claude Code trusts, or with trust once ([`Harness::check_project_trust`]). Live
    /// slash commands run as prompt text, `/compact` takes instructions, and the CLI's stdio MCP
    /// servers inherit `HARNESS_MEMORY_SESSION`. No session tree: a fork leaves no link to its parent on disk.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            chat: true,
            chat_preview: true,
            slash_commands_via_prompt: true,
            compact_instructions: true,
            memory_chat_guards: true,
            approval_modes: true,
            project_trust: true,
            project_trust_required: true,
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

    /// `claude` in stream-json mode (see [`launch`]).
    fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        launch::claude_launch_spec(request)
    }

    /// Claude Code skips its own trust dialog in stream-json mode, so the project's
    /// `.claude/settings*.json` (hooks, environment, helpers, modes) would apply unasked: a folder
    /// the user never trusted in Claude Code is refused, and so is every folder while the global
    /// file cannot be read. Only the folder's own entry counts. The app never records a trust answer.
    fn check_project_trust(&self, env: &Env, project: &Path) -> Result<()> {
        match config::project_trust(env, project) {
            ProjectTrust::Trusted => Ok(()),
            ProjectTrust::Untrusted => Err(BusinessError::Untrusted(UNTRUSTED.to_owned())),
            ProjectTrust::Unreadable => Err(BusinessError::UntrustedUnreadable(UNREADABLE.to_owned())),
        }
    }

    /// Only a session saved in the project's session folder by this folder is resumed: one of
    /// another folder with the same slug is not found ([`sessions`]).
    fn check_resume(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<()> {
        self.resolve_session(env, project, session).map(|_| ())
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

        sessions::find_project_session(&self.sessions_dir(env, project), project, id)
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

    /// Write for Claude's `default` mode, Yolo for `bypassPermissions`, else none (see [`modes`]).
    fn configured_approval_mode(&self, env: &Env, project: Option<&Path>) -> Option<ApprovalMode> {
        modes::approval_mode(&modes::native_approval_mode(env, project).name)
    }

    /// `permissions.defaultMode` from the settings layers (see [`modes`]).
    fn native_approval_mode(&self, env: &Env, project: Option<&Path>) -> Option<NativeApprovalMode> {
        Some(modes::native_approval_mode(env, project))
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

    /// Marketplaces, user plugins and the user scope's MCP servers (see [`crate::sync`]).
    fn share_support(&self) -> ShareSupport {
        ShareSupport {
            marketplaces: true,
            plugins: true,
            packages: false,
            mcp_servers: true,
        }
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["2.1."]
    }
}
