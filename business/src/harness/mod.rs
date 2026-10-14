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
//! Chat sessions additionally need [`Harness::launch_spec`] (the RPC command line) and
//! [`Harness::dialect`] (how the harness speaks the PI-family RPC protocol). Saved sessions need
//! [`Harness::sessions_dir`]; listing, trees and transcripts default to the PI-family session
//! format and are overridden by harnesses that store sessions differently. Extension actions
//! (install, remove, …) need [`Harness::cli_action`], run by [`run_cli_action`].

mod catalog;
mod cli;
mod detect;
mod launch;
mod registry;

pub use catalog::{CommandCatalog, SharedCommand, gui, merge_commands, parse_live_commands, rpc, spec, unsupported};
pub use cli::{
    CLI_ACTION_TIMEOUT, CliInvocation, CliRun, MAX_CLI_TARGET_CHARS, TargetKind, cli_target, optional_cli_target,
    run_cli_action,
};
pub use detect::{VERSION_TIMEOUT, first_semver, override_name_matches};
pub use launch::{LaunchRequest, flag_with, rpc_exec_spec, utility_args};
pub use registry::HarnessRegistry;

/// Everything an adapter's `impl Harness` block needs: `use crate::harness::prelude::*;`.
pub mod prelude {
    pub use std::path::Path;

    pub use std::path::PathBuf;

    pub use super::{CliInvocation, CommandCatalog, Harness, HarnessPaths, LaunchRequest};
    pub use crate::env::Env;
    pub use crate::error::Result;
    pub use crate::model::{
        ApprovalMode, Capabilities, ConfigView, ExtensionAction, ExtensionsView, HarnessId, HarnessInfo, InstallHint,
    };
    pub use crate::pi_family::rpc::RpcDialect;
    pub use crate::process::ExecSpec;
}

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    ApprovalMode, Capabilities, ConfigView, Detection, ExtensionAction, ExtensionsView, HarnessId, HarnessInfo,
    HarnessSummary, InstallHint, SessionMessagesPage, SessionRef, SessionSummary, SessionTree, VersionSupport,
};
use crate::pi_family::rpc::RpcDialect;
use crate::pi_family::sessions::{self, SessionCache};
use crate::process::{BoxFuture, ExecSpec, ProcessRunner};

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

    /// The folder holding the saved sessions of `project` (a canonical project directory): the
    /// harness's encoded per-project folder or a configured session directory.
    fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf;

    /// Saved sessions of `project`, newest first (default: PI-family session files, see
    /// [`crate::pi_family::sessions`]). A missing folder lists nothing.
    fn list_sessions(&self, env: &Env, project: &Path, cache: &SessionCache) -> Result<Vec<SessionSummary>> {
        Ok(sessions::list_sessions(
            self.id(),
            &self.sessions_dir(env, project),
            cache,
        ))
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

    /// The approval mode the user configured for `project` (harnesses with
    /// `capabilities.approvalModes`); `None` when the harness has no approval modes.
    fn configured_approval_mode(&self, _env: &Env, _project: Option<&Path>) -> Option<ApprovalMode> {
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

    /// The output line that reports a failed extension action although the CLI exited with 0
    /// (`stdout` and `stderr` are already cleaned and redacted). Default: the exit code decides.
    fn cli_failure_line(&self, _stdout: &str, _stderr: &str) -> Option<String> {
        None
    }

    /// Copyable install instructions shown when the CLI is missing.
    fn install_hint(&self) -> InstallHint;

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
