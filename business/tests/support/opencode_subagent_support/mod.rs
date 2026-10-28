//! Helpers of the OpenCode subagent mapper tests: the snapshots and steps of one subagent in a list
//! of events, and text that exercises the bounds and the redaction.
#![allow(dead_code)]

pub mod crafted;

use harness_business::model::{AgentEvent, Subagent, SubagentStep};

/// The snapshots of subagent `id`, in order.
pub fn snapshots(events: &[AgentEvent], id: &str) -> Vec<Subagent> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Subagent { subagent } if subagent.id == id => Some((**subagent).clone()),
            _ => None,
        })
        .collect()
}

/// The newest snapshot of subagent `id`.
pub fn latest(events: &[AgentEvent], id: &str) -> Subagent {
    snapshots(events, id)
        .pop()
        .unwrap_or_else(|| panic!("no snapshot of {id}: {events:#?}"))
}

/// The steps reported for subagent `id`, in order (a step appears again as it changes).
pub fn steps(events: &[AgentEvent], id: &str) -> Vec<SubagentStep> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::SubagentStep { subagent_id, step } if subagent_id == id => Some((**step).clone()),
            _ => None,
        })
        .collect()
}

/// A token of the shape the redactor knows, built at run time.
pub fn fake_token() -> String {
    format!("{}_{}", "ghp", "A1b2C3d4E5".repeat(4))
}

/// A server password of the shape the launcher makes (64 lowercase hex), built at run time.
pub fn fake_password() -> String {
    (3..7_u64)
        .map(|n| format!("{:016x}", 0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(n) ^ 0x5bd1_e995))
        .collect()
}

/// `password` laid in filler so that a text cut to `limit` characters (the last one the ellipsis
/// of the cut) ends in the middle of it.
pub fn straddling(limit: usize, password: &str) -> String {
    format!(
        "{}{password}{}",
        "x".repeat(limit - 1 - password.len() / 2),
        "x".repeat(64)
    )
}

/// `password` laid so that the end of a text cut to its last `limit` bytes starts in the middle of
/// it.
pub fn straddling_tail(limit: usize, password: &str) -> String {
    format!("{password}{}", "q".repeat(limit - password.len() / 2))
}

/// A piece of at least four characters of `password` in `text`, if any shows.
pub fn leaked<'a>(text: &str, password: &'a str) -> Option<&'a str> {
    (0..=password.len() - 4)
        .map(|start| &password[start..start + 4])
        .find(|piece| text.contains(piece))
}

/// The subagent events as JSON, to look for text that must not be in any of them.
pub fn shown(events: &[AgentEvent]) -> String {
    let panel: Vec<&AgentEvent> = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Subagent { .. } | AgentEvent::SubagentStep { .. }))
        .collect();

    serde_json::to_string(&panel).expect("events serialize")
}

/// `count` lines `<prefix> 1`, `<prefix> 2`, …
pub fn numbered(prefix: &str, count: usize) -> String {
    (1..=count)
        .map(|number| format!("{prefix} {number}"))
        .collect::<Vec<_>>()
        .join("\n")
}
