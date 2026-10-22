//! The harness abstraction: one trait that every coding-agent CLI adapter implements.
//!
//! [`Harness`] works like an abstract class: adapters implement the required methods (identity,
//! paths, config and extensions views, command catalog) and inherit shared behaviour from default
//! methods (detection, summaries). Shared parsing for CLIs that use the PI file formats lives in
//! [`crate::pi_family`] and is reused by composition, never copied between adapters.
//!
//! # Adding a harness
//!
//! 1. Add a [`HarnessId`] variant.
//! 2. Create `harnesses/<name>/` with a type implementing [`Harness`] and a [`CommandCatalog`].
//! 3. Add one line to [`HarnessRegistry::with_defaults`].
//!
//! The UI renders most of a harness from [`HarnessInfo`], [`Capabilities`] and the views, but it
//! still has per-harness touch points (badge labels, colour tokens, mock fixtures): see
//! `docs/adding-a-harness.md`.
//!
//! Chat sessions additionally need [`Harness::launch_spec`] (the command line) and
//! [`Harness::chat_protocol`] (which chat engine drives the process; for the PI-family RPC
//! protocol, [`Harness::dialect`] says how the harness speaks it). Saved sessions need
//! [`Harness::sessions_dir`] (and [`Harness::session_store`] for the machine-wide totals);
//! listing, trees and transcripts default to the PI-family session format and are overridden by
//! harnesses that store sessions differently, or replaced by [`Harness::cli_sessions`] for a
//! harness whose sessions are only reachable through its CLI.
//! Extension actions (install, remove, …) need [`Harness::cli_action`], run by [`run_cli_action`].

mod catalog;
mod cli;
mod companion;
mod detect;
mod launch;
mod opencode_generation;
mod registry;

pub use catalog::{
    Builtin, CommandCatalog, SharedCommand, builtin_specs, gui, merge_commands, parse_live_commands, rpc, spec,
    unsupported,
};
pub(crate) use cli::clean;
pub use cli::{
    CLI_ACTION_TIMEOUT, CliInvocation, CliRun, HarnessCli, MAX_CLI_TARGET_CHARS, TargetKind, cli_target,
    optional_cli_target, run_cli_action,
};
pub use companion::{CompanionSetup, McpTarget, SkillsStrategy};
pub use detect::{VERSION_TIMEOUT, first_semver, override_name_matches};
pub use launch::{LaunchRequest, flag_with, rpc_exec_spec, utility_args};
pub use opencode_generation::OpenCodeGeneration;
pub use registry::HarnessRegistry;

/// Everything an adapter's `impl Harness` block needs: `use crate::harness::prelude::*;`.
pub mod prelude {
    pub use std::path::Path;

    pub use std::path::PathBuf;

    pub use super::{
        ChatProtocol, CliInvocation, CliSessionCtx, CliSessions, CommandCatalog, CompanionSetup, Harness, HarnessPaths,
        LaunchRequest, McpTarget, OpenCodeGeneration, ShareSupport, SkillsStrategy,
    };
    pub use crate::env::Env;
    pub use crate::error::Result;
    pub use crate::model::{
        ApprovalMode, Capabilities, CliResult, ConfigView, ExtensionAction, ExtensionsView, HarnessId, HarnessInfo,
        InstallHint, NativeApprovalMode, SessionFolder,
    };
    pub use crate::pi_family::rpc::RpcDialect;
    pub use crate::pi_family::sessions::SessionStore;
    pub use crate::process::ExecSpec;
}

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    ApprovalMode, Capabilities, CliResult, ConfigView, Detection, ExtensionAction, ExtensionsView, HarnessId,
    HarnessInfo, HarnessSummary, InstallHint, NativeApprovalMode, SessionFolder, SessionMessagesPage, SessionRef,
    SessionSummary, SessionTotals, SessionTree, SyncItemKind, VersionSupport,
};
use crate::pi_family::rpc::RpcDialect;
use crate::pi_family::sessions::{self, SessionCache};
use crate::process::{BlockingRunner, BoxFuture, ExecSpec, ProcessRunner, ResolvedBinary};

