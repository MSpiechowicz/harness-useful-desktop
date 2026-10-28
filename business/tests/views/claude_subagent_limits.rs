//! How many subagents a Claude chat remembers: a long chat with many subagents (or a stream that
//! invents them) keeps a bounded number, forgets finished ones oldest first and never forgets one
//! that still runs. A subagent's report is sent once.

use crate::claude_frames;

use claude_frames::{feed, mapper};
use harness_business::claude_stream::mapper::events;
use harness_business::model::AgentEvent;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// More subagents than a chat remembers.
const MANY: usize = 300;

fn started(task: &str) -> Value {
    json!({ "type": "system", "subtype": "task_started", "task_id": task, "tool_use_id": format!("call_{task}"),
        "description": "Scout", "subagent_type": "us-scout", "task_type": "local_agent", "prompt": "Look around" })
}

fn finished(task: &str, summary: &str) -> Value {
    json!({ "type": "system", "subtype": "task_notification", "task_id": task, "status": "completed",
        "summary": summary, "output_file": "" })
}

#[test]
fn finished_subagents_are_forgotten_oldest_first_and_a_running_one_never_is() {
    let mut mapper = mapper();

    // The oldest subagent still runs while many others start and finish after it.
    feed(&mut mapper, &[started("long_runner")]);

    for number in 0..MANY {
        let task = format!("t{number}");
        feed(&mut mapper, &[started(&task), finished(&task, "done")]);
    }

    assert_eq!(
        mapper.subagent_active("long_runner"),
        Some(true),
        "a running subagent stays"
    );
    assert_eq!(
        mapper.subagent_active("t0"),
        None,
        "the oldest finished one is forgotten"
    );
    assert_eq!(
        mapper.subagent_active(&format!("t{}", MANY - 1)),
        Some(false),
        "the newest finished one is kept"
    );

    // Frames of a forgotten subagent are ignored instead of reviving it.
    let late = feed(
        &mut mapper,
        &[json!({ "type": "system", "subtype": "task_progress", "task_id": "t0", "description": "x" })],
    );
    assert!(events(&late).is_empty());
    assert_eq!(mapper.subagent_active("t0"), None);
}

#[test]
fn subagents_that_all_run_are_all_kept_up_to_the_limit_and_the_rest_is_not_tracked() {
    let mut mapper = mapper();

    for number in 0..MANY {
        feed(&mut mapper, &[started(&format!("t{number}"))]);
    }

    let tracked = (0..MANY)
        .filter(|number| mapper.subagent_active(&format!("t{number}")) == Some(true))
        .count();

    assert_eq!(tracked, 256);
    assert_eq!(mapper.subagent_active("t0"), Some(true), "the running ones stay");
    assert_eq!(
        mapper.subagent_active("t299"),
        None,
        "the one past the limit is not tracked"
    );
}

#[test]
fn a_report_is_sent_once() {
    let mut mapper = mapper();
    let text = json!({ "type": "assistant", "parent_tool_use_id": "call_t1", "message": { "id": "msg_c1",
        "model": "claude-haiku-5", "content": [{ "type": "text", "text": "The answer." }] } });

    feed(&mut mapper, &[started("t1"), text]);

    let reports = |out: Vec<AgentEvent>| -> Vec<Option<String>> {
        out.into_iter()
            .filter_map(|event| match event {
                AgentEvent::Subagent { subagent } => Some(subagent.report),
                _ => None,
            })
            .collect()
    };

    let first = reports(events(&feed(&mut mapper, &[finished("t1", "")])));
    assert_eq!(first, vec![Some("The answer.".to_owned())]);

    let again = reports(events(&feed(&mut mapper, &[finished("t1", "")])));
    assert_eq!(
        again,
        vec![None],
        "the finished subagent does not keep the text to send it again"
    );
}

/// The reports a run of frames sends, one per snapshot.
fn reports(out: Vec<AgentEvent>) -> Vec<Option<String>> {
    out.into_iter()
        .filter_map(|event| match event {
            AgentEvent::Subagent { subagent } => Some(subagent.report),
            _ => None,
        })
        .collect()
}

#[test]
fn a_report_already_sent_is_never_replaced_by_a_later_summary() {
    let text = |id: &str| {
        json!({ "type": "assistant", "parent_tool_use_id": "call_t1", "message": { "id": id,
            "model": "claude-haiku-5", "content": [{ "type": "text", "text": "The full answer." }] } })
    };
    let completed = json!({ "type": "system", "subtype": "task_updated", "task_id": "t1",
        "patch": { "status": "completed" } });

    // The report came with the notification, and a second notification follows.
    let mut chat = mapper();
    feed(&mut chat, &[started("t1"), text("msg_1")]);
    assert_eq!(
        reports(events(&feed(&mut chat, &[finished("t1", "Short summary")]))),
        vec![Some("The full answer.".to_owned())]
    );
    assert_eq!(
        reports(events(&feed(&mut chat, &[finished("t1", "Short summary")]))),
        vec![None],
        "a second notification keeps the full report"
    );

    // The final text arrived after the subagent completed, before its notification.
    let mut chat = mapper();
    feed(&mut chat, &[started("t1"), completed]);
    assert_eq!(
        reports(events(&feed(&mut chat, &[text("msg_2")]))),
        vec![Some("The full answer.".to_owned())]
    );
    assert_eq!(
        reports(events(&feed(&mut chat, &[finished("t1", "Short summary")]))),
        vec![None],
        "the notification keeps the late report"
    );
}

/// A frame of the subagent that call `call` spawned: a text of API message `message`.
fn child_text(call: &str, message: &str) -> Value {
    json!({ "type": "assistant", "parent_tool_use_id": call, "message": { "id": message,
        "model": "claude-haiku-5", "content": [{ "type": "text", "text": "Found it." }] } })
}

#[test]
fn frames_of_refused_subagents_do_not_hold_the_slots_of_those_that_wait_for_their_task() {
    let mut mapper = mapper();

    for number in 0..256 {
        feed(&mut mapper, &[started(&format!("t{number}"))]);
    }

    // Past the limit: the tasks are refused, and so are their frames (more calls than slots).
    for number in 300..320 {
        let task = format!("t{number}");
        feed(&mut mapper, &[started(&task)]);
        feed(
            &mut mapper,
            &[child_text(&format!("call_{task}"), &format!("msg_{number}"))],
        );
    }

    // A subagent finishes, and a new one's frames arrive before its task starts.
    feed(&mut mapper, &[finished("t0", "done")]);
    feed(&mut mapper, &[child_text("call_late", "msg_late")]);
    let out = feed(&mut mapper, &[started("late")]);

    let shown: Vec<String> = events(&out)
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::SubagentStep { subagent_id, step } if subagent_id == "late" => Some(step.detail),
            _ => None,
        })
        .collect();

    assert_eq!(shown, ["Found it."], "the frames that waited for the task still show");
}
