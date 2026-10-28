//! The subagents of an OpenCode chat as the subagent panel shows them: what the chat's `task` calls
//! and their child sessions say, linked, bounded and redacted.

use crate::opencode_mapper_support;
use crate::opencode_subagent_support;

use harness_business::model::{
    MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_LINES, MAX_STEP_TEXT_CHARS, MAX_SUBAGENT_TEXT_CHARS, SubagentRoute,
    SubagentStatus, SubagentStepKind, SubagentStepStatus,
};
use opencode_mapper_support::{SESSION, events, feed, mapper, part, status};
use opencode_subagent_support::crafted::{aborted, child_status, created, linked, prose, reply, task, tool};
use opencode_subagent_support::{fake_token, latest, numbered, shown, snapshots, steps};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CHILD: &str = "ses_child1";
const OTHER: &str = "ses_child2";

#[test]
fn a_task_call_and_its_child_session_make_one_subagent_with_steps_and_a_report() {
    let mut mapper = mapper();
    let usage = json!({ "input": 100, "output": 20, "reasoning": 5, "cache": { "read": 10, "write": 0 } });
    let running = json!({ "command": "cargo test\n--quiet", "description": "Run" });

    let out = events(feed(
        &mut mapper,
        &[
            task("call_t1", "Check the tests", "general", json!({})),
            created(CHILD, SESSION),
            reply(CHILD, "msg_c1", usage, 0.002),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "running", "input": running, "time": { "start": 11 } }),
            ),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "completed", "input": running, "output": "line 1\nline 2\nok",
                    "time": { "start": 11, "end": 12 } }),
            ),
            prose(CHILD, "prt_x", "msg_c1", "All green.", true),
            child_status(CHILD, "idle"),
            task(
                "call_t1",
                "Check the tests",
                "general",
                json!({ "status": "completed", "metadata": { "sessionId": CHILD },
                    "output": "task_id: ses_child1 (for resuming)\n\n<task_result>\nAll tests pass.\n</task_result>" }),
            ),
        ],
    ));

    let all = snapshots(&out, CHILD);
    assert_eq!(
        all[0].label.as_deref(),
        Some("Check the tests"),
        "the only open call is the child's"
    );
    assert_eq!(all[0].task.as_deref(), Some("Brief of Check the tests"));
    assert!(
        all[1..].iter().all(|snapshot| snapshot.task.is_none()),
        "the brief is sent once"
    );

    let last = latest(&out, CHILD);
    assert_eq!(last.parent_tool_call_id.as_deref(), Some("call_t1"));
    assert_eq!(last.agent, "general");
    assert_eq!(last.description.as_deref(), Some("Check the tests"));
    assert_eq!(last.status, SubagentStatus::Completed);
    assert_eq!(last.model.as_deref(), Some("mock/m1"));
    assert_eq!(
        (last.tokens, last.context_tokens, last.cost_usd),
        (Some(135), Some(135), Some(0.002))
    );
    assert_eq!(last.tool_count, Some(1));
    assert_eq!(last.current_tool, None, "nothing runs any more");
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
        steps.len(),
        3,
        "a call arrives running and done, the text once: {steps:#?}"
    );
    assert_eq!(
        (steps[0].status, steps[0].detail.as_str()),
        (SubagentStepStatus::Running, "cargo test --quiet")
    );
    assert_eq!(
        (steps[0].name.as_deref(), steps[0].at, steps[0].output.as_deref()),
        (Some("bash"), Some(11), None)
    );
    assert_eq!(steps[1].id, "call_a");
    assert_eq!(steps[1].status, SubagentStepStatus::Done);
    assert_eq!(
        (steps[1].output.as_deref(), steps[1].output_lines),
        (Some("line 1\nline 2\nok"), Some(3))
    );
    assert_eq!(
        (steps[2].kind, steps[2].detail.as_str()),
        (SubagentStepKind::Text, "All green.")
    );

    assert_eq!(mapper.subagent_active(CHILD), Some(false));
    let settings = mapper.subagent_settings(CHILD).expect("settings");
    assert_eq!(settings.agent.as_deref(), Some("general"));
    assert_eq!(
        settings.model.map(|model| (model.provider, model.id)),
        Some(("mock".to_owned(), "m1".to_owned()))
    );
}

