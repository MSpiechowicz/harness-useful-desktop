//! The list of supported harnesses.

use std::future::poll_fn;
use std::path::Path;
use std::task::Poll;

use super::Harness;
use crate::env::Env;
use crate::harnesses::{Omp, OpenCode, Pi};
use crate::model::{BinaryOverride, HarnessId, HarnessSummary};
use crate::process::{BoxFuture, ProcessRunner, find_override};

/// Every registered harness, in display order.
pub struct HarnessRegistry {
    harnesses: Vec<Box<dyn Harness>>,
}

impl HarnessRegistry {
    /// The harnesses this app ships with. Adding a harness adds one line here.
    pub fn with_defaults() -> Self {
        Self::new(vec![Box::new(Omp), Box::new(Pi), Box::new(OpenCode)])
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

/// Polls every future to completion concurrently and returns their outputs in order.
async fn join_all<T>(futures: Vec<BoxFuture<'_, T>>) -> Vec<T> {
    let mut pending: Vec<Option<BoxFuture<'_, T>>> = futures.into_iter().map(Some).collect();
    let mut outputs: Vec<Option<T>> = pending.iter().map(|_| None).collect();

    poll_fn(|context| {
        let mut busy = false;

        for (slot, output) in pending.iter_mut().zip(outputs.iter_mut()) {
            let Some(future) = slot else {
                continue;
            };

            match future.as_mut().poll(context) {
                Poll::Ready(value) => {
                    *output = Some(value);
                    *slot = None;
                }
                Poll::Pending => busy = true,
            }
        }

        if busy { Poll::Pending } else { Poll::Ready(()) }
    })
    .await;

    outputs.into_iter().flatten().collect()
}
