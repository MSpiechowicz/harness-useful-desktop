//! The list of supported harnesses.

use std::path::Path;

use super::Harness;
use crate::env::Env;
use crate::harnesses::{Omp, OpenCode, Pi};
use crate::model::{BinaryOverride, HarnessId, HarnessSummary};
use crate::process::{ProcessRunner, find_override, join_all};

/// Every registered harness, in display order.
pub struct HarnessRegistry {
    harnesses: Vec<Box<dyn Harness>>,
}

impl HarnessRegistry {
    /// The harnesses this app ships with. Adding a harness adds one line here.
    pub fn with_defaults() -> Self {
        Self::new(vec![Box::new(Omp), Box::new(Pi), Box::new(OpenCode::default())])
    }

    /// A registry over `harnesses` (tests, or a future plugin mechanism).
    pub fn new(harnesses: Vec<Box<dyn Harness>>) -> Self {
        Self { harnesses }
    }

    /// Every harness, in display order.
    pub fn all(&self) -> impl Iterator<Item = &dyn Harness> {
        self.harnesses.iter().map(Box::as_ref)
    }

    /// The harness with `id`, if registered.
    pub fn get(&self, id: HarnessId) -> Option<&dyn Harness> {
        self.all().find(|harness| harness.id() == id)
    }

    /// Summaries of every harness, detected concurrently (each probe has its own timeout).
    pub async fn summaries(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        overrides: &[BinaryOverride],
    ) -> Vec<HarnessSummary> {
        let probes = self
            .all()
            .map(|harness| {
                let override_path: Option<&Path> = find_override(overrides, harness.id());
                harness.summary(env, runner, override_path)
            })
            .collect();

        join_all(probes).await
    }
}
