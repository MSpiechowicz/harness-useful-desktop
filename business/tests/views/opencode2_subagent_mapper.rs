//! The subagents of an OpenCode 2 chat as the subagent panel shows them: what the chat's `subagent`
//! calls and their child sessions say, linked, bounded and redacted.

use crate::opencode_subagent_support;
use crate::opencode2_mapper_support;

use harness_business::model::{
    MAX_STEP_OUTPUT_LINES, MAX_STEP_TEXT_CHARS, MAX_SUBAGENT_TEXT_CHARS, SubagentRoute, SubagentStatus,
    SubagentStepKind, SubagentStepStatus,
};
use harness_business::opencode_server_v2::events::Event;
use opencode_subagent_support::{fake_token, latest, numbered, shown, snapshots, steps};
use opencode2_mapper_support::{SESSION, event, events, feed, mapper, ours};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CHILD: &str = "ses_child1";
const OTHER: &str = "ses_child2";

pub(crate) fn created(id: &str, parent: &str) -> Event {
    event("session.created", json!({ "sessionID": id, "parentID": parent }))
}

/// An event of subagent session `session`.
pub(crate) fn child(session: &str, kind: &str, mut data: Value) -> Event {
    data["sessionID"] = json!(session);
    data["assistantMessageID"] = json!("msg_c1");

    event(kind, data)
}

/// The chat's `subagent` call `call`, as the model starts it.
pub(crate) fn call(call: &str, description: &str, agent: &str, prompt: &str) -> Vec<Event> {
    vec![
        ours(
            "session.tool.input.started",
            json!({ "assistantMessageID": "msg_a", "id": call, "name": "subagent" }),
        ),
        ours(
            "session.tool.called",
            json!({ "assistantMessageID": "msg_a", "id": call, "input":
                { "description": description, "agent": agent, "prompt": prompt } }),
        ),
    ]
}

/// The progress of the chat's call `call`, which names the session it runs in.
pub(crate) fn progress(call: &str, session: &str) -> Event {
    ours(
        "session.tool.progress",
        json!({ "id": call, "metadata": { "sessionID": session, "status": "running" } }),
    )
}

pub(crate) fn answered(call: &str, text: &str) -> Event {
    ours(
        "session.tool.success",
        json!({ "assistantMessageID": "msg_a", "id": call, "content": [{ "type": "text", "text": text }] }),
    )
}

pub(crate) fn started(session: &str, call: &str, tool: &str, input: Value) -> Vec<Event> {
    vec![
        child(
            session,
            "session.tool.input.started",
            json!({ "id": call, "name": tool }),
        ),
        child(session, "session.tool.called", json!({ "id": call, "input": input })),
    ]
}

pub(crate) fn result(session: &str, call: &str, text: &str) -> Event {
    child(
        session,
        "session.tool.success",
        json!({ "id": call, "content": [{ "type": "text", "text": text }] }),
    )
}

pub(crate) fn step_started(session: &str) -> Event {
    child(
        session,
        "session.step.started",
        json!({ "agent": "general", "model": { "id": "m1", "providerID": "mock" } }),
    )
}

pub(crate) fn step_ended(session: &str, tokens: Value, cost: f64) -> Event {
    child(
        session,
        "session.step.ended",
        json!({ "finish": "stop", "cost": cost, "tokens": tokens }),
    )
}

pub(crate) fn text_ended(session: &str, ordinal: u64, text: &str) -> Event {
    child(
        session,
        "session.text.ended",
        json!({ "ordinal": ordinal, "text": text }),
    )
}

fn execution(session: &str, kind: &str, data: Value) -> Event {
    child(session, kind, data)
}

pub(crate) fn collect(
    mapper: &mut harness_business::opencode_server_v2::Mapper,
    groups: Vec<Vec<Event>>,
) -> Vec<harness_business::model::AgentEvent> {
    let all: Vec<Event> = groups.into_iter().flatten().collect();

    events(feed(mapper, &all))
}

