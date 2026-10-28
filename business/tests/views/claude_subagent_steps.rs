//! The steps of Claude Code subagents: tool calls and prose from the frames a subagent writes
//! (`parent_tool_use_id`), however the tool that spawned it is named, also in the background after
//! the parent's turn ended, and the bounds and redaction on everything they carry.

use crate::claude_frames;

use claude_frames::{feed, hints, kinds, mapper, requesting};
use harness_business::claude_stream::mapper::{Mapped, events};
use harness_business::model::{
    AgentEvent, MAX_STEP_OUTPUT_BYTES, MAX_STEP_OUTPUT_LINES, MAX_SUBAGENT_TEXT_CHARS, Subagent, SubagentRoute,
    SubagentStatus, SubagentStep, SubagentStepKind, SubagentStepStatus,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A token of the shape the redactor knows, built at run time.
fn fake_token() -> String {
    format!("{}_{}", "ghp", "A1b2C3d4E5".repeat(4))
}

fn steps(mapped: &[Mapped]) -> Vec<(String, SubagentStep)> {
    events(mapped)
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::SubagentStep { subagent_id, step } => Some((subagent_id, *step)),
            _ => None,
        })
        .collect()
}

fn snapshots(mapped: &[Mapped]) -> Vec<Subagent> {
    events(mapped)
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Subagent { subagent } => Some(*subagent),
            _ => None,
        })
        .collect()
}

/// The main chat's `assistant` frame calling `Agent` (or another tool) once per `(id, name, input)`.
fn spawn(calls: &[(&str, &str, Value)]) -> Value {
    let blocks: Vec<Value> = calls
        .iter()
        .map(|(id, name, input)| json!({ "type": "tool_use", "id": id, "name": name, "input": input }))
        .collect();

    json!({ "type": "assistant", "parent_tool_use_id": null,
        "message": { "id": "msg_main", "model": "claude-opus-5-5", "content": blocks } })
}

fn started(task: &str, call: &str, kind: &str, extra: Value) -> Value {
    let mut frame = json!({ "type": "system", "subtype": "task_started", "task_id": task, "tool_use_id": call,
        "description": format!("Scout {task}"), "subagent_type": kind, "task_type": "local_agent",
        "prompt": "Look around" });
    frame
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    frame
}

fn child_assistant(parent: &str, message: &str, blocks: Value) -> Value {
    json!({ "type": "assistant", "parent_tool_use_id": parent, "timestamp": "2026-10-28T10:00:00.250Z",
        "message": { "id": message, "model": "claude-haiku-5", "content": blocks,
            "usage": { "input_tokens": 100, "output_tokens": 20, "cache_read_input_tokens": 3000 } } })
}

fn tool_use(id: &str, name: &str, input: Value) -> Value {
    json!([{ "type": "tool_use", "id": id, "name": name, "input": input }])
}

fn child_result(parent: &str, call: &str, content: Value, is_error: bool) -> Value {
    json!({ "type": "user", "parent_tool_use_id": parent,
        "message": { "content": [{ "type": "tool_result", "tool_use_id": call, "content": content,
            "is_error": is_error }] } })
}

fn notification(task: &str, extra: Value) -> Value {
    let mut frame = json!({ "type": "system", "subtype": "task_notification", "task_id": task,
        "status": "completed", "output_file": "" });
    frame
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    frame
}

fn turn_result() -> Value {
    json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "result": "",
        "user_message_uuid": "u1", "user_message_uuids": ["u1"] })
}

