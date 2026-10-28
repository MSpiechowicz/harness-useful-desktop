//! Tests of the OMP subagent step tracker.

use serde_json::json;

use super::*;
use crate::harnesses::omp::subagents::decode_subagent;

fn tool(name: &str, args: &str, end: u64) -> Value {
    json!({ "tool": name, "args": args, "endMs": end })
}

fn frame(count: u32, tools: Vec<Value>, current: Option<(&str, &str)>, output: &[&str]) -> Value {
    let mut progress = json!({
        "id": "Scout", "agent": "explore", "status": "running", "toolCount": count,
        "recentTools": tools, "recentOutput": output
    });

    if let Some((name, args)) = current {
        progress["currentTool"] = json!(name);
        progress["currentToolArgs"] = json!(args);
    }

    json!({ "type": "subagent_progress", "payload": { "agent": "explore", "task": "Map it", "progress": progress } })
}

fn observe(steps: &mut SubagentSteps, frame: &Value) -> Vec<AgentEvent> {
    steps.observe(frame, decode_subagent(frame).unwrap())
}

fn steps_of(events: &[AgentEvent]) -> Vec<&SubagentStep> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::SubagentStep { step, .. } => Some(&**step),
            _ => None,
        })
        .collect()
}

fn snapshot(events: &[AgentEvent]) -> &Subagent {
    match &events[0] {
        AgentEvent::Subagent { subagent } => subagent,
        other => panic!("the snapshot comes first, got {other:?}"),
    }
}

#[test]
fn a_rising_tool_count_yields_one_step_per_new_call_and_the_running_one_is_replaced() {
    let mut steps = SubagentSteps::default();

    let first = observe(&mut steps, &frame(0, vec![], Some(("read", "a.rs")), &[]));
    let running = steps_of(&first);
    assert_eq!(
        (running[0].id.as_str(), running[0].status),
        ("Scout#1", SubagentStepStatus::Running)
    );
    assert_eq!(running[0].detail, "a.rs");

    let tools = vec![
        tool("read", "a.rs", 10),
        json!({ "tool": "grep", "args": "fn", "isError": true, "endMs": 20 }),
    ];
    let second = observe(&mut steps, &frame(2, tools.clone(), Some(("edit", "b.rs")), &[]));
    let done = steps_of(&second);
    let shown: Vec<_> = done
        .iter()
        .map(|step| (step.id.as_str(), step.status, step.at))
        .collect();
    assert_eq!(
        shown,
        [
            ("Scout#1", SubagentStepStatus::Done, Some(10)),
            ("Scout#2", SubagentStepStatus::Failed, Some(20)),
            ("Scout#3", SubagentStepStatus::Running, None),
        ]
    );

    let repeated = observe(&mut steps, &frame(2, tools.clone(), Some(("edit", "b.rs")), &[]));
    assert!(steps_of(&repeated).is_empty(), "a repeated frame adds nothing");

    let mut more = tools;
    more.push(tool("edit", "b.rs", 30));
    let third = observe(&mut steps, &frame(3, more, None, &[]));
    let shown: Vec<_> = steps_of(&third)
        .iter()
        .map(|step| (step.id.clone(), step.status))
        .collect();
    assert_eq!(shown, [("Scout#3".to_owned(), SubagentStepStatus::Done)]);
}

#[test]
fn the_task_is_sent_once_and_the_snapshot_cannot_stop_or_reach_the_subagent_directly() {
    let mut steps = SubagentSteps::default();

    let first = observe(&mut steps, &frame(0, vec![], None, &[]));
    let second = observe(&mut steps, &frame(0, vec![], None, &[]));

    assert_eq!(snapshot(&first).task.as_deref(), Some("Map it"));
    assert_eq!(snapshot(&second).task, None);
    assert_eq!(snapshot(&second).can_stop, Some(false));
    assert_eq!(
        snapshot(&second).message_route,
        Some(crate::model::SubagentRoute::Orchestrator)
    );
    assert_eq!(snapshot(&second).report, None);
}

#[test]
fn new_output_lands_on_the_latest_finished_step_once() {
    let mut steps = SubagentSteps::default();
    let tools = vec![tool("bash", "make", 5)];

    let first = observe(&mut steps, &frame(1, tools.clone(), None, &["ok", "done"]));
    let step = steps_of(&first)[0].clone();
    assert_eq!((step.output.as_deref(), step.output_lines), (Some("ok\ndone"), Some(2)));

    let same = observe(&mut steps, &frame(1, tools.clone(), None, &["ok", "done"]));
    assert!(steps_of(&same).is_empty());

    let later = observe(&mut steps, &frame(1, tools, None, &["ok", "done", "more"]));
    let again = steps_of(&later);
    assert_eq!(
        (again[0].id.as_str(), again[0].output.as_deref()),
        ("Scout#1", Some("ok\ndone\nmore"))
    );
}