#[test]
fn a_subagent_call_and_its_child_session_make_one_subagent_with_steps_and_a_report() {
    let mut mapper = mapper();
    let usage = json!({ "input": 100, "output": 20, "reasoning": 5, "cache": { "read": 10, "write": 0 } });

    let out = collect(
        &mut mapper,
        vec![
            call("call_s", "Check the tests", "general", "Run the tests"),
            vec![created(CHILD, SESSION), progress("call_s", CHILD), step_started(CHILD)],
            started(CHILD, "call_a", "shell", json!({ "command": "cargo test\n--quiet" })),
            vec![
                result(CHILD, "call_a", "line 1\nline 2\nok"),
                text_ended(CHILD, 0, "All green."),
                step_ended(CHILD, usage, 0.002),
                execution(CHILD, "session.execution.succeeded", json!({})),
                answered(
                    "call_s",
                    "task_id: ses_child1\n\n<task_result>\nAll tests pass.\n</task_result>",
                ),
            ],
        ],
    );

    let all = snapshots(&out, CHILD);
    assert_eq!(
        all[0].label.as_deref(),
        Some("Check the tests"),
        "the only open call is the child's"
    );
    assert_eq!(all[0].task.as_deref(), Some("Run the tests"));
    assert!(
        all[1..].iter().all(|snapshot| snapshot.task.is_none()),
        "the brief is sent once"
    );

    let last = latest(&out, CHILD);
    assert_eq!(last.parent_tool_call_id.as_deref(), Some("call_s"));
    assert_eq!(
        (last.agent.as_str(), last.status),
        ("general", SubagentStatus::Completed)
    );
    assert_eq!(last.model.as_deref(), Some("mock/m1"));
    assert_eq!(
        (last.tokens, last.context_tokens, last.cost_usd),
        (Some(135), Some(135), Some(0.002))
    );
    assert_eq!((last.tool_count, last.current_tool), (Some(1), None));
    assert_eq!(
        last.report.as_deref(),
        Some("All tests pass."),
        "the answer inside the wrapper"
    );
    assert_eq!(
        (last.message_route, last.can_stop),
        (Some(SubagentRoute::Direct), Some(false))
    );

    let steps = steps(&out, CHILD);
    assert_eq!(
        steps
            .iter()
            .map(|step| (step.status, step.detail.as_str()))
            .collect::<Vec<_>>(),
        [
            (SubagentStepStatus::Running, ""),
            (SubagentStepStatus::Running, "cargo test --quiet"),
            (SubagentStepStatus::Done, "cargo test --quiet"),
            (SubagentStepStatus::Done, "All green."),
        ],
        "a call is named when its input starts, shows its input, and keeps both when it ends"
    );
    assert_eq!(
        (steps[2].name.as_deref(), steps[2].at, steps[2].id.as_str()),
        (Some("shell"), Some(1), "call_a")
    );
    assert_eq!(
        (steps[2].output.as_deref(), steps[2].output_lines),
        (Some("line 1\nline 2\nok"), Some(3))
    );
    assert_eq!(
        (steps[3].kind, steps[3].id.as_str()),
        (SubagentStepKind::Text, "msg_c1:0")
    );

    assert_eq!(mapper.subagent_active(CHILD), Some(false));
}

