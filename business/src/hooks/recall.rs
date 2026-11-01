//! The Harness Knowledge recall of desktop chats: the context element of the `knowledge-recall`
//! family hook ([`HookSeed::KnowledgeRecall`]), produced by the app when it composes a prompt.
//!
//! The engine plans the hook like any `addContext` hook, with the placeholder text of its file.
//! [`run_with_recall`] carries out the firings, and in place of that text puts the pointers of the
//! app's own `harness-knowledge recall --hook --project <the chat's project>` run
//! ([`knowledge_recall`]): the message on its stdin, no shell, at most 3 s, nothing when it fails.
//! The element is `<hook name="Knowledge">` like every other, so [`super::compose_prompt`] keeps it
//! within the 8 KiB limit. The message reaches nothing else: command hooks never see it.
//! The recall runs next to the commands of the firings, so it adds no time to a slow command.

use std::collections::HashSet;

use super::command::{FiringResult, run_firings};
use super::compose::ResolvedPart;
use super::engine::{HookSet, SendPlan};
use super::plan::Firing;
use crate::ecosystem::{KnowledgeFeatures, knowledge_recall};
use crate::env::Env;
use crate::model::{HookEvent, HookSeed};
use crate::process::ProcessRunner;

/// The name of the recall's `<hook>` element.
pub const KNOWLEDGE_ELEMENT_NAME: &str = "Knowledge";

/// The ids of the recall hooks in `hooks`.
pub fn recall_hook_ids(hooks: &HookSet) -> HashSet<String> {
    hooks
        .of_event(HookEvent::PromptSubmit)
        .filter(|entry| entry.hook.seed.is_some_and(HookSeed::is_app_run_recall))
        .map(|entry| entry.hook.id.clone())
        .collect()
}

/// Carries out the firings of `plan` ([`run_firings`]) and the recall of the hooks of `hooks` among
/// them, for the user message `prompt` in `project`. A recall hook's part is the recalled block, or
/// no part when there is none.
pub async fn run_with_recall(
    runner: &dyn ProcessRunner,
    env: &Env,
    features: &KnowledgeFeatures,
    hooks: &HookSet,
    plan: &SendPlan,
    prompt: &str,
    project: Option<&str>,
) -> FiringResult {
    let recall = recall_hook_ids(hooks);
    let wanted = recall_planned(&plan.firings, &recall);

    let (mut result, recalled) = tokio::join!(run_firings(runner, env, &plan.firings), async {
        if wanted {
            knowledge_recall(env, runner, features, prompt, project).await
        } else {
            None
        }
    });

    result.parts = place_recall(result.parts, &recall, recalled);
    result
}

/// True when a firing holds the part of a recall hook.
fn recall_planned(firings: &[Firing], recall: &HashSet<String>) -> bool {
    firings
        .iter()
        .flat_map(|firing| firing.parts.iter())
        .any(|part| recall.contains(part.hook_id()))
}

/// `parts` with each recall hook's placeholder replaced by `recalled`, or dropped without it.
fn place_recall(parts: Vec<ResolvedPart>, recall: &HashSet<String>, recalled: Option<String>) -> Vec<ResolvedPart> {
    parts
        .into_iter()
        .filter_map(|part| {
            if !recall.contains(&part.hook_id) {
                return Some(part);
            }

            let text = recalled.clone()?;

            Some(ResolvedPart {
                hook_name: KNOWLEDGE_ELEMENT_NAME.to_owned(),
                text,
                ..part
            })
        })
        .collect()
}
