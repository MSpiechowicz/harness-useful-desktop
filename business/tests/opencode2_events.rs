//! The typed OpenCode 2 event stream (`opencode_server_v2::events`), against the recorded events
//! (`fixtures/opencode2/recorded`) and the event schema of `opencode` 2.1.2.

mod opencode2_recorded;

use harness_business::opencode_server_v2::events::{
    Event, EventError, EventKind, SessionStatus, ShellEnded, ToolContent, parse,
};
use harness_business::opencode_server_v2::wire::Delivery;
use opencode2_recorded::{DIRECTORY, events};
use pretty_assertions::assert_eq;
use serde_json::json;

const SESSION: &str = "ses_ea27cfa99ffe21eKppwXzXASSa";

/// Every recorded event of one recording, parsed.
fn recorded(file: &str) -> Vec<Event> {
    events()
        .into_iter()
        .filter(|(name, _)| name == file)
        .map(|(_, data)| parse(&data).expect("recorded event"))
        .collect()
}

fn event(kind: &str, data: serde_json::Value) -> Result<Event, EventError> {
    parse(&json!({"id": "evt_1", "created": 1, "type": kind, "data": data}).to_string())
}

#[test]
fn every_recorded_event_parses_and_session_events_are_known() {
    let recorded = events();
    assert!(recorded.len() > 500, "{} recorded events", recorded.len());

    for (file, data) in recorded {
        let event = parse(&data).unwrap_or_else(|error| panic!("{file}: {error}: {data}"));
        assert!(event.id.starts_with("evt_"), "{file}: {data}");

        if let EventKind::Unknown(kind) = &event.kind {
            let ours = ["session.", "permission.", "form."]
                .iter()
                .any(|prefix| kind.starts_with(prefix));
            assert!(!ours, "{file}: `{kind}` has no typed variant");
        }
    }
}

#[test]
fn a_turn_streams_its_steps_text_and_usage() {
    let turn = recorded("turn.jsonl");

    assert_eq!(turn[0].kind, EventKind::ServerConnected);
    assert_eq!((turn[0].created, turn[0].location.as_ref()), (None, None));

    let text: Vec<&str> = turn
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::TextEnded(ended) => Some(ended.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, ["hello"]);

    let step = turn
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::StepEnded(ended) => Some(ended),
            _ => None,
        })
        .expect("a step");
    assert_eq!(step.finish, "stop");
    assert_eq!(
        (step.tokens.input, step.tokens.output, step.tokens.cache.read),
        (280, 3, 5120)
    );

    let started = turn
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::StepStarted(started) => Some(started),
            _ => None,
        })
        .expect("a step start");
    assert_eq!(started.agent, "build");
    assert_eq!(
        started.model.as_ref().map(|model| model.id.as_str()),
        Some("big-pickle")
    );
    assert_eq!(started.assistant_message_id, step.assistant_message_id);

    let kinds: Vec<&str> = turn.iter().map(|event| kind_name(&event.kind)).collect();
    let order = [
        "inbox.enqueued",
        "execution.started",
        "inbox.delivered",
        "step.started",
        "step.ended",
        "usage.updated",
        "execution.succeeded",
    ];
    let positions: Vec<usize> = order
        .iter()
        .map(|name| {
            kinds
                .iter()
                .position(|kind| kind == name)
                .unwrap_or_else(|| panic!("{name}"))
        })
        .collect();
    assert!(positions.is_sorted(), "{kinds:?}");

    let renamed = turn.iter().find_map(|event| match &event.kind {
        EventKind::Renamed(renamed) => Some(renamed.title.as_str()),
        _ => None,
    });
    assert_eq!(renamed, Some("hello"));

    let session = turn
        .iter()
        .find(|event| matches!(event.kind, EventKind::ExecutionSucceeded(_)));
    assert!(
        session
            .and_then(|event| event.session_id.as_deref())
            .is_some_and(|id| id.starts_with("ses_"))
    );
}

fn kind_name(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::InboxEnqueued(_) => "inbox.enqueued",
        EventKind::ExecutionStarted(_) => "execution.started",
        EventKind::InboxDelivered(_) => "inbox.delivered",
        EventKind::StepStarted(_) => "step.started",
        EventKind::StepEnded(_) => "step.ended",
        EventKind::UsageUpdated(_) => "usage.updated",
        EventKind::ExecutionSucceeded(_) => "execution.succeeded",
        _ => "other",
    }
}