#[test]
fn a_count_jump_beyond_the_recent_list_keeps_the_ordinals_of_what_is_left() {
    let mut steps = SubagentSteps::default();
    let tools: Vec<Value> = (0..RECENT_TOOLS + 2)
        .map(|n| tool("read", &format!("f{n}"), 0))
        .collect();

    let events = observe(&mut steps, &frame(40, tools, None, &[]));
    let ids: Vec<&str> = steps_of(&events).iter().map(|step| step.id.as_str()).collect();

    assert_eq!(ids, ["Scout#36", "Scout#37", "Scout#38", "Scout#39", "Scout#40"]);
}

#[test]
fn secrets_in_tool_arguments_and_output_are_redacted() {
    let token = format!("ghp_{}", "a1B2".repeat(9));
    let mut steps = SubagentSteps::default();
    let tools = vec![tool("bash", &format!("curl -H 'Authorization: token {token}'"), 1)];

    let events = observe(&mut steps, &frame(1, tools, None, &[&format!("token {token}")]));
    let step = steps_of(&events)[0].clone();

    assert!(!step.detail.contains(&token), "{}", step.detail);
    assert!(!step.output.unwrap().contains(&token));
}

fn with_status(mut frame: Value, status: &str) -> Value {
    frame["payload"]["progress"]["status"] = json!(status);
    frame
}

fn lifecycle(status: &str) -> Value {
    json!({ "type": "subagent_lifecycle", "payload": { "id": "Scout", "agent": "explore", "status": status } })
}

fn shown(events: &[AgentEvent]) -> Vec<(String, SubagentStepStatus)> {
    steps_of(events)
        .iter()
        .map(|step| (step.id.clone(), step.status))
        .collect()
}

#[test]
fn a_second_identical_call_gets_its_own_running_step() {
    let mut steps = SubagentSteps::default();
    let read = tool("read", "a.rs", 10);

    observe(&mut steps, &frame(0, vec![], Some(("read", "a.rs")), &[]));
    let second = observe(&mut steps, &frame(1, vec![read.clone()], Some(("read", "a.rs")), &[]));

    assert_eq!(
        shown(&second),
        [
            ("Scout#1".to_owned(), SubagentStepStatus::Done),
            ("Scout#2".to_owned(), SubagentStepStatus::Running),
        ]
    );

    let repeated = observe(&mut steps, &frame(1, vec![read], Some(("read", "a.rs")), &[]));
    assert!(steps_of(&repeated).is_empty(), "the same frame again adds nothing");
}

#[test]
fn an_ended_subagent_closes_the_call_in_flight() {
    for (status, closed) in [
        ("failed", SubagentStepStatus::Failed),
        ("aborted", SubagentStepStatus::Failed),
        ("completed", SubagentStepStatus::Done),
    ] {
        let mut steps = SubagentSteps::default();
        observe(&mut steps, &frame(0, vec![], Some(("bash", "make")), &[]));

        let ended = observe(&mut steps, &lifecycle(status));
        let closing = steps_of(&ended);
        assert_eq!(closing.len(), 1, "{status}");
        assert_eq!(
            (closing[0].id.as_str(), closing[0].status, closing[0].detail.as_str()),
            ("Scout#1", closed, "make"),
            "{status}"
        );

        let again = observe(&mut steps, &lifecycle(status));
        assert!(steps_of(&again).is_empty(), "{status}: closed once");
    }
}

#[test]
fn a_final_progress_frame_closes_the_call_without_starting_another() {
    let mut steps = SubagentSteps::default();
    observe(&mut steps, &frame(0, vec![], Some(("bash", "make")), &[]));

    let last = with_status(frame(0, vec![], Some(("edit", "b.rs")), &[]), "failed");
    let ended = observe(&mut steps, &last);

    assert_eq!(shown(&ended), [("Scout#1".to_owned(), SubagentStepStatus::Failed)]);
}

#[test]
fn a_call_the_tool_count_finished_is_not_closed_again() {
    let mut steps = SubagentSteps::default();
    observe(&mut steps, &frame(0, vec![], Some(("read", "a.rs")), &[]));

    let last = with_status(frame(1, vec![tool("read", "a.rs", 10)], None, &[]), "completed");
    let ended = observe(&mut steps, &last);

    assert_eq!(shown(&ended), [("Scout#1".to_owned(), SubagentStepStatus::Done)]);
    assert_eq!(steps_of(&ended)[0].at, Some(10));
}

#[test]
fn lifecycle_frames_add_no_steps() {
    let mut steps = SubagentSteps::default();
    let frame = json!({ "type": "subagent_lifecycle", "payload": { "id": "Scout", "agent": "explore", "status": "completed" } });

    let events = observe(&mut steps, &frame);

    assert_eq!(events.len(), 1);
    assert_eq!(snapshot(&events).status, crate::model::SubagentStatus::Completed);
}
