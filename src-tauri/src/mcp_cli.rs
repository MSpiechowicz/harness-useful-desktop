//! The harness CLI that changes an MCP file the app only reads (Claude Code's user scope, see
//! `harness_business::ecosystem::mcp_cli`): the registrations of the dashboard's, the memory
//! server's and the app's own server run it for such a file, from their blocking task. Any other
//! file, or one whose harness has no detected binary, takes the file path, which never writes a
//! CLI-owned file.

use std::sync::Arc;

use harness_business::ecosystem::McpFile;
use harness_business::env::Env;
use harness_business::harness::{HarnessCli, HarnessRegistry};
use harness_business::model::{HarnessId, HarnessSummary};
use harness_business::process::{ResolvedBinary, TokioRunner};

use crate::state::AppState;

/// A harness's user MCP file and its detected binary (which runs when its CLI changes the file).
pub(crate) type HarnessFile = (HarnessId, McpFile, Option<ResolvedBinary>);

/// The binary `summary` detected, when the harness is installed.
pub(crate) fn detected_binary(summary: &HarnessSummary) -> Option<ResolvedBinary> {
    let detection = &summary.detection;
    let path = detection.binary_path.as_ref().filter(|_| detection.installed)?;

    Some(ResolvedBinary {
        path: path.into(),
        is_batch_script: detection.is_batch_script,
    })
}

/// The cached detection's binary of `harness`.
pub(crate) async fn cached_binary(state: &AppState, harness: HarnessId) -> Option<ResolvedBinary> {
    state.detections.read().await.get(&harness).and_then(detected_binary)
}

/// What runs the CLI of a harness: its registry, the environment and the process runner.
#[derive(Clone)]
pub(crate) struct CliContext {
    registry: Arc<HarnessRegistry>,
    env: Arc<Env>,
    runner: TokioRunner,
}

impl CliContext {
    pub(crate) fn of(state: &AppState) -> Self {
        Self {
            registry: state.registry.clone(),
            env: state.env.clone(),
            runner: state.runner,
        }
    }

    /// The CLI that changes `file` of `harness`, when the harness's CLI changes it and `binary` is
    /// known.
    pub(crate) fn cli_for<'a>(
        &'a self,
        harness: HarnessId,
        file: &McpFile,
        binary: Option<&'a ResolvedBinary>,
    ) -> Option<HarnessCli<'a>> {
        if !file.via_cli() {
            return None;
        }

        Some(HarnessCli {
            harness: self.registry.get(harness)?,
            env: &self.env,
            runner: &self.runner,
            binary: binary?,
        })
    }
}