#[test]
fn parallel_calls_link_by_metadata_and_their_interleaved_events_stay_apart() {
    let mut mapper = mapper();
    let usage = json!({ "input": 1, "output": 1 });
    let running = |command: &str| json!({ "status": "running", "input": { "command": command } });

    let out = events(feed(
        &mut mapper,
        &[
            task("call_a", "Find the bug", "explore", json!({})),
            task("call_b", "Write the docs", "docs", json!({})),
            created(CHILD, SESSION),
            created(OTHER, SESSION),
            reply(OTHER, "msg_o1", usage, 0.0),
            tool(OTHER, "call_o", "grep", running("grep -rn docs")),
            task("call_b", "Write the docs", "docs", linked(OTHER)),
            tool(CHILD, "call_c", "bash", running("ls")),
            task("call_a", "Find the bug", "explore", linked(CHILD)),
            tool(
                OTHER,
                "call_o",
                "grep",
                json!({ "status": "completed", "input": { "command": "grep -rn docs" }, "output": "x" }),
            ),
        ],
    ));

    let first = &snapshots(&out, CHILD)[0];
    assert_eq!(
        (first.label.as_deref(), first.agent.as_str()),
        (None, "subagent"),
        "two open calls are ambiguous"
    );

    let (child, other) = (latest(&out, CHILD), latest(&out, OTHER));
    assert_eq!(
        (
            child.label.as_deref(),
            child.agent.as_str(),
            child.parent_tool_call_id.as_deref()
        ),
        (Some("Find the bug"), "explore", Some("call_a"))
    );
    // The agent the server reports for the child wins over the one the parent's call asked for.
    assert_eq!(
        (
            other.label.as_deref(),
            other.agent.as_str(),
            other.parent_tool_call_id.as_deref()
        ),
        (Some("Write the docs"), "general", Some("call_b"))
    );
    assert_eq!((child.index, other.index), (0, 1), "in the order they started");

    let child_steps: Vec<_> = steps(&out, CHILD)
        .into_iter()
        .map(|step| (step.id, step.detail))
        .collect();
    let other_steps: Vec<_> = steps(&out, OTHER)
        .into_iter()
        .map(|step| (step.id, step.detail))
        .collect();
    assert_eq!(child_steps, [("call_c".to_owned(), "ls".to_owned())]);
    assert_eq!(
        other_steps,
        [
            ("call_o".to_owned(), "grep -rn docs".to_owned()),
            ("call_o".to_owned(), "grep -rn docs".to_owned()),
        ],
        "the end keeps what the running step showed"
    );
    assert_eq!(mapper.subagent_active(CHILD), Some(true));
}

#[test]
fn a_child_without_a_task_call_is_still_a_subagent_and_user_messages_are_not_steps() {
    let mut mapper = mapper();
    let user = json!({ "type": "message.updated", "properties": { "sessionID": CHILD, "info": {
        "id": "msg_u1", "role": "user", "time": { "created": 1 } } } });

    let out = events(feed(
        &mut mapper,
        &[
            created(CHILD, SESSION),
            user,
            prose(CHILD, "prt_u", "msg_u1", "The brief the parent wrote", true),
            reply(CHILD, "msg_c1", json!({ "output": 3, "input": 4 }), 0.0),
            prose(CHILD, "prt_draft", "msg_c1", "still writing", false),
            tool(CHILD, "call_a", "read", json!({ "status": "pending", "input": {} })),
            tool(
                CHILD,
                "call_a",
                "read",
                json!({ "status": "running", "input": { "filePath": "a.rs" } }),
            ),
            tool(
                CHILD,
                "call_a",
                "read",
                json!({ "status": "running", "input": { "filePath": "a.rs" },
                "metadata": { "output": "partial" } }),
            ),
        ],
    ));

    let last = latest(&out, CHILD);
    assert_eq!(
        (last.agent.as_str(), last.label, last.parent_tool_call_id),
        ("general", None, None)
    );
    assert_eq!(last.status, SubagentStatus::Running);
    assert_eq!(
        (last.current_tool.as_deref(), last.current_args.as_deref()),
        (Some("read"), Some("a.rs"))
    );
    assert_eq!(last.can_stop, Some(true));

    let steps = steps(&out, CHILD);
    assert_eq!(
        steps.iter().map(|step| step.detail.as_str()).collect::<Vec<_>>(),
        ["", "a.rs"],
        "the brief and an unfinished text are no steps, and a repeat of a step is not sent again"
    );
}

#[test]
fn a_stopped_child_ends_aborted_and_its_open_calls_fail() {
    let mut mapper = mapper();

    let out = events(feed(
        &mut mapper,
        &[
            created(CHILD, SESSION),
            reply(CHILD, "msg_c1", json!({}), 0.0),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "running", "input": { "command": "sleep 99" },
                "time": { "start": 5 } }),
            ),
            aborted(CHILD),
            child_status(CHILD, "idle"),
        ],
    ));

    let steps = steps(&out, CHILD);
    let ended = steps.last().expect("a step");
    assert_eq!(
        (ended.id.as_str(), ended.status),
        ("call_a", SubagentStepStatus::Failed)
    );
    assert_eq!(
        (ended.detail.as_str(), ended.at),
        ("sleep 99", Some(5)),
        "it keeps what it showed"
    );

    let last = latest(&out, CHILD);
    assert_eq!(
        (last.status, last.can_stop, last.current_tool),
        (SubagentStatus::Aborted, Some(false), None)
    );
    assert_eq!(mapper.subagent_active(CHILD), Some(false));

    let again = events(feed(&mut mapper, &[child_status(CHILD, "idle")]));
    assert_eq!(
        snapshots(&again, CHILD),
        [],
        "going idle after an abort changes nothing"
    );
}