#[test]
fn a_custom_background_agent_keeps_streaming_steps_after_the_parent_turn_ended() {
    let mut mapper = mapper();
    let call = json!({ "subagent_type": "us-scout", "description": "Scout the parser", "prompt": "Look around",
        "run_in_background": true });
    feed(
        &mut mapper,
        &[
            requesting("u1"),
            spawn(&[("toolu_agent", "Agent", call)]),
            started("t1", "toolu_agent", "us-scout", json!({ "is_backgrounded": true })),
            turn_result(),
        ],
    );
    assert!(!mapper.is_working());

    let later = feed(
        &mut mapper,
        &[
            json!({ "type": "stream_event", "parent_tool_use_id": "toolu_agent",
                "event": { "type": "message_start", "message": { "id": "msg_c1" } } }),
            child_assistant(
                "toolu_agent",
                "msg_c1",
                tool_use("toolu_grep", "Grep", json!({ "pattern": "fn parse", "path": "src" })),
            ),
            child_result("toolu_agent", "toolu_grep", json!("src/a.rs\nsrc/b.rs"), false),
            child_assistant(
                "toolu_agent",
                "msg_c2",
                json!([{ "type": "text", "text": "Found two parsers." }]),
            ),
            notification("t1", json!({ "summary": "Scout finished" })),
        ],
    );

    assert!(hints(&later).is_empty());
    assert!(!mapper.is_working());
    assert!(
        kinds(&later)
            .iter()
            .all(|kind| kind == "subagent" || kind == "subagentStep"),
        "{:?}",
        kinds(&later)
    );

    let tool = |status, output: Option<&str>| SubagentStep {
        id: "toolu_grep".to_owned(),
        kind: SubagentStepKind::Tool,
        name: Some("Grep".to_owned()),
        detail: "fn parse".to_owned(),
        status,
        at: Some(1_793_181_600_250),
        output: output.map(str::to_owned),
        output_lines: output.map(|_| 2),
    };
    let text = SubagentStep {
        id: "msg_c2:0".to_owned(),
        kind: SubagentStepKind::Text,
        name: None,
        detail: "Found two parsers.".to_owned(),
        status: SubagentStepStatus::Done,
        at: Some(1_793_181_600_250),
        output: None,
        output_lines: None,
    };
    let expected = vec![
        ("t1".to_owned(), tool(SubagentStepStatus::Running, None)),
        (
            "t1".to_owned(),
            tool(SubagentStepStatus::Done, Some("src/a.rs\nsrc/b.rs")),
        ),
        ("t1".to_owned(), text),
    ];
    assert_eq!(steps(&later), expected);

    let snapshots = snapshots(&later);
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].model.as_deref(), Some("claude-haiku-5"));
    assert_eq!(snapshots[0].context_tokens, Some(3120));
    assert_eq!(snapshots[0].status, SubagentStatus::Running);
    assert_eq!(snapshots[0].can_stop, Some(true));
    assert_eq!(snapshots[0].label.as_deref(), Some("Scout t1"));
    assert_eq!(snapshots[0].message_route, Some(SubagentRoute::Orchestrator));
    assert_eq!(snapshots[0].task, None);

    assert_eq!(snapshots[1].status, SubagentStatus::Completed);
    assert_eq!(snapshots[1].can_stop, Some(false));
    assert_eq!(snapshots[1].report.as_deref(), Some("Found two parsers."));
    assert_eq!(snapshots[1].intent.as_deref(), Some("Scout finished"));
}

#[test]
fn the_report_falls_back_to_the_summary_and_a_late_final_text_completes_it() {
    let mut mapper = mapper();
    feed(&mut mapper, &[started("t1", "toolu_a", "Explore", json!({}))]);

    let ended = snapshots(&feed(
        &mut mapper,
        &[notification("t1", json!({ "summary": "Only a summary" }))],
    ));
    assert_eq!(ended[0].report.as_deref(), Some("Only a summary"));

    let late = feed(
        &mut mapper,
        &[child_assistant(
            "toolu_a",
            "msg_late",
            json!([{ "type": "text", "text": "The real report" }]),
        )],
    );
    assert_eq!(steps(&late).len(), 1);
    assert_eq!(
        snapshots(&late).last().unwrap().report.as_deref(),
        Some("The real report")
    );
}

