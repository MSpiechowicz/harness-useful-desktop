//! OMP subagent progress frames decoded to a subagent snapshot and its steps.

use harness_business::harness::Harness;
use harness_business::harnesses::Omp;
use harness_business::model::{AgentEvent, SubagentRoute, SubagentStepStatus};
use harness_business::pi_family::rpc::{DecodeState, Inbound};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn omp_subagent_progress_decodes_to_a_snapshot_and_steps_with_the_task_sent_once() {
    let omp = Omp.dialect();
    let mut state = DecodeState::new("/nonexistent");
    let progress = |count: u32| {
        json!({
            "type": "subagent_progress",
            "payload": {
                "agent": "explore", "task": "Map the parser",
                "progress": {
                    "id": "Scout", "agent": "explore", "status": "running", "toolCount": count,
                    "recentTools": [{ "tool": "read", "args": "a.rs", "endMs": 7 }],
                    "currentTool": "grep", "currentToolArgs": "fn parse"
                }
            }
        })
    };

    let events = |inbound: Vec<Inbound>| -> Vec<AgentEvent> {
        inbound
            .into_iter()
            .map(|inbound| match inbound {
                Inbound::Event(event) => event,
                other => panic!("expected events, got {other:?}"),
            })
            .collect()
    };

    let first = events(omp.decode(progress(1), &mut state));
    let [
        AgentEvent::Subagent { subagent },
        AgentEvent::SubagentStep {
            subagent_id,
            step: done,
        },
        AgentEvent::SubagentStep { step: running, .. },
    ] = first.as_slice()
    else {
        panic!("a snapshot, a finished step and a running step: {first:?}");
    };
    assert_eq!(subagent.task.as_deref(), Some("Map the parser"));
    assert_eq!(subagent.message_route, Some(SubagentRoute::Orchestrator));
    assert_eq!(subagent.can_stop, Some(false));
    assert_eq!(subagent_id, "Scout");
    assert_eq!(
        (done.id.as_str(), done.status, done.detail.as_str()),
        ("Scout#1", SubagentStepStatus::Done, "a.rs")
    );
    assert_eq!(
        (running.id.as_str(), running.status),
        ("Scout#2", SubagentStepStatus::Running)
    );

    let again = events(omp.decode(progress(1), &mut state));
    let [AgentEvent::Subagent { subagent }] = again.as_slice() else {
        panic!("a repeated frame is only the snapshot: {again:?}");
    };
    assert_eq!(subagent.task, None);
}