#[test]
fn a_failed_call_fails_its_subagent_and_an_aborted_one_aborts_it() {
    let mut mapper = mapper();

    let out = events(feed(
        &mut mapper,
        &[
            task("call_a", "One", "general", json!({})),
            created(CHILD, SESSION),
            task(
                "call_a",
                "One",
                "general",
                json!({ "status": "error", "error": "Tool execution failed",
                "metadata": { "sessionId": CHILD } }),
            ),
            task("call_b", "Two", "general", json!({})),
            created(OTHER, SESSION),
            task(
                "call_b",
                "Two",
                "general",
                json!({ "status": "error", "error": "Tool execution aborted",
                "metadata": { "sessionId": OTHER } }),
            ),
        ],
    ));

    assert_eq!(latest(&out, CHILD).status, SubagentStatus::Failed);
    assert_eq!(latest(&out, OTHER).status, SubagentStatus::Aborted);
    assert_eq!(latest(&out, CHILD).report, None);
}

#[test]
fn the_last_text_is_the_report_when_the_call_has_no_output() {
    let mut mapper = mapper();

    let out = events(feed(
        &mut mapper,
        &[
            created(CHILD, SESSION),
            reply(CHILD, "msg_c1", json!({}), 0.0),
            prose(CHILD, "prt_1", "msg_c1", "First thought", true),
            prose(CHILD, "prt_2", "msg_c1", "Final answer", true),
            child_status(CHILD, "busy"),
            child_status(CHILD, "idle"),
        ],
    ));

    let last = latest(&out, CHILD);
    assert_eq!(
        (last.status, last.report.as_deref()),
        (SubagentStatus::Completed, Some("Final answer"))
    );
    assert_eq!(
        latest(&out[..out.len() - 4], CHILD).report,
        None,
        "a subagent at work has no report yet"
    );
}

#[test]
fn texts_are_bounded_and_secrets_never_reach_the_panel() {
    let token = fake_token();
    let mut mapper = mapper();
    let brief = "x".repeat(100_000);
    let call = |state: Value| {
        part(
            SESSION,
            json!({ "id": "prt_t", "messageID": "msg_parent", "type": "tool", "tool": "task", "callID": "call_t",
                "state": state }),
        )
    };
    let input = json!({ "description": format!("Use {token}"), "prompt": brief, "subagent_type": "general" });
    let long_command = format!("echo {token} {}", "y".repeat(1000));

    let out = events(feed(
        &mut mapper,
        &[
            call(json!({ "status": "running", "input": input })),
            created(CHILD, SESSION),
            reply(CHILD, "msg_c1", json!({}), 0.0),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "running", "input": { "command": long_command } }),
            ),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "completed", "input": { "command": long_command },
                    "output": format!("{}\n{token}", numbered("line", 1000)) }),
            ),
            prose(
                CHILD,
                "prt_x",
                "msg_c1",
                &format!("{token} {}", "z".repeat(10_000)),
                true,
            ),
            call(
                json!({ "status": "completed", "input": input, "metadata": { "sessionId": CHILD },
                "output": format!("<task_result>\nKey {token}\n{}\n</task_result>", "r".repeat(100_000)) }),
            ),
        ],
    ));

    assert!(!shown(&out).contains(&token), "no panel event carries the token");

    let last = latest(&out, CHILD);
    assert_eq!(
        last.task.as_ref().map(|task| task.chars().count()),
        None,
        "the brief went with the first snapshot"
    );
    let first = &snapshots(&out, CHILD)[0];
    assert_eq!(
        first.task.as_ref().map(|task| task.chars().count()),
        Some(MAX_SUBAGENT_TEXT_CHARS)
    );
    assert_eq!(
        last.report.as_ref().map(|report| report.chars().count()),
        Some(MAX_SUBAGENT_TEXT_CHARS)
    );
    assert!(
        last.label
            .as_deref()
            .is_some_and(|label| label.chars().count() <= MAX_STEP_DETAIL_CHARS)
    );

    let steps = steps(&out, CHILD);
    assert!(
        steps
            .iter()
            .all(|step| step.detail.chars().count() <= MAX_STEP_TEXT_CHARS)
    );
    assert!(steps[0].detail.chars().count() <= MAX_STEP_DETAIL_CHARS);

    let done = &steps[1];
    let output = done.output.as_deref().expect("output");
    assert_eq!(done.output_lines, Some(1001), "the lines before the cut");
    assert_eq!(output.lines().count(), MAX_STEP_OUTPUT_LINES);
    assert!(
        output.starts_with("line 602") && output.ends_with("••••"),
        "the end of the output: {output}"
    );

    assert_eq!(steps[2].kind, SubagentStepKind::Text);
    assert_eq!(steps[2].detail.chars().count(), MAX_STEP_TEXT_CHARS);
}

#[test]
fn a_chat_that_goes_idle_does_not_end_the_work_of_a_running_subagent() {
    let mut mapper = mapper();

    let out = events(feed(
        &mut mapper,
        &[created(CHILD, SESSION), status("busy"), status("idle")],
    ));

    assert_eq!(latest(&out, CHILD).status, SubagentStatus::Running);
    assert_eq!(mapper.subagent_active("ses_unknown"), None);
}
