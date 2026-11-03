//! What the app looks up itself for the steps of a prompt: the Harness Memory notes
//! ([`HookSeed::MemoryFirst`]) and the Harness Knowledge pointers ([`HookSeed::KnowledgeFirst`]).
//!
//! The engine plans a step's hook like any `addContext` hook. [`run_with_recall`] carries out the
//! firings, and for each of these two hooks among them runs the app's own `harness-memory recall
//! --hook` or `harness-knowledge recall --hook` with `--project <the chat's project>`
//! ([`memory_recall`], [`knowledge_recall`]): the message on its stdin, no shell, at most 3 s. What
//! they find comes back apart from the firings' parts, as a [`Lookup`] for
//! [`super::steps::arrange`], which puts it in the step. The message reaches nothing else: command
//! hooks never see it. The recalls run next to the commands of the firings and each other, so they
//! add no time to a slow command. The same hook whether its file still fires at `sessionStart` (a
//! file not updated yet) or at `promptSubmit`: a recall runs when its step does. The retired
//! `knowledge-recall` hook triggers nothing.
//!
//! [`seed_planned`] tells the same for the lookups the shell runs itself (the branch and the
//! limits).

use std::collections::HashSet;

use super::command::{FiringResult, run_firings};
use super::engine::{HookSet, SendPlan};
use super::plan::Firing;
use super::steps::Lookup;
use crate::ecosystem::{KnowledgeFeatures, MemoryFeatures, knowledge_recall, memory_recall};
use crate::env::Env;
use crate::model::{HookEvent, HookSeed};
use crate::process::ProcessRunner;

/// The feature probes the two recalls are gated on.
#[derive(Clone, Copy)]
pub struct RecallFeatures<'a> {
    /// Harness Knowledge: `recall`.
    pub knowledge: &'a KnowledgeFeatures,
    /// Harness Memory: `recall`.
    pub memory: &'a MemoryFeatures,
}

/// The ids of the hooks in `hooks` that trigger the Harness Knowledge recall, on any event.
pub fn recall_hook_ids(hooks: &HookSet) -> HashSet<String> {
    seed_hook_ids(hooks, HookSeed::KnowledgeFirst)
}

/// The ids of the hooks in `hooks` whose file has `seed`, on any event.
fn seed_hook_ids(hooks: &HookSet, seed: HookSeed) -> HashSet<String> {
    hooks
        .hooks
        .iter()
        .filter(|entry| entry.hook.seed == Some(seed))
        .map(|entry| entry.hook.id.clone())
        .collect()
}

/// True when `hooks` holds an active hook whose file has `seed`.
pub fn has_seed_hook(hooks: &HookSet, seed: HookSeed) -> bool {
    hooks.hooks.iter().any(|entry| entry.hook.seed == Some(seed))
}

/// True when a firing of `plan` (only one of `event` when given) holds the part of a hook of
/// `hooks` that has `seed`: the step is in this message, so its lookup is wanted.
pub fn seed_planned(hooks: &HookSet, plan: &SendPlan, seed: HookSeed, event: Option<HookEvent>) -> bool {
    let ids = seed_hook_ids(hooks, seed);

    plan.firings
        .iter()
        .filter(|firing| event.is_none_or(|wanted| firing.event == wanted))
        .any(|firing| holds(firing, &ids))
}

/// What the firings and the recalls produced.
#[derive(Debug)]
pub struct Recalled {
    /// The firings' parts and failures. A recall hook's part keeps its own text.
    pub firing: FiringResult,
    /// The notes for the Harness Memory step: not run when its hook is not planned.
    pub memory: Lookup,
    /// The pointers for the Harness Knowledge step: not run when its hook is not planned.
    pub knowledge: Lookup,
}

/// Carries out the firings of `plan` ([`run_firings`]) and the recalls of the hooks of `hooks` among
/// them, for the user message `prompt` in `project`.
pub async fn run_with_recall(
    runner: &dyn ProcessRunner,
    env: &Env,
    features: RecallFeatures<'_>,
    hooks: &HookSet,
    plan: &SendPlan,
    prompt: &str,
    project: Option<&str>,
) -> Recalled {
    let wants_memory = seed_planned(hooks, plan, HookSeed::MemoryFirst, None);
    let wants_knowledge = seed_planned(hooks, plan, HookSeed::KnowledgeFirst, None);

    let (firing, memory, knowledge) = tokio::join!(
        run_firings(runner, env, &plan.firings),
        async {
            if wants_memory {
                memory_recall(env, runner, features.memory, prompt, project).await
            } else {
                Lookup::NotRun
            }
        },
        async {
            if wants_knowledge {
                knowledge_recall(env, runner, features.knowledge, prompt, project).await
            } else {
                Lookup::NotRun
            }
        },
    );

    Recalled {
        firing,
        memory,
        knowledge,
    }
}

/// True when `firing` holds the part of one of the hooks `ids`.
fn holds(firing: &Firing, ids: &HashSet<String>) -> bool {
    firing.parts.iter().any(|part| ids.contains(part.hook_id()))
}
