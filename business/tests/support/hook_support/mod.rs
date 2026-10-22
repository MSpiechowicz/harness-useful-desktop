//! Hooks, hook sets, chats and agent events for the engine tests (`hooks_engine.rs`,
//! `hooks_follow_up.rs`).
#![allow(dead_code)]

use std::path::PathBuf;

use harness_business::hooks::{
    ChatContext, EventOutcome, Firing, FollowUpPlan, HookSession, HookSet, PlannedPart, SendOrigin, SendPlan,
};
use harness_business::model::{
    AgentAction, AgentEvent, Hook, HookAction, HookApproval, HookAuthor, HookEvent, HookOrigin, HookScope, HookStatus,
    HookView,
};
use serde_json::Value;

pub const PROJECT: &str = "/work/app";

pub fn hook(number: u8, event: HookEvent, action: HookAction) -> Hook {
    Hook {
        id: format!("{number:032x}"),
        name: format!("hook {number}"),
        description: None,
        scope: HookScope::Global,
        project: None,
        event,
        matcher: None,
        action,
        enabled: true,
        requires: Vec::new(),
        seed: None,
        created_at: format!("2026-10-17T10:00:{number:02}Z"),
        created_by: HookAuthor::Desktop,
        updated_at: "2026-10-17T10:00:00Z".to_owned(),
        updated_by: HookAuthor::Desktop,
    }
}

pub fn context(number: u8, event: HookEvent) -> Hook {
    hook(
        number,
        event,
        HookAction::AddContext {
            text: format!("context {number}"),
        },
    )
}

pub fn follow_up(number: u8, event: HookEvent) -> Hook {
    hook(
        number,
        event,
        HookAction::FollowUp {
            text: format!("follow-up {number}"),
        },
    )
}

pub fn command(number: u8, event: HookEvent) -> Hook {
    hook(
        number,
        event,
        HookAction::Command {
            command: format!("echo {number}"),
            timeout_secs: None,
        },
    )
}

/// `hook` for the tools `matcher` names only.
pub fn matching(mut hook: Hook, matcher: &str) -> Hook {
    hook.matcher = Some(matcher.to_owned());
    hook
}

pub fn with_status(hook: Hook, status: HookStatus) -> HookView {
    HookView {
        id: hook.id.clone(),
        name: Some(hook.name.clone()),
        status,
        sha256: "0".repeat(64),
        problem: None,
        approval: HookApproval::Approved,
        approved: None,
        hook: Some(hook),
        origin: HookOrigin::Local,
        team_author: None,
        team_updated_at: None,
        team_repo: None,
        team_changed: false,
        share_block: None,
        team_disabled: false,
        root: None,
        repo_key: None,
    }
}

pub fn set(hooks: Vec<Hook>) -> HookSet {
    let views: Vec<HookView> = hooks
        .into_iter()
        .map(|hook| with_status(hook, HookStatus::Active))
        .collect();
    HookSet::for_chat(&views, Some(PROJECT))
}

pub fn chat(fresh: bool, utility: bool) -> HookSession {
    HookSession::new(ChatContext {
        session_id: "session-1".to_owned(),
        folder: PathBuf::from("/work/app/sub"),
        project: Some(PROJECT.to_owned()),
        fresh,
        utility,
    })
}

pub fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

pub fn tool_end(name: &str, is_error: bool, touched: &[&str]) -> AgentEvent {
    AgentEvent::ToolEnd {
        tool_call_id: format!("call-{name}"),
        name: name.to_owned(),
        is_error,
        result: Value::Null,
        touched_paths: touched.iter().map(|path| (*path).to_owned()).collect(),
    }
}

pub fn settled() -> AgentEvent {
    AgentEvent::Settled { aborted: Some(false) }
}

/// `(event, hook names)` of each firing, texts and commands alike.
pub fn fired(plan: &SendPlan) -> Vec<(HookEvent, Vec<String>)> {
    plan.firings
        .iter()
        .map(|firing| (firing.event, names(firing)))
        .collect()
}

pub fn names(firing: &Firing) -> Vec<String> {
    firing
        .parts
        .iter()
        .map(|part| match part {
            PlannedPart::Text(text) => text.hook_name.clone(),
            PlannedPart::Command(request) => request.hook_name.clone(),
        })
        .collect()
}

pub fn texts(firing: &Firing) -> Vec<String> {
    firing
        .parts
        .iter()
        .filter_map(|part| match part {
            PlannedPart::Text(text) => Some(text.text.clone()),
            PlannedPart::Command(_) => None,
        })
        .collect()
}

pub fn follow_up_plan(outcome: EventOutcome) -> FollowUpPlan {
    match outcome {
        EventOutcome::FollowUp(plan) => plan,
        other => panic!("expected a follow-up, got {other:?}"),
    }
}

/// A user message as the shell sends it: planned, then counted right before it goes out.
pub fn user_send(session: &mut HookSession, hooks: &HookSet, action: &AgentAction) -> SendPlan {
    let mut plan = session.on_send(hooks, action, SendOrigin::User);
    session.sending(&mut plan);
    plan
}

/// A user prompt, a successful tool call, and the settle that ends the run.
pub fn user_run(session: &mut HookSession, hooks: &HookSet) -> EventOutcome {
    user_send(session, hooks, &prompt("Fix it"));
    session.on_event(hooks, &tool_end("read", false, &[]));
    session.on_event(hooks, &settled())
}