#[test]
fn tools_report_their_name_input_result_and_failure() {
    let tools = recorded("tools.jsonl");

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ToolInputStarted(started) => Some(started.name.as_str()),
            _ => None,
        })
        .collect();
    assert!(names.contains(&"read") && names.contains(&"glob"), "{names:?}");

    let read = tools
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::ToolCalled(called) if called.input.get("path").is_some() => Some(called),
            _ => None,
        })
        .expect("a read call");
    assert!(read.id.starts_with("call_"));

    let success = tools
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::ToolSuccess(success) if success.id == read.id => Some(success),
            _ => None,
        })
        .expect("the read result");
    assert!(matches!(&success.content[0], ToolContent::Text { text } if !text.is_empty()));

    let rejected = recorded("edit_reject.jsonl")
        .into_iter()
        .find_map(|event| match event.kind {
            EventKind::ToolFailed(failed) => Some(failed),
            _ => None,
        })
        .expect("a refused write");
    assert_eq!(rejected.error.kind, "permission.rejected");
    assert_eq!(rejected.error.message, "Do not change that file.");
}

#[test]
fn permissions_and_forms_carry_what_the_cards_show() {
    let edit = recorded("edit_once.jsonl");

    let asked = edit
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::PermissionAsked(asked) => Some(asked),
            _ => None,
        })
        .expect("an ask");
    assert_eq!(asked.id, "per_15d7c3a33001lsSJ9gGeiFzCh8");
    assert_eq!(
        (asked.action.as_str(), asked.resources.as_slice()),
        ("edit", ["notes.txt".to_owned()].as_slice())
    );
    assert!(
        asked.metadata["files"][0]["patch"]
            .as_str()
            .is_some_and(|patch| patch.starts_with("Index: notes.txt"))
    );

    let replied = edit
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::PermissionReplied(replied) => Some(replied),
            _ => None,
        })
        .expect("a reply");
    assert_eq!(
        (replied.request_id.as_str(), replied.reply.as_str()),
        (asked.id.as_str(), "once")
    );

    let question = recorded("question.jsonl");
    let created = question
        .iter()
        .find(|event| matches!(event.kind, EventKind::FormCreated(_)))
        .expect("a form");
    let EventKind::FormCreated(form) = &created.kind else {
        unreachable!()
    };
    assert_eq!(form.form.id, "frm_15d80582d001AwxHnvC4VOQ9RP");
    assert_eq!(form.form.fields[0].title.as_deref(), Some("Color preference"));
    assert_eq!(created.session_id.as_deref(), Some("ses_ea27fb347ffe9TdQgsIApShsEN"));
    assert_eq!(
        created.location.as_ref().map(|location| location.directory.as_str()),
        Some(DIRECTORY)
    );

    let answered = question
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::FormReplied(replied) => Some(replied),
            _ => None,
        })
        .expect("an answer");
    assert_eq!(answered.answer["q0"], "blue");
}