#[test]
fn parallel_calls_link_by_progress_and_their_interleaved_events_stay_apart() {
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            call("call_a", "Find the bug", "explore", "Look"),
            call("call_b", "Write the docs", "docs", "Write"),
            vec![created(CHILD, SESSION), created(OTHER, SESSION), step_started(OTHER)],
            started(OTHER, "call_o", "grep", json!({ "pattern": "docs" })),
            vec![progress("call_b", OTHER)],
            started(CHILD, "call_c", "shell", json!({ "command": "ls" })),
            vec![progress("call_a", CHILD), result(OTHER, "call_o", "found")],
        ],
    );

    let first = &snapshots(&out, CHILD)[0];
    assert_eq!(
        (first.label.as_deref(), first.agent.as_str()),
        (None, "subagent"),
        "two open calls are ambiguous"
    );

    let (one, two) = (latest(&out, CHILD), latest(&out, OTHER));
    assert_eq!(
        (
            one.label.as_deref(),
            one.agent.as_str(),
            one.parent_tool_call_id.as_deref()
        ),
        (Some("Find the bug"), "explore", Some("call_a"))
    );
    // The agent the server reports for the child wins over the one the parent's call asked for.
    assert_eq!(
        (
            two.label.as_deref(),
            two.agent.as_str(),
            two.parent_tool_call_id.as_deref()
        ),
        (Some("Write the docs"), "general", Some("call_b"))
    );

    let ids = |session: &str| -> Vec<(String, SubagentStepStatus)> {
        steps(&out, session)
            .into_iter()
            .map(|step| (step.id, step.status))
            .collect()
    };
    assert_eq!(ids(CHILD)[0].0, "call_c");
    assert!(ids(CHILD).iter().all(|(id, _)| id == "call_c"));
    assert_eq!(
        ids(OTHER).last(),
        Some(&("call_o".to_owned(), SubagentStepStatus::Done))
    );
    assert_eq!(
        steps(&out, OTHER).last().map(|step| step.detail.clone()),
        Some("docs".to_owned())
    );
}

#[test]
fn a_child_the_chat_only_adopted_still_gets_its_calls_label_from_the_progress() {
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            call("call_s", "Check the tests", "general", "Run the tests"),
            vec![progress("call_s", CHILD)],
        ],
    );
    assert_eq!(
        snapshots(&out, CHILD),
        [],
        "a session the progress merely names is not a subagent yet"
    );
    assert_eq!(mapper.subagent_active(CHILD), None);
    assert_eq!(mapper.take_unlisted(), [CHILD]);

    mapper.adopt(CHILD, SESSION, None);
    let out = collect(&mut mapper, vec![vec![step_started(CHILD)]]);

    let last = latest(&out, CHILD);
    assert_eq!(
        (last.label.as_deref(), last.agent.as_str()),
        (Some("Check the tests"), "general")
    );
    assert_eq!(snapshots(&out, CHILD)[0].task.as_deref(), Some("Run the tests"));
    assert_eq!(mapper.subagent_active(CHILD), Some(true));
}

#[test]
fn an_interrupted_child_ends_aborted_and_its_open_calls_fail() {
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            vec![created(CHILD, SESSION), step_started(CHILD)],
            started(CHILD, "call_a", "shell", json!({ "command": "sleep 99" })),
            vec![execution(
                CHILD,
                "session.execution.interrupted",
                json!({ "reason": "superseded" }),
            )],
        ],
    );
    assert_eq!(
        latest(&out, CHILD).status,
        SubagentStatus::Running,
        "a newer input took its place"
    );

    let out = collect(
        &mut mapper,
        vec![
            vec![execution(
                CHILD,
                "session.execution.interrupted",
                json!({ "reason": "user" }),
            )],
            vec![execution(CHILD, "session.execution.succeeded", json!({}))],
        ],
    );

    let ended = steps(&out, CHILD);
    assert_eq!(ended.len(), 1);
    assert_eq!(
        (ended[0].id.as_str(), ended[0].status, ended[0].detail.as_str()),
        ("call_a", SubagentStepStatus::Failed, "sleep 99")
    );

    let last = latest(&out, CHILD);
    assert_eq!(
        (last.status, last.can_stop, last.current_tool),
        (SubagentStatus::Aborted, Some(false), None)
    );
    assert_eq!(
        snapshots(&out, CHILD).len(),
        1,
        "finishing after an abort changes nothing"
    );
}

