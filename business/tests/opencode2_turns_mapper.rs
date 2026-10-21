//! The OpenCode 2 mapper's turns: errors, retries, the status-only fallback, aborts, held
//! follow-ups and steering, as the OpenCode 1 mapper reports them. Crafted events of one session.

mod opencode2_mapper_support;
mod opencode2_recorded;

use harness_business::model::{AgentEvent, MessageRole, NoticeLevel, PromptStatus};
use harness_business::opencode_server::wire::Outgoing;
use opencode2_mapper_support::{ended, events, feed, mapper, ours, shape};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn a_failed_execution_fails_the_turn_with_its_first_line() {
    let mut mapper = mapper();
    mapper.prompt_sent();

    let out = feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours(
                "session.step.failed",
                json!({ "assistantMessageID": "msg_a", "error": { "type": "provider", "message": "Rate limited\n  at stack" } }),
            ),
            ours(
                "session.execution.failed",
                json!({ "error": { "type": "provider", "message": "Something else" } }),
            ),
        ],
    );

    assert_eq!(
        shape(&out),
        [
            "taken",
            "agentStart",
            "messageStart",
            "messageEnd",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );

    let events = events(out);
    let reply = &ended(&events, MessageRole::Assistant)[0];
    assert_eq!(reply.stop_reason.as_deref(), Some("error"));
    assert_eq!(reply.error_message.as_deref(), Some("Rate limited\n  at stack"));

    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Error,
        error: Some("Rate limited".to_owned()),
        retryable: false,
    }));
}

#[test]
fn a_failure_outside_a_turn_is_a_notice_unless_it_repeats_the_last_error() {
    let mut mapper = mapper();
    let failed = ours(
        "session.execution.failed",
        json!({ "error": { "type": "provider", "message": "Model not found" } }),
    );

    feed(
        &mut mapper,
        &[ours("session.execution.started", json!({})), failed.clone()],
    );
    assert!(
        feed(&mut mapper, std::slice::from_ref(&failed)).is_empty(),
        "the turn's error, repeated"
    );

    assert_eq!(
        events(feed(&mut mapper, &[failed])),
        [AgentEvent::Notice {
            level: NoticeLevel::Error,
            message: "Model not found".to_owned()
        }]
    );
}

#[test]
fn a_status_only_server_still_runs_turns_and_retries() {
    let mut mapper = mapper();
    mapper.prompt_sent();

    let out = feed(
        &mut mapper,
        &[
            ours("session.status", json!({ "status": { "type": "busy" } })),
            ours(
                "session.status",
                json!({ "status": { "type": "retry", "attempt": 2, "message": "Overloaded\nmore", "next": 5 } }),
            ),
            ours("session.status", json!({ "status": { "type": "busy" } })),
            ours(
                "session.retry.scheduled",
                json!({ "attempt": 3, "at": 9, "error": { "message": "Again" } }),
            ),
            ours("session.step.started", json!({ "assistantMessageID": "msg_b" })),
            ours("session.status", json!({ "status": { "type": "idle" } })),
            ours("session.execution.succeeded", json!({})),
        ],
    );

    let retries: Vec<(String, Option<u32>, Option<String>)> = events(out.clone())
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Retry {
                phase,
                attempt,
                message,
                ..
            } => Some((phase, attempt, message)),
            _ => None,
        })
        .collect();
    assert_eq!(
        retries,
        [
            ("start".to_owned(), Some(2), Some("Overloaded".to_owned())),
            ("end".to_owned(), Some(2), None),
            ("start".to_owned(), Some(3), Some("Again".to_owned())),
            ("end".to_owned(), Some(3), None)
        ]
    );

    let shapes = shape(&out);
    assert_eq!(shapes.iter().filter(|kind| *kind == "agentStart").count(), 1);
    assert_eq!(shapes.iter().filter(|kind| *kind == "settled").count(), 1);
    assert!(!mapper.is_working());
}

#[test]
fn an_abort_before_the_run_started_interrupts_it_when_it_starts() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    mapper.abort_requested();

    let out = feed(&mut mapper, &[ours("session.execution.started", json!({}))]);
    assert_eq!(shape(&out), ["taken", "agentStart", "abort"]);

    let out = feed(&mut mapper, &[ours("session.execution.succeeded", json!({}))]);
    assert_eq!(events(out).last(), Some(&AgentEvent::Settled { aborted: Some(true) }));
}

#[test]
fn held_follow_ups_run_after_the_turn_and_settle_once_done() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[ours("session.execution.started", json!({}))]);

    let held = mapper.queue_follow_up(Outgoing::new("next one", &[]));
    assert_eq!(
        held,
        [AgentEvent::QueueUpdate {
            steering: Vec::new(),
            follow_up: vec!["next one".to_owned()],
            live_steered: 0
        }]
    );

    let out = feed(&mut mapper, &[ours("session.execution.succeeded", json!({}))]);
    assert_eq!(
        shape(&out),
        ["agentEnd", "promptResult", "queueUpdate", "send:next one"]
    );
    assert!(mapper.is_working(), "the follow-up is on its way");

    let out = feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours("session.execution.succeeded", json!({})),
        ],
    );
    assert_eq!(
        shape(&out),
        ["taken", "agentStart", "agentEnd", "promptResult", "settled"]
    );
}

#[test]
fn an_aborted_turn_keeps_its_follow_ups_and_settles_at_once() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[ours("session.execution.started", json!({}))]);
    mapper.queue_follow_up(Outgoing::new("later", &[]));
    mapper.abort_requested();

    let out = feed(
        &mut mapper,
        &[ours("session.execution.interrupted", json!({ "reason": "user" }))],
    );

    assert_eq!(events(out).last(), Some(&AgentEvent::Settled { aborted: Some(true) }));
    assert!(mapper.has_follow_ups());
    assert_eq!(mapper.remove_follow_up("later").map(|update| update.len()), Some(1));
    assert!(!mapper.has_follow_ups());
}

#[test]
fn a_late_failed_prompt_reports_its_error_and_moves_on() {
    let mut mapper = mapper();
    mapper.prompt_sent();

    let out = mapper.prompt_failed_late("HTTP 500", true);

    assert_eq!(shape(&out), ["taken", "promptResult", "settled"]);
    assert!(!mapper.is_working());
}

#[test]
fn steering_the_run_never_took_leaves_the_queue_when_the_turn_ends() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[ours("session.execution.started", json!({}))]);

    mapper.steer_sent(Outgoing::new("too late", &[]));
    assert_eq!(
        mapper.steer_failed("too late"),
        [AgentEvent::QueueUpdate {
            steering: Vec::new(),
            follow_up: Vec::new(),
            live_steered: 0
        }]
    );

    mapper.steer_sent(Outgoing::new("stranded", &[]));
    let out = feed(&mut mapper, &[ours("session.execution.succeeded", json!({}))]);
    assert_eq!(shape(&out), ["queueUpdate", "agentEnd", "promptResult", "settled"]);
}