#[test]
fn steering_queueing_and_interrupting_are_typed() {
    let deliveries: Vec<(String, Option<Delivery>)> = recorded("steer_queue.jsonl")
        .into_iter()
        .filter_map(|event| match event.kind {
            EventKind::InboxEnqueued(enqueued) => Some((enqueued.item.kind, enqueued.item.delivery)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deliveries,
        [
            ("user".to_owned(), Some(Delivery::Steer)),
            ("user".to_owned(), Some(Delivery::Steer)),
            ("user".to_owned(), Some(Delivery::Queue)),
        ]
    );

    let interrupt = recorded("interrupt.jsonl");
    let aborted = interrupt
        .iter()
        .any(|event| matches!(&event.kind, EventKind::ToolFailed(failed) if failed.error.kind == "aborted"));
    assert!(aborted);
    let reason = interrupt.iter().find_map(|event| match &event.kind {
        EventKind::ExecutionInterrupted(interrupted) => Some(interrupted.reason.as_str()),
        _ => None,
    });
    assert_eq!(reason, Some("user"));
}

#[test]
fn shell_commands_compactions_and_forks_are_typed() {
    let busy = recorded("busy.jsonl");
    let ended: Vec<&ShellEnded> = busy
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ShellEnded(ended) => Some(ended),
            _ => None,
        })
        .collect();
    assert_eq!(ended[0].shell.command, "echo busy");
    assert_eq!(
        (ended[0].shell.exit, ended[0].output.output.as_str()),
        (Some(0), "busy\n")
    );
    assert!(
        busy.iter()
            .any(|event| matches!(event.kind, EventKind::ShellStarted(_)))
    );

    let ops = recorded("ops.jsonl");
    let compaction = ops.iter().find_map(|event| match &event.kind {
        EventKind::CompactionEnded(ended) => Some(ended),
        _ => None,
    });
    let compaction = compaction.expect("a compaction");
    assert_eq!(compaction.reason, "manual");
    assert!(compaction.text.starts_with("## Objective"));
    assert!(
        ops.iter()
            .any(|event| matches!(event.kind, EventKind::CompactionStarted(_)))
    );
    assert!(
        ops.iter()
            .any(|event| matches!(event.kind, EventKind::CompactionDelta(_)))
    );

    let boundaries: Vec<&str> = ops
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Forked(forked) => Some(forked.boundary.kind.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(boundaries, ["before", "through"]);
    assert!(
        ops.iter()
            .any(|event| matches!(event.kind, EventKind::SessionDeleted(_)))
    );
    assert!(ops.iter().any(|event| event.kind == EventKind::AgentsUpdated));
    assert!(
        ops.iter()
            .any(|event| event.kind == EventKind::Unknown("project.updated".to_owned()))
    );
}

#[test]
fn schema_events_missing_from_the_recordings_are_typed() {
    let status = |status| event("session.status", json!({"sessionID": SESSION, "status": status}));

    let busy = status(json!({"type": "busy"})).expect("busy");
    assert!(matches!(busy.kind, EventKind::Status(changed) if changed.status == SessionStatus::Busy));

    let retry = status(json!({"type": "retry", "attempt": 2, "message": "rate limited", "next": 1700})).expect("retry");
    let EventKind::Status(changed) = retry.kind else {
        panic!("a status")
    };
    assert_eq!(
        changed.status,
        SessionStatus::Retry {
            attempt: 2,
            message: "rate limited".to_owned(),
            next: 1700
        }
    );
    let later = status(json!({"type": "sleeping"})).expect("an unknown status");
    assert!(matches!(later.kind, EventKind::Status(changed) if changed.status == SessionStatus::Other));

    let idle = event("session.idle", json!({"sessionID": SESSION})).expect("idle");
    assert!(matches!(idle.kind, EventKind::Idle(_)));
    assert_eq!(idle.session_id.as_deref(), Some(SESSION));

    let cancelled = event("form.cancelled", json!({"id": "frm_1", "sessionID": SESSION})).expect("cancelled");
    assert!(matches!(cancelled.kind, EventKind::FormCancelled(settled) if settled.answer.is_empty()));

    let delta = event(
        "session.tool.input.delta",
        json!({"sessionID": SESSION, "assistantMessageID": "msg_1", "id": "call_1", "delta": "{\"pa"}),
    )
    .expect("input delta");
    assert!(matches!(delta.kind, EventKind::ToolInputDelta(delta) if delta.delta == "{\"pa"));

    let agent = event("session.agent.selected", json!({"sessionID": SESSION, "agent": "plan"})).expect("agent");
    assert!(matches!(agent.kind, EventKind::AgentSelected(selected) if selected.agent == "plan"));

    let model = event(
        "session.model.selected",
        json!({"sessionID": SESSION, "model": {"id": "big-pickle", "providerID": "opencode", "variant": "high"}}),
    )
    .expect("model");
    assert!(
        matches!(model.kind, EventKind::ModelSelected(selected) if selected.model.variant.as_deref() == Some("high"))
    );

    let failed = event(
        "session.execution.failed",
        json!({"sessionID": SESSION, "error": {"type": "provider", "message": "quota", "status": 429}}),
    )
    .expect("failed");
    assert!(matches!(failed.kind, EventKind::ExecutionFailed(failed) if failed.error.message == "quota"));
}

#[test]
fn unknown_types_and_fields_are_tolerated_and_broken_events_refused() {
    let future = event("session.something.new", json!({"sessionID": SESSION})).expect("a future event");
    assert_eq!(future.kind, EventKind::Unknown("session.something.new".to_owned()));
    assert_eq!(future.session_id.as_deref(), Some(SESSION));

    let extra = event(
        "session.renamed",
        json!({"sessionID": SESSION, "title": "New", "reason": "auto", "nested": {"x": 1}}),
    )
    .expect("extra fields");
    assert!(matches!(extra.kind, EventKind::Renamed(renamed) if renamed.title == "New"));

    assert_eq!(parse("not json"), Err(EventError::Envelope));
    assert_eq!(parse(r#"{"id": "evt_1", "data": {}}"#), Err(EventError::Envelope));

    let broken = event(
        "session.text.delta",
        json!({"sessionID": SESSION, "secret": "token-123"}),
    );
    assert_eq!(
        broken,
        Err(EventError::Data {
            kind: "session.text.delta".to_owned()
        })
    );
    let message = broken.expect_err("broken").to_string();
    assert!(!message.contains("token-123"), "{message}");
}
