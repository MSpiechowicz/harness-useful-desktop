use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Identifies a supported coding-agent harness.
///
/// Adding a harness means adding a variant here plus a `harnesses/<name>` module that
/// implements the harness trait and a registry entry; the UI renders data-driven from those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum HarnessId {
    /// oh-my-pi (`omp`).
    Omp,
    /// PI coding agent (`pi`).
    Pi,
}

impl HarnessId {
    /// Every supported harness, in display order.
    pub const ALL: [HarnessId; 2] = [HarnessId::Omp, HarnessId::Pi];

    /// The default executable name of the harness CLI.
    pub fn binary_name(self) -> &'static str {
        match self {
            HarnessId::Omp => "omp",
            HarnessId::Pi => "pi",
        }
    }
}