#[test]
fn parallel_agents_of_one_message_get_their_own_interleaved_steps() {
    let mut mapper = mapper();
    let input = |name: &str| json!({ "subagent_type": "us-scout", "description": name, "prompt": name });
    let out = feed(
        &mut mapper,
        &[
            requesting("u1"),
            spawn(&[("toolu_a", "Agent", input("A")), ("toolu_b", "Agent", input("B"))]),
            started("ta", "toolu_a", "us-scout", json!({})),
            started("tb", "toolu_b", "us-scout", json!({})),
            child_assistant(
                "toolu_b",
                "msg_b",
                tool_use("c_b", "Bash", json!({ "command": "ls b" })),
            ),
            child_assistant(
                "toolu_a",
                "msg_a",
                tool_use("c_a", "Bash", json!({ "command": "ls a" })),
            ),
            child_result("toolu_a", "c_a", json!([{ "type": "text", "text": "A done" }]), false),
            child_result("toolu_b", "c_b", json!("boom"), true),
        ],
    );

    let seen: Vec<(String, String, SubagentStepStatus, Option<String>)> = steps(&out)
        .into_iter()
        .map(|(subagent, step)| (subagent, step.detail, step.status, step.output))
        .collect();

    assert_eq!(
        seen,
        [
            ("tb".to_owned(), "ls b".to_owned(), SubagentStepStatus::Running, None),
            ("ta".to_owned(), "ls a".to_owned(), SubagentStepStatus::Running, None),
            (
                "ta".to_owned(),
                "ls a".to_owned(),
                SubagentStepStatus::Done,
                Some("A done".to_owned())
            ),
            (
                "tb".to_owned(),
                "ls b".to_owned(),
                SubagentStepStatus::Failed,
                Some("boom".to_owned())
            ),
        ]
    );
}

#[test]
fn frames_before_the_task_started_wait_and_are_applied_in_order() {
    let mut mapper = mapper();
    let early = feed(
        &mut mapper,
        &[
            child_assistant(
                "toolu_a",
                "msg_1",
                tool_use("c1", "Read", json!({ "file_path": "/w/a.rs" })),
            ),
            child_result("toolu_a", "c1", json!("fn a() {}"), false),
        ],
    );
    assert!(events(&early).is_empty());

    let out = feed(&mut mapper, &[started("ta", "toolu_a", "Explore", json!({}))]);

    assert_eq!(kinds(&out)[0], "subagent");
    let seen: Vec<_> = steps(&out)
        .into_iter()
        .map(|(_, step)| (step.id, step.status))
        .collect();
    assert_eq!(
        seen,
        [
            ("c1".to_owned(), SubagentStepStatus::Running),
            ("c1".to_owned(), SubagentStepStatus::Done)
        ]
    );
    assert_eq!(
        snapshots(&out).len(),
        2,
        "the model and the context size follow the start"
    );
}

#[test]
fn only_so_many_frames_and_calls_wait_for_their_task() {
    let mut mapper = mapper();
    let frames: Vec<Value> = (0..40)
        .map(|n| {
            child_assistant(
                "toolu_busy",
                &format!("msg_{n}"),
                tool_use(&format!("c{n}"), "Bash", json!({ "command": "ls" })),
            )
        })
        .collect();
    feed(&mut mapper, &frames);

    let others: Vec<Value> = (0..10)
        .map(|n| {
            child_assistant(
                &format!("toolu_{n}"),
                "msg_x",
                tool_use(&format!("o{n}"), "Bash", json!({ "command": "ls" })),
            )
        })
        .collect();
    feed(&mut mapper, &others);

    let busy = steps(&feed(
        &mut mapper,
        &[started("busy", "toolu_busy", "Explore", json!({}))],
    ));
    assert_eq!(busy.len(), 32);
    assert_eq!(busy[0].1.id, "c0");
    assert_eq!(busy[31].1.id, "c31");

    // The busy call took one of the eight places, so seven of the ten other calls kept theirs.
    let kept = (0..10)
        .filter(|n| {
            let out = feed(
                &mut mapper,
                &[started(&format!("t{n}"), &format!("toolu_{n}"), "Explore", json!({}))],
            );
            !steps(&out).is_empty()
        })
        .count();
    assert_eq!(kept, 7);
}