/// How the app talks to a harness's chat process: which engine of [`crate::agent`] drives it.
#[derive(Clone, Copy)]
pub enum ChatProtocol {
    /// The PI-family RPC protocol on stdin/stdout (`--mode rpc`), spoken in this dialect.
    Rpc(&'static dyn RpcDialect),
    /// An `opencode serve` HTTP server of OpenCode 1 on a loopback port, with its event stream.
    OpenCodeServer,
    /// An `opencode serve` HTTP server of OpenCode 2 (its `/api` routes) on a loopback port.
    OpenCodeServerV2,
    /// Claude Code's stream-json mode with its control protocol on stdin/stdout.
    ClaudeStream,
}

/// Where a harness keeps its files, resolved from an [`Env`] (and the project, if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessPaths {
    /// Root of everything the harness stores for this user (`~/.omp`, `~/.pi`). Reported by
    /// detection and used as a reveal/containment root.
    pub config_root: PathBuf,
    /// User-level agent directory holding settings, skills and themes (profile-aware for OMP).
    pub agent_dir: PathBuf,
    /// Base directory of installed plugins and marketplace data (may be an XDG data directory).
    pub data_root: PathBuf,
    /// The active configuration profile, if the harness supports profiles and one is selected.
    pub profile: Option<String>,
    /// The project-local configuration directory (`<project>/.omp`, `<project>/.pi`).
    pub project_dir: Option<PathBuf>,
}

/// Which kinds of items "Share setup" exports from a harness and imports into it (see
/// [`crate::sync`]). The default shares nothing, so a new harness opts in kind by kind; MCP servers
/// also need a [`McpTarget::UserMcpJson`] or [`McpTarget::OpenCodeConfig`] in
/// [`Harness::companion_setup`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShareSupport {
    /// Plugin marketplaces (OMP's marketplace registry).
    pub marketplaces: bool,
    /// Plugins installed in the user scope (OMP's plugin registry and npm plugins, OpenCode's npm
    /// plugins of the global config file).
    pub plugins: bool,
    /// Packages of the user settings (PI's `packages`).
    pub packages: bool,
    /// Servers of the user MCP file.
    pub mcp_servers: bool,
}

impl ShareSupport {
    /// Shares nothing.
    pub const NONE: Self = Self {
        marketplaces: false,
        plugins: false,
        packages: false,
        mcp_servers: false,
    };

    /// True when items of `kind` are shared.
    pub fn supports(self, kind: SyncItemKind) -> bool {
        match kind {
            SyncItemKind::Marketplace => self.marketplaces,
            SyncItemKind::Plugin => self.plugins,
            SyncItemKind::Package => self.packages,
            SyncItemKind::McpServer => self.mcp_servers,
        }
    }
}

/// What a [`CliSessions`] method needs to run the harness CLI for one project.
#[derive(Clone, Copy)]
pub struct CliSessionCtx<'a> {
    /// The environment, for paths and the child's search directories.
    pub env: &'a Env,
    /// The canonical project directory; the CLI runs in it.
    pub project: &'a Path,
    /// The detected harness executable.
    pub binary: &'a ResolvedBinary,
    /// Runs the CLI (blocking: these methods are called from blocking tasks).
    pub runner: &'a dyn BlockingRunner,
}

