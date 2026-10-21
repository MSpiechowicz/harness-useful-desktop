//! OpenCode (`opencode`, `opencode-ai`), following the OpenCode sources (1.19.3) and documentation.
//! OpenCode 2 (`@opencode/cli`, 2.1.x) uses the same binary name: detection notes the version and
//! [`OpenCode::generation`] picks the behaviour of each generation ([`OpenCodeGeneration`]).
//!
//! Config and data follow the XDG directories (see [`paths`]). OpenCode keeps its sessions in a
//! SQLite database that also holds credentials, so the app never opens it: sessions are listed,
//! exported and deleted through the CLI ([`Harness::cli_sessions`]). Chat runs one
//! `opencode serve` per chat ([`ChatProtocol::OpenCodeServer`], see [`launch`]), not the
//! PI-family RPC, and is marked as a preview. Extension actions install npm plugins through
//! `opencode plugin <module> -g` ([`cli`]) and remove them by editing the global config file
//! ([`plugin_remove`]), since OpenCode has no uninstall command. "Share setup" shares the npm
//! plugins and MCP servers of the global config file ([`share`]), checking the names OpenCode merges
//! from all its layers before writing a server ([`merged`]). Every spawn switches off
//! OpenCode's self-updater.

mod catalog;
mod cli;
mod config;
mod extensions;
mod launch;
mod merged;
mod paths;
mod plugin_ids;
mod plugin_remove;
mod sessions;
mod setup;
mod share;

pub use catalog::OpenCodeCatalog;
pub use launch::opencode_launch_spec;
pub use merged::{PROBE_ARGS, PROBE_MAX_BYTES, PROBE_TIMEOUT, merged_mcp_names};
pub use paths::{PROJECT_DIR_NAME, config_dir, custom_config_dir, data_dir, home_config_dir};
pub use sessions::OpenCodeSessions;
pub(crate) use share::{PluginEntry, global_plugin_entries, user_npm_plugins};

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::error::BusinessError;
use crate::harness::prelude::*;
use crate::harnesses::pi::PiDialect;
use crate::model::{SessionMessagesPage, SessionRef, SessionSummary, SessionTree};
use crate::pi_family::sessions::SessionCache;

/// Environment variables set on every spawn of `opencode` (see [`Harness::spawn_env`]): its
/// self-updater stays off.
pub const SPAWN_ENV: &[(&str, &str)] = &[("OPENCODE_DISABLE_AUTOUPDATE", "1")];

/// The OpenCode adapter. Clones share the generation, which detection updates
/// ([`Harness::note_version`]); until then it is [`OpenCodeGeneration::V1`].
#[derive(Debug, Clone, Default)]
pub struct OpenCode {
    generation: Arc<AtomicU8>,
}

impl OpenCode {
    /// An adapter fixed to `generation` until the next detection (tests, callers that know it).
    pub fn with_generation(generation: OpenCodeGeneration) -> Self {
        Self {
            generation: Arc::new(AtomicU8::new(generation.code())),
        }
    }

    /// The generation the last detection found ([`OpenCodeGeneration::V1`] before any).
    pub fn generation(&self) -> OpenCodeGeneration {
        OpenCodeGeneration::from_code(self.generation.load(Ordering::Relaxed))
    }
}

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
            chat: true,
            approval_modes: true,
            slash_commands_via_prompt: true,
            remove_queued: true,
            chat_preview: true,
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

    /// `opencode serve` on a loopback port (see [`launch`]).
    fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec> {
        opencode_launch_spec(request)
    }

    /// Never reached for chat: OpenCode chats over [`ChatProtocol::OpenCodeServer`], not RPC.
    fn dialect(&self) -> &'static dyn RpcDialect {
        &PiDialect
    }

    fn chat_protocol(&self) -> ChatProtocol {
        match self.generation() {
            OpenCodeGeneration::V1 => ChatProtocol::OpenCodeServer,
            OpenCodeGeneration::V2 => ChatProtocol::OpenCodeServerV2,
        }
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

    /// `None`: OpenCode's own permission settings apply without an override, and the app does
    /// not read them (the UI names "OpenCode settings" as the source).
    fn configured_approval_mode(&self, _env: &Env, _project: Option<&Path>) -> Option<ApprovalMode> {
        None
    }

    fn cli_sessions(&self) -> Option<&dyn CliSessions> {
        Some(sessions::cli_sessions())
    }

    /// `opencode plugin <module> -g` (see [`cli`]).
    fn cli_action(&self, action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
        cli::opencode_cli_action(action, project)
    }

    /// Plugin removal edits the global config file (see [`plugin_remove`]); every other action goes
    /// to the CLI.
    fn config_action(
        &self,
        env: &Env,
        action: &ExtensionAction,
        project: Option<&Path>,
        dry_run: bool,
    ) -> Result<Option<CliResult>> {
        match action {
            ExtensionAction::PluginUninstall { name } => {
                plugin_remove::remove_plugin(env, name, project, dry_run).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn spawn_env(&self) -> &'static [(&'static str, &'static str)] {
        SPAWN_ENV
    }

    /// OpenCode 1 from its install script (`npm` on Windows), OpenCode 2 from npm only: the
    /// script installs OpenCode 1 releases and refuses 2.x.
    fn install_hint(&self) -> InstallHint {
        let command = if self.generation() == OpenCodeGeneration::V2 {
            "npm install -g @opencode/cli"
        } else if cfg!(windows) {
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

    /// The npm plugins and the MCP servers of the global config file (see [`share`]).
    fn share_support(&self) -> ShareSupport {
        ShareSupport {
            marketplaces: false,
            plugins: true,
            packages: false,
            mcp_servers: true,
        }
    }

    fn supported_versions(&self) -> &'static [&'static str] {
        &["1.18.", "1.19.", "2.1."]
    }

    fn note_version(&self, version: Option<&str>) {
        let generation = version.map_or(OpenCodeGeneration::V1, OpenCodeGeneration::from_version);

        self.generation.store(generation.code(), Ordering::Relaxed);
    }
}

/// OpenCode has no session files; its sessions go through [`Harness::cli_sessions`].
fn no_session_files() -> BusinessError {
    BusinessError::Unsupported("OpenCode sessions are read through its CLI".to_owned())
}
