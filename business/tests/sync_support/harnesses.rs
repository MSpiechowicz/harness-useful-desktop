//! The harnesses a "Share setup" test machine has, and their summaries.

use harness_business::harness::Harness;
use harness_business::harnesses::{Claude, Omp, OpenCode, Pi};
use harness_business::model::{Detection, HarnessId, HarnessSummary, VersionSupport};

pub const BOTH: &[HarnessId] = &[HarnessId::Omp, HarnessId::Pi];

/// The summaries of OMP, PI, OpenCode and Claude Code, with `installed` detected as installed.
pub fn summaries(installed: &[HarnessId]) -> Vec<HarnessSummary> {
    let harnesses: [&dyn Harness; 4] = [&Omp, &Pi, &OpenCode::default(), &Claude];

    harnesses
        .iter()
        .map(|harness| HarnessSummary {
            info: harness.info(),
            capabilities: harness.capabilities(),
            detection: Detection {
                installed: installed.contains(&harness.id()),
                binary_path: None,
                is_batch_script: false,
                version: None,
                version_support: VersionSupport::Untested,
                config_dir: None,
                config_dir_exists: false,
                install_hint: None,
                error: None,
            },
        })
        .collect()
}
