//! What the installed Harness Code can do: `harness-code features` prints one feature per line and
//! exits 0. The desktop reads these:
//!
//! - `code-index`: `status|enable|disable --json`, the switch of the code graph behind the
//!   `code_*` tools;
//! - `code-prepare`: `build --project P --json`, the build of a repository's index before an agent
//!   asks for it.
//!
//! The server lists `code-hint` and `code-match` too, which only the skills and Harness Knowledge
//! use. An older server does not know the command and exits non-zero. [`CodeFeatures::support`]
//! asks once per binary through a [`LineProbe`] (cached per binary, an older server cached as one
//! without features, a failed probe not cached).

use std::path::PathBuf;

use super::catalog::CODE_SIDECAR;
use super::sidecar_embedder::sidecar_feature_server;
use super::sidecar_line_probe::{LineAnswer, LineProbe};
use crate::env::Env;
use crate::error::Result;
use crate::process::ProcessRunner;

/// The line for the code graph switch (`status|enable|disable`).
const CODE_INDEX: &str = "code-index";

/// The line for building the index of a repository ahead of the first agent call.
const CODE_PREPARE: &str = "code-prepare";

/// What the installed server says it can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeSupport {
    /// It answers `features`: which of the lines the desktop reads it listed.
    Listed {
        /// `code-index` is listed.
        index: bool,
        /// `code-prepare` is listed.
        prepare: bool,
    },
    /// It predates `features` (or failed it): none of them.
    Unsupported,
    /// Harness Code is not installed.
    NotInstalled,
    /// The server could not be asked (it did not start or answer in time).
    Unknown,
}

impl CodeSupport {
    /// Whether the server has `status|enable|disable`: `code-index` is listed.
    pub fn index(self) -> bool {
        matches!(self, Self::Listed { index: true, .. })
    }

    /// Whether the server has `build`: `code-prepare` is listed.
    pub fn prepare(self) -> bool {
        matches!(self, Self::Listed { prepare: true, .. })
    }
}

/// The probe of the installed server's features. See the module docs.
#[derive(Debug)]
pub struct CodeFeatures {
    probe: LineProbe,
}

impl Default for CodeFeatures {
    fn default() -> Self {
        Self {
            probe: LineProbe::new(&CODE_SIDECAR, "features"),
        }
    }
}

impl CodeFeatures {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// What the installed server can do (see the module docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> CodeSupport {
        match self.probe.ask(env, runner).await {
            answer @ LineAnswer::Listed(_) => CodeSupport::Listed {
                index: answer.lists(CODE_INDEX),
                prepare: answer.lists(CODE_PREPARE),
            },
            LineAnswer::Unsupported => CodeSupport::Unsupported,
            LineAnswer::NotInstalled => CodeSupport::NotInstalled,
            LineAnswer::Unknown => CodeSupport::Unknown,
        }
    }

    /// The installed server when `feature` of its `features` is listed: `notFound` when Harness
    /// Code is not installed, `unsupported` (naming `what`) when it predates the feature.
    pub async fn feature_server(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        feature: fn(CodeSupport) -> bool,
        what: &str,
    ) -> Result<PathBuf> {
        let listed = async { feature(self.support(env, runner).await) };

        sidecar_feature_server(&CODE_SIDECAR, env, listed, what).await
    }
}
