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
//! The UI renders from [`HarnessInfo`], [`Capabilities`] and the views, so no UI change is needed.

mod catalog;
mod detect;
mod registry;

pub use catalog::{CommandCatalog, SharedCommand, gui, merge_commands, rpc, spec, unsupported};
pub use detect::{VERSION_TIMEOUT, first_semver, override_name_matches};
pub use registry::HarnessRegistry;

/// Everything an adapter's `impl Harness` block needs: `use crate::harness::prelude::*;`.
pub mod prelude {
    pub use std::path::Path;

    pub use super::{CommandCatalog, Harness, HarnessPaths};
    pub use crate::env::Env;
    pub use crate::error::Result;
    pub use crate::model::{Capabilities, ConfigView, ExtensionsView, HarnessId, HarnessInfo, InstallHint};
}

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::model::{
    Capabilities, ConfigView, Detection, ExtensionsView, HarnessId, HarnessInfo, HarnessSummary, InstallHint,
    VersionSupport,
};
use crate::process::{BoxFuture, ProcessRunner};

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
