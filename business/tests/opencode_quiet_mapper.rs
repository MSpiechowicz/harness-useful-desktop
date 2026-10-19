//! The OpenCode mapper's quiet runs (`!command`s and compactions run like turns but are not shown
//! as turns) and the messages a session already had when the chat moved to it (`Mapper::seen`).

mod opencode_mapper_support;

use harness_business::model::AgentEvent;
use harness_business::opencode_server::Mapped;
use harness_business::opencode_server::mapper::QuietKind;
use harness_business::opencode_server::wire::Outgoing;
use opencode_mapper_support::{SESSION, assistant, feed, mapper, part, shape, status};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The events OpenCode sends for `POST /session/{id}/shell` (as recorded), without the `idle`.
fn shell_run() -> Vec<Value> {
    let user = json!({ "type": "message.updated", "properties": { "sessionID": SESSION, "info": {
        "id": "msg_155aa02ed001geWU25KTlQNhQF", "role": "user", "sessionID": SESSION, "time": { "created": 1 } } } });
    let synthetic = part(
        SESSION,
        json!({ "id": "prt_s1", "messageID": "msg_155aa02ed001geWU25KTlQNhQF", "type": "text",
            "text": "The following tool was executed by the user", "synthetic": true }),
    );
    let tool = part(
        SESSION,
        json!({ "id": "prt_s2", "messageID": "msg_155aa02ef001Sr49UnQlIJIlpb", "type": "tool", "tool": "bash",
            "callID": "call_s", "state": { "status": "completed", "input": { "command": "echo from-shell" },
            "output": "from-shell\n", "title": "" } }),
    );

    vec![
        status("busy"),
        user,
        synthetic,
        assistant("msg_155aa02ef001Sr49UnQlIJIlpb", false),
        tool,
        assistant("msg_155aa02ef001Sr49UnQlIJIlpb", true),
    ]
}

#[test]
fn a_shell_run_shows_nothing_and_ends_with_its_answer_and_idle() {
    let mut mapper = mapper();
    mapper.quiet_begin(QuietKind::Shell);

    assert!(shape(&feed(&mut mapper, &shell_run())).is_empty());
    assert!(mapper.is_working(), "prompts wait while the command runs");
    assert!(!mapper.can_steer());

    assert!(shape(&feed(&mut mapper, &[status("idle")])).is_empty());
    assert_eq!(
        mapper.quiet_kind(),
        Some(QuietKind::Shell),
        "the call has not answered yet"
    );

    assert!(mapper.quiet_answered(true, None).is_empty());
    assert_eq!(mapper.quiet_kind(), None);
    assert!(!mapper.is_working());
}

#[test]
fn an_answer_before_idle_waits_for_idle_or_expires() {
    let mut mapper = mapper();
    mapper.quiet_begin(QuietKind::Shell);
    feed(&mut mapper, &shell_run());

    assert!(mapper.quiet_answered(true, None).is_empty());
    assert!(mapper.is_working());

    feed(&mut mapper, &[status("idle")]);
    assert!(!mapper.is_working());

    let generation = mapper.quiet_begin(QuietKind::Shell);
    mapper.quiet_answered(true, None);
    assert!(mapper.quiet_expire(generation + 1).is_empty());
    assert!(mapper.is_working(), "another run's timer leaves this one alone");

    mapper.quiet_expire(generation);
    assert!(!mapper.is_working());
}

#[test]
fn a_refused_run_ends_at_once() {
    let mut mapper = mapper();
    mapper.quiet_begin(QuietKind::Shell);

    mapper.quiet_answered(false, None);

    assert!(!mapper.is_working());
}

#[test]
fn a_compaction_ends_with_its_first_error_then_sends_the_held_follow_up() {
    let mut mapper = mapper();
    mapper.quiet_begin(QuietKind::Compaction);
    mapper.queue_follow_up(Outgoing::new("after", &[]));

    let error = json!({ "type": "session.error", "properties": { "sessionID": SESSION,
        "error": { "name": "UnknownError", "data": { "message": "context too small\n  at stack" } } } });
    let hidden = feed(&mut mapper, &[status("busy"), error, status("idle")]);
    assert!(hidden.is_empty());

    let ended = mapper.quiet_answered(true, None);

    assert_eq!(shape(&ended), ["compaction", "queueUpdate", "send:after"]);
    assert_eq!(
        ended[0],
        Mapped::Event(AgentEvent::Compaction {
            phase: "end".to_owned(),
            reason: Some("manual".to_owned()),
            error: Some("context too small".to_owned()),
        })
    );
}

#[test]
fn messages_the_session_already_had_are_never_shown() {
    let mut mapper = mapper();
    let records = [json!({
        "info": { "id": "msg_155aa02ef001Sr49UnQlIJIlpb", "role": "assistant" },
        "parts": [{ "id": "prt_s2", "type": "tool", "tool": "bash" }]
    })];
    mapper.seen(&records);
    mapper.prompt_sent();

    let late = feed(&mut mapper, &shell_run());
    let shown = shape(&late);

    assert!(!shown.iter().any(|kind| kind.starts_with("tool")), "{shown:?}");
    assert_eq!(
        shown.iter().filter(|kind| *kind == "messageEnd").count(),
        0,
        "the copied reply is not shown again: {shown:?}"
    );
}