/// Saved sessions of a harness that exposes them only through its CLI (list, export, delete
/// commands) instead of session files the app reads. Every spawn carries
/// [`Harness::spawn_env`]. Sessions are linear: there is no tree, resume or fork.
pub trait CliSessions: Send + Sync {
    /// Saved sessions of the project, newest first.
    fn list(&self, ctx: &CliSessionCtx<'_>) -> Result<Vec<SessionSummary>>;

    /// One page of the transcript of `session` (the cursor format is the same as
    /// [`sessions::session_messages`]).
    fn messages(
        &self,
        ctx: &CliSessionCtx<'_>,
        session: &SessionRef,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionMessagesPage>;

    /// Project-relative files the edit tools of `session` may have changed.
    fn files_touched(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<Vec<String>>;

    /// Deletes `session`.
    fn delete(&self, ctx: &CliSessionCtx<'_>, session: &SessionRef) -> Result<()>;
}

/// A coding-agent CLI adapter. See the module docs for how to add one.
pub trait Harness: Send + Sync {
    /// Stable identifier.
    fn id(&self) -> HarnessId;

    /// Static description shown in onboarding and the harness switcher.
    fn info(&self) -> HarnessInfo;

    /// Feature flags that decide which tabs and actions the UI renders.
    fn capabilities(&self) -> Capabilities;

    /// Executable name looked up on `PATH`.
    fn binary_name(&self) -> &'static str {
        self.id().binary_name()
    }

    /// Resolves the harness directories for `env` and an optional project.
    fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths;

    /// The readable, redacted configuration (user layer first, then profiles and project).
    fn config_view(&self, env: &Env, project: Option<&Path>) -> Result<ConfigView>;

    /// Everything installed into the harness: plugins, skills, MCP servers, themes, ...
    fn extensions_view(&self, env: &Env, project: Option<&Path>) -> Result<ExtensionsView>;

    /// The slash-command catalog.
    fn catalog(&self) -> &dyn CommandCatalog;

    /// The shell-free command line that runs the harness in RPC mode for `request`.
    fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec>;

    /// The RPC dialect the harness speaks in `--mode rpc`.
    fn dialect(&self) -> &'static dyn RpcDialect;

    /// The protocol of the process [`Harness::launch_spec`] starts (default: RPC in
    /// [`Harness::dialect`]).
    fn chat_protocol(&self) -> ChatProtocol {
        ChatProtocol::Rpc(self.dialect())
    }

    /// The folder holding the saved sessions of `project` (a canonical project directory): the
    /// harness's encoded per-project folder or a configured session directory.
    fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf;

    /// [`Harness::sessions_dir`] for display, and whether the project's own settings chose it
    /// (default: never; harnesses with a project-level session folder setting override this).
    fn session_folder(&self, env: &Env, project: &Path) -> SessionFolder {
        SessionFolder {
            path: self.sessions_dir(env, project).to_string_lossy().into_owned(),
            configured: false,
        }
    }

    /// Saved sessions of `project`, newest first (default: PI-family session files, see
    /// [`crate::pi_family::sessions`]). A missing folder lists nothing.
    fn list_sessions(&self, env: &Env, project: &Path, cache: &SessionCache) -> Result<Vec<SessionSummary>> {
        Ok(sessions::list_sessions(
            self.id(),
            &self.sessions_dir(env, project),
            cache,
        ))
    }

    /// Where the session files of every project live (default: `Unsupported`, no session files).
    fn session_store(&self, _env: &Env) -> Result<sessions::SessionStore> {
        Err(BusinessError::Unsupported(format!(
            "{} has no session files",
            self.info().short_name
        )))
    }

    /// The session count and last use across every project, from [`Harness::session_store`] (see
    /// [`sessions::session_totals`]); `Unsupported` without session files (CLI sessions included).
    fn session_totals(&self, env: &Env) -> Result<SessionTotals> {
        let store = self.session_store(env)?;

        Ok(sessions::session_totals(&store, sessions::MAX_TOTALS_ENTRIES))
    }

    /// The session file `session` names, looked up only in `project`'s session folder.
    fn resolve_session(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<PathBuf> {
        own_session(self.id(), session)?;
        sessions::find_session(&self.sessions_dir(env, project), &session.id)
    }

    /// Deletes a saved session of `project` with its companion directory and backups (see
    /// [`sessions::delete_session`]); with `empty_copy_only`, only an empty copy is deleted.
    fn delete_session(&self, env: &Env, project: &Path, session: &SessionRef, empty_copy_only: bool) -> Result<()> {
        own_session(self.id(), session)?;
        sessions::delete_session(&self.sessions_dir(env, project), &session.id, empty_copy_only)
    }

    /// The branch tree of a saved session.
    fn session_tree(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<SessionTree> {
        sessions::session_tree(&self.resolve_session(env, project, session)?, session)
    }

    /// One page of the active-path transcript of a saved session (see
    /// [`sessions::session_messages`] for the cursor).
    fn session_messages(
        &self,
        env: &Env,
        project: &Path,
        session: &SessionRef,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionMessagesPage> {
        sessions::session_messages(&self.resolve_session(env, project, session)?, cursor, limit)
    }

    /// Project-relative files the edit tools of a saved session may have changed.
    fn session_files_touched(&self, env: &Env, project: &Path, session: &SessionRef) -> Result<Vec<String>> {
        sessions::session_files_touched(&self.resolve_session(env, project, session)?, project)
    }

    /// The CLI-backed sessions of a harness without readable session files; when `Some`, they
    /// replace the session file methods above. Default: `None`.
    fn cli_sessions(&self) -> Option<&dyn CliSessions> {
        None
    }

    /// Environment variables set on every spawn of the harness binary (version probe, CLI actions,
    /// chat launch, CLI sessions), e.g. to switch off a self-updater. Default: none.
    fn spawn_env(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The approval mode the user configured for `project` (harnesses with
    /// `capabilities.approvalModes`); `None` when the harness has no approval modes or its own
    /// settings decide without one (OpenCode).
    fn configured_approval_mode(&self, _env: &Env, _project: Option<&Path>) -> Option<ApprovalMode> {
        None
    }

    /// The harness's own permission mode for `project` (the user's layers without one), by the
    /// harness's name for it (Claude Code `permissions.defaultMode`). Default: `None`.
    fn native_approval_mode(&self, _env: &Env, _project: Option<&Path>) -> Option<NativeApprovalMode> {
        None
    }

    /// The CLI arguments of an extension action (see [`run_cli_action`]). `project` is the
    /// authorized project directory when the UI acts on the project layer (the CLI then runs in
    /// it). Default: every action is unsupported.
    fn cli_action(&self, action: &ExtensionAction, project: Option<&Path>) -> Result<CliInvocation> {
        let _ = (action, project);

        Err(BusinessError::Unsupported(format!(
            "{} has no extension actions",
            self.info().short_name
        )))
    }

    /// Applies `action` by editing the harness's own config file instead of running its CLI, or
    /// previews that edit when `dry_run` is set (nothing is written). `Ok(None)`: the action goes
    /// to the CLI ([`Harness::cli_action`]); `extension_action` asks this first. Reads and writes
    /// files, so call it from a blocking-capable context. Default: every action goes to the CLI.
    fn config_action(
        &self,
        env: &Env,
        action: &ExtensionAction,
        project: Option<&Path>,
        dry_run: bool,
    ) -> Result<Option<CliResult>> {
        let _ = (env, action, project, dry_run);

        Ok(None)
    }

    /// The output line that reports a failed extension action although the CLI exited with 0
    /// (`stdout` and `stderr` are already cleaned and redacted). Default: the exit code decides.
    fn cli_failure_line(&self, _stdout: &str, _stderr: &str) -> Option<String> {
        None
    }

    /// The summary of a successful extension action from its `stdout` (already cleaned and
    /// redacted), for a CLI whose last line is not meant for people (a JSON result line). Default:
    /// the last line.
    fn cli_summary(&self, _stdout: &str) -> Option<String> {
        None
    }

    /// Copyable install instructions shown when the CLI is missing.
    fn install_hint(&self) -> InstallHint;

    /// How the "Recommended setup" step installs the skills and registers the dashboard's MCP
    /// server for this harness.
    fn companion_setup(&self) -> CompanionSetup;

    /// Which kinds of items "Share setup" exports from and imports into this harness. Default:
    /// nothing.
    fn share_support(&self) -> ShareSupport {
        ShareSupport::NONE
    }

    /// Arguments that make the CLI print its version and exit.
    fn version_args(&self) -> &'static [&'static str] {
        &["--version"]
    }

    /// Extracts the version from `--version` output (default: the first `x.y.z`).
    fn parse_version(&self, output: &str) -> Option<String> {
        first_semver(output)
    }

    /// Version prefixes this app was tested with (e.g. `"18."`); empty means none verified yet.
    fn supported_versions(&self) -> &'static [&'static str] {
        &[]
    }

    /// Called by [`Harness::detect`] with the version the probe read (`None` when it read none or
    /// the binary is missing), so a harness whose behaviour depends on its version (OpenCode 1 or
    /// 2) can pick it. Default: nothing.
    fn note_version(&self, version: Option<&str>) {
        let _ = version;
    }

    /// The OpenCode generation the last detection found; `None` for every harness but OpenCode.
    fn opencode_generation(&self) -> Option<OpenCodeGeneration> {
        None
    }

    /// How `version` relates to [`Harness::supported_versions`].
    fn version_support(&self, version: &str) -> VersionSupport {
        let tested = self
            .supported_versions()
            .iter()
            .any(|prefix| version.starts_with(prefix));

        if tested {
            VersionSupport::Supported
        } else {
            VersionSupport::Untested
        }
    }

    /// Probes the machine for the CLI. `override_path` is the user's chosen binary; it is honoured
    /// only when its file name is [`Harness::binary_name`] (see [`override_name_matches`]).
    fn detect<'a>(
        &'a self,
        env: &'a Env,
        runner: &'a dyn ProcessRunner,
        override_path: Option<&'a Path>,
    ) -> BoxFuture<'a, Detection> {
        Box::pin(detect::detect(self, env, runner, override_path))
    }

    /// Info, capabilities and a fresh detection in one value.
    fn summary<'a>(
        &'a self,
        env: &'a Env,
        runner: &'a dyn ProcessRunner,
        override_path: Option<&'a Path>,
    ) -> BoxFuture<'a, HarnessSummary> {
        Box::pin(async move {
            HarnessSummary {
                info: self.info(),
                capabilities: self.capabilities(),
                detection: self.detect(env, runner, override_path).await,
                native_approval_mode: self.native_approval_mode(env, None),
            }
        })
    }
}

/// Fails with `InvalidInput` when `session` belongs to a harness other than `harness`.
fn own_session(harness: HarnessId, session: &SessionRef) -> Result<()> {
    if session.harness != harness {
        return Err(BusinessError::InvalidInput(
            "the session belongs to another harness".to_owned(),
        ));
    }

    Ok(())
}