#[test]
fn a_long_prompt_is_kept_once_and_cut_at_the_limit() {
    let mut mapper = mapper();
    let prompt = "p".repeat(100 * 1024);
    let out = snapshots(&feed(
        &mut mapper,
        &[started("t1", "toolu_a", "Explore", json!({ "prompt": prompt }))],
    ));
    let task = out[0].task.as_deref().unwrap();

    assert_eq!(task.chars().count(), MAX_SUBAGENT_TEXT_CHARS);
    assert!(task.ends_with('…'));

    let progress = snapshots(&feed(
        &mut mapper,
        &[json!({ "type": "system", "subtype": "task_progress", "task_id": "t1", "description": "d" })],
    ));
    assert_eq!(progress[0].task, None);
}

#[test]
fn a_long_result_keeps_its_end_and_reports_how_many_lines_it_had() {
    let mut mapper = mapper();
    feed(&mut mapper, &[started("t1", "toolu_a", "Explore", json!({}))]);
    feed(
        &mut mapper,
        &[child_assistant(
            "toolu_a",
            "msg_1",
            tool_use("c1", "Bash", json!({ "command": "seq 1000" })),
        )],
    );

    let lines: Vec<String> = (1..=1000).map(|n| format!("line {n}")).collect();
    let out = steps(&feed(
        &mut mapper,
        &[child_result("toolu_a", "c1", json!(lines.join("\n")), false)],
    ));
    let step = &out[0].1;
    let output = step.output.as_deref().unwrap();

    assert_eq!(step.output_lines, Some(1000));
    assert_eq!(output.lines().count(), MAX_STEP_OUTPUT_LINES);
    assert!(output.starts_with("line 601"));
    assert!(output.ends_with("line 1000"));
    assert_eq!(step.name.as_deref(), Some("Bash"));
    assert_eq!(step.detail, "seq 1000");

    let wide = "w".repeat(2 * MAX_STEP_OUTPUT_BYTES);
    feed(
        &mut mapper,
        &[child_assistant(
            "toolu_a",
            "msg_2",
            tool_use("c2", "Bash", json!({ "command": "wide" })),
        )],
    );
    let out = steps(&feed(&mut mapper, &[child_result("toolu_a", "c2", json!(wide), false)]));
    assert_eq!(out[0].1.output.as_deref().unwrap().len(), MAX_STEP_OUTPUT_BYTES);
}

#[test]
fn secrets_in_arguments_output_and_prose_never_reach_the_chat() {
    let mut mapper = mapper();
    let token = fake_token();
    feed(&mut mapper, &[started("t1", "toolu_a", "Explore", json!({}))]);

    let out = feed(
        &mut mapper,
        &[
            child_assistant(
                "toolu_a",
                "msg_1",
                tool_use(
                    "c1",
                    "Bash",
                    json!({ "command": format!("curl -H 'Authorization: {token}' x") }),
                ),
            ),
            child_result("toolu_a", "c1", json!(format!("token={token}\nok")), false),
            child_assistant(
                "toolu_a",
                "msg_2",
                json!([{ "type": "text", "text": format!("Found {token} in env") }]),
            ),
            notification("t1", json!({})),
        ],
    );

    let wire = serde_json::to_string(&events(&out)).unwrap();
    assert!(!wire.contains(&token), "{wire}");
    assert!(wire.contains("curl"));
    assert_eq!(steps(&out).len(), 3);
}

#[test]
fn a_subagent_that_stopped_cannot_be_stopped_again() {
    let mut mapper = mapper();
    assert_eq!(mapper.subagent_active("t1"), None);

    feed(&mut mapper, &[started("t1", "toolu_a", "Explore", json!({}))]);
    assert_eq!(mapper.subagent_active("t1"), Some(true));

    let ended = snapshots(&feed(
        &mut mapper,
        &[notification("t1", json!({ "status": "stopped" }))],
    ));
    assert_eq!(ended[0].status, SubagentStatus::Aborted);
    assert_eq!(ended[0].can_stop, Some(false));
    assert_eq!(ended[0].report, None);
    assert_eq!(mapper.subagent_active("t1"), Some(false));
}
