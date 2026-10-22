//! Codex (`codex`), following the Codex CLI 0.154 ([plan](../../../../docs/plans/codex.md)).
//!
//! Everything lives in one home directory (`~/.codex` or `CODEX_HOME`, see [`paths`]): the
//! `config.toml` settings, skills, plugins and the sessions of every project. The harness is
//! detected, its settings are shown ([`config`], with the configured mode of [`modes`]) and its
//! plugins, marketplaces, skills and MCP servers are listed ([`extensions`]). Plugin and
//! marketplace actions run `codex plugin …` ([`cli`]). Recommended setup registers the Harness
//! family MCP servers through `codex mcp` ([`McpTarget::CodexCli`]); Harness Skills have no Codex
//! adapter yet. Share setup shares its marketplaces, plugins and MCP servers
//! ([`Harness::share_support`], an imported server is added through `codex mcp add`). Sessions
//! are read through `codex app-server` ([`sessions`]). Chat (a preview) runs `codex app-server`
//! too ([`launch`], [`ChatProtocol::CodexAppServer`]) with the app's approval modes and trust
//! once ([`trust`]); the catalog maps Codex's commands to the chat's actions and views and adds
//! the skills ([`catalog`]). The login store (`auth.json`) and Codex's private files are never
//! read ([`crate::security::deny`]).

mod catalog;
mod cli;
mod config;
mod extensions;
mod launch;
mod modes;
mod paths;
pub mod sessions;
mod settings;
mod trust;

pub use catalog::CodexCatalog;
pub use cli::codex_cli_action;
pub use extensions::{MarketplaceRecord, PluginRecord, marketplace_records, plugin_records};
pub use launch::{APP_SERVER_ARGS, codex_launch_spec, is_thread_id};
pub use paths::{PROJECT_DIR_NAME, codex_home};
pub use settings::{CONFIG_FILE, user_config_path};
pub use trust::{ProjectTrust, project_trust};

use self::modes::ConfiguredMode;
use crate::error::BusinessError;
use crate::harness::prelude::*;
use crate::harnesses::pi::PiDialect;

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

    /// What the views list, the extension actions the CLI runs and the chat (a preview), whose
    /// queued messages can be withdrawn and whose skills run as prompt text. Codex compacts with
    /// its own prompt, so `/compact` takes no instructions. No session tree: Codex 0.154's
    /// `thread/list` leaves `forkedFromId` out (only `thread/read` has it), so forks are not
    /// counted. Its approval modes are Always ask, Write and Yolo, never Auto (see [`modes`]). A
    /// chat may trust the project once ([`trust`]), but no folder is refused: Codex itself ignores
    /// the own layer of a folder it does not trust, so nothing of the project applies unasked
    /// (`project_trust_required` stays off).
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            chat: true,
            chat_preview: true,
            slash_commands_via_prompt: true,
            remove_queued: true,
            approval_modes: true,
            yolo_mode: true,
            project_trust: true,
            plugins: true,
            marketplaces: true,
            mcp: true,
            skills: true,
            cli_actions: true,
            ..Capabilities::default()
        }
    }

    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths {
        paths::paths(env, project)
    }

    fn catalog(&self) -> &dyn CommandCatalog {
        &catalog::CATALOG
    }

    /// `config.toml` of the user and the project (see [`config`]).
    fn config_view(&self, env: &Env, project: Option<&Path>) -> Result<ConfigView> {
        Ok(config::config_view(env, project))
    }

    /// Plugins, marketplaces, skills and MCP servers (see [`extensions`]).
    fn extensions_view(&self, env: &Env, project: Option<&Path>) -> Result<ExtensionsView> {
        Ok(extensions::extensions_view(env, project))
    }

    /// `codex app-server` in the project (see [`launch`]).
    fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        launch::codex_launch_spec(request)
    }

    /// Never reached: Codex chats over [`ChatProtocol::CodexAppServer`], not RPC.
    fn dialect(&self) -> &'static dyn RpcDialect {
        &PiDialect
    }

    fn chat_protocol(&self) -> ChatProtocol {
        ChatProtocol::CodexAppServer
    }

    /// `<home>/sessions`, shared by every project (see [`paths`]). Shown only: the sessions are
    /// read through the app-server ([`sessions`]).
    fn sessions_dir(&self, env: &Env, _project: &Path) -> PathBuf {
        paths::sessions_dir(env)
    }

    fn list_sessions(&self, _env: &Env, _project: &Path, _cache: &SessionCache) -> Result<Vec<SessionSummary>> {
        Err(read_through_app_server())
    }

    fn resolve_session(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<PathBuf> {
        Err(read_through_app_server())
    }

    fn delete_session(&self, _env: &Env, _project: &Path, _session: &SessionRef, _empty_copy_only: bool) -> Result<()> {
        Err(read_through_app_server())
    }

    fn session_tree(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<SessionTree> {
        Err(read_through_app_server())
    }

    fn session_messages(
        &self,
        _env: &Env,
        _project: &Path,
        _session: &SessionRef,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> Result<SessionMessagesPage> {
        Err(read_through_app_server())
    }

    fn session_files_touched(&self, _env: &Env, _project: &Path, _session: &SessionRef) -> Result<Vec<String>> {
        Err(read_through_app_server())
    }

    /// List, transcript, touched files and delete through `codex app-server` (see [`sessions`]).
    fn cli_sessions(&self) -> Option<&dyn CliSessions> {
        Some(sessions::cli_sessions())
    }

    /// Always ask for `untrusted`, Write for `on-request`, both in a `workspace-write` sandbox, and
    /// Yolo for `never` with `danger-full-access`; else none (see [`modes`]).
    fn configured_approval_mode(&self, env: &Env, project: Option<&Path>) -> Option<ApprovalMode> {
        ConfiguredMode::load(env, project).approval_mode()
    }

    /// `approval_policy / sandbox_mode` from the settings layers (see [`modes`]).
    fn native_approval_mode(&self, env: &Env, project: Option<&Path>) -> Option<NativeApprovalMode> {
        Some(ConfiguredMode::load(env, project).native())
    }

    fn cli_action(&self, action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
        codex_cli_action(action, project)
    }

    /// The npm package, on every OS (never run by the app).
    fn install_hint(&self) -> InstallHint {
        InstallHint::single(
            "npm install -g @openai/codex",
            "https://developers.openai.com/codex/cli",
        )
    }

    /// The MCP servers in the user's `config.toml` through `codex mcp`. Harness Skills has no
    /// Codex adapter yet (plan Decision 6), so no skills row.
    fn companion_setup(&self) -> CompanionSetup {
        CompanionSetup {
            skills: SkillsStrategy::Unsupported {
                reason: "Harness Skills for Codex are not available yet.",
            },
            mcp: McpTarget::CodexCli,
        }
    }

    /// Marketplaces, plugins and the MCP servers of `config.toml` (see [`crate::sync`]).
    fn share_support(&self) -> ShareSupport {
        ShareSupport {
            marketplaces: true,
            plugins: true,
            packages: false,
            mcp_servers: true,
        }
    }

    /// `codex --version` prints `codex-cli 0.154.0`; the first `x.y.z` is the version.
    fn supported_versions(&self) -> &'static [&'static str] {
        &["0.154.", "0.155.", "0.156."]
    }
}

/// Codex has no session files the app reads; its sessions go through [`Harness::cli_sessions`].
fn read_through_app_server() -> BusinessError {
    BusinessError::Unsupported("Codex sessions are read through its app-server".to_owned())
}
