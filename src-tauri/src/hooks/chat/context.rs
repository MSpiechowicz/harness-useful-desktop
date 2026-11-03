//! The context of a user prompt: the lookups of its steps and the composed message.

use harness_business::ecosystem::close_limits;
use harness_business::hooks::{
    HookSession, HookSet, Lookups, RecallFeatures, SendPlan, arrange, compose_prompt, run_with_recall, seed_planned,
};
use harness_business::model::{AgentAction, HookEvent, HookSeed};
use harness_business::process::TokioRunner;
use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::ChatHooks;
use super::lookups::{branch_note, limit_gauges};
use super::report::report;
use crate::state::AppState;

/// `action` (an eligible prompt) with the context of `plan` in front of its text: the numbered
/// steps, the memory settings notice, then the other hooks (`harness_business::hooks::arrange`). A
/// `sessionStart` context or a notice that did not fit comes again on the next prompt.
///
/// The app looks up what the steps of `plan` carry, each only while its hook is in the plan and
/// all next to the commands of the firings: the Harness Memory notes and the Harness Knowledge
/// pointers of the chat's project (`harness_business::hooks::recall`), the Git branch (at the
/// first message, where its `sessionStart` hook fires) and the plan limits that are close (see
/// [`super::lookups`]). A lookup that fails leaves its step to say so, and never holds the message
/// back.
pub(super) async fn with_context(
    state: &AppState,
    chat: &ChatHooks,
    action: AgentAction,
    plan: &SendPlan,
    hooks: &HookSet,
    session: &mut HookSession,
) -> AgentAction {
    let AgentAction::Prompt {
        message,
        images,
        streaming_behavior,
    } = action
    else {
        return action;
    };

    let key = chat.key();
    let project = chat.project();
    let folder = session.chat().folder.clone();

    let wants_branch = seed_planned(hooks, plan, HookSeed::BranchSync, Some(HookEvent::SessionStart));
    let wants_limits = seed_planned(hooks, plan, HookSeed::DashboardLimits, None);

    let features = RecallFeatures {
        knowledge: state.hooks.knowledge_features(),
        memory: &state.memory_features,
    };

    let timed = std::time::Instant::now();
    let (recalled, (branch, prefetched), gauges) = tokio::join!(
        run_with_recall(
            &TokioRunner,
            &state.env,
            features,
            hooks,
            plan,
            &message,
            project.as_deref()
        ),
        async {
            if wants_branch {
                // Started with the chat when the first message was foreseen, else looked up now.
                chat.branch.note(&folder, || branch_note(state, &folder)).await
            } else {
                (None, false)
            }
        },
        async {
            if wants_limits {
                limit_gauges(state, &chat.limit_providers(), project.as_deref()).await
            } else {
                Vec::new()
            }
        },
    );

    tracing::debug!(session = %key, lookups_ms = timed.elapsed().as_millis(), branch = wants_branch, branch_prefetched = prefetched, limits = wants_limits, "hook lookups done");

    report(&state.agents, key, &recalled.firing.failures);

    // What the step shows is told: a steer later only comes for a level beyond it.
    chat.limits.told(&gauges);

    let lookups = Lookups {
        memory: recalled.memory,
        knowledge: recalled.knowledge,
        branch,
        limits: close_limits(&gauges, Timestamp::now(), &TimeZone::system()),
    };

    let parts = arrange(plan.memory_notice.clone(), recalled.firing.parts, &lookups);
    let (message, delivered) = compose_prompt(&message, &parts);
    session.context_composed(plan, &parts, &delivered);

    tracing::debug!(session = %key, parts = delivered.len(), "hook context added");

    AgentAction::Prompt {
        message,
        images,
        streaming_behavior,
    }
}