#[test]
fn a_failed_call_fails_its_subagent_and_an_aborted_one_aborts_it() {
    let mut mapper = mapper();
    let failed = |call: &str, kind: &str| {
        ours(
            "session.tool.failed",
            json!({ "assistantMessageID": "msg_a", "id": call, "error": { "type": kind, "message": "No" } }),
        )
    };

    let out = collect(
        &mut mapper,
        vec![
            call("call_a", "One", "general", "1"),
            vec![
                created(CHILD, SESSION),
                progress("call_a", CHILD),
                failed("call_a", "error"),
            ],
            call("call_b", "Two", "general", "2"),
            vec![
                created(OTHER, SESSION),
                progress("call_b", OTHER),
                failed("call_b", "aborted"),
            ],
        ],
    );

    assert_eq!(latest(&out, CHILD).status, SubagentStatus::Failed);
    assert_eq!(latest(&out, OTHER).status, SubagentStatus::Aborted);
}

#[test]
fn a_failed_child_call_shows_what_it_wrote_and_its_error() {
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            vec![created(CHILD, SESSION)],
            started(CHILD, "call_a", "shell", json!({ "command": "make" })),
            vec![child(
                CHILD,
                "session.tool.failed",
                json!({ "id": "call_a", "error": { "type": "error", "message": "exit 2" },
                    "content": [{ "type": "text", "text": "compiling" }] }),
            )],
        ],
    );

    let last = steps(&out, CHILD).pop().expect("a step");
    assert_eq!(
        (last.status, last.output.as_deref(), last.output_lines),
        (SubagentStepStatus::Failed, Some("compiling\nexit 2"), Some(2))
    );
}

#[test]
fn texts_are_bounded_and_secrets_never_reach_the_panel() {
    let token = fake_token();
    let mut mapper = mapper();
    let brief = "x".repeat(100_000);
    let command = format!("echo {token} {}", "y".repeat(1000));
    let report = format!("<task_result>\nKey {token}\n{}\n</task_result>", "r".repeat(100_000));

    let out = collect(
        &mut mapper,
        vec![
            call("call_s", &format!("Use {token}"), "general", &brief),
            vec![created(CHILD, SESSION), progress("call_s", CHILD)],
            started(CHILD, "call_a", "shell", json!({ "command": command })),
            vec![
                result(CHILD, "call_a", &format!("{}\n{token}", numbered("line", 1000))),
                text_ended(CHILD, 0, &format!("{token} {}", "z".repeat(10_000))),
                answered("call_s", &report),
            ],
        ],
    );

    assert!(!shown(&out).contains(&token), "no panel event carries the token");

    let first = &snapshots(&out, CHILD)[0];
    assert_eq!(
        first.task.as_ref().map(|task| task.chars().count()),
        Some(MAX_SUBAGENT_TEXT_CHARS)
    );
    assert_eq!(
        latest(&out, CHILD).report.as_ref().map(|report| report.chars().count()),
        Some(MAX_SUBAGENT_TEXT_CHARS)
    );

    let steps = steps(&out, CHILD);
    let done = steps
        .iter()
        .find(|step| step.status == SubagentStepStatus::Done && step.id == "call_a")
        .expect("done");
    let output = done.output.as_deref().expect("output");
    assert_eq!(done.output_lines, Some(1001), "the lines before the cut");
    assert_eq!(output.lines().count(), MAX_STEP_OUTPUT_LINES);
    assert!(
        output.starts_with("line 602") && output.ends_with("••••"),
        "the end of the output: {output}"
    );

    let text = steps.last().expect("text");
    assert_eq!(
        (text.kind, text.detail.chars().count()),
        (SubagentStepKind::Text, MAX_STEP_TEXT_CHARS)
    );
}

#[test]
fn a_chat_run_that_ends_does_not_end_a_running_subagent_and_other_sessions_are_not_subagents() {
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            vec![created(CHILD, SESSION), created(OTHER, "ses_stranger")],
            vec![
                ours("session.execution.started", json!({})),
                ours("session.execution.succeeded", json!({})),
            ],
            vec![step_started(OTHER)],
        ],
    );

    assert_eq!(latest(&out, CHILD).status, SubagentStatus::Running);
    assert_eq!(
        snapshots(&out, OTHER),
        [],
        "a session started by a stranger is not the chat's"
    );
    assert_eq!(mapper.subagent_active(OTHER), None);
}
