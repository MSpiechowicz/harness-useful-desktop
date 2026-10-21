//! The OpenCode 2 mapper's quiet runs (`!command`s and compactions the app starts, not shown as
//! turns) and compactions OpenCode starts on its own during a turn. Against the recorded runs
//! (`fixtures/opencode2/recorded`) and crafted events.

mod opencode2_mapper_support;
mod opencode2_recorded;

use harness_business::model::{AgentEvent, MessageRole};
use harness_business::opencode_server::wire::Outgoing;
use harness_business::opencode_server_v2::events::Event;
use harness_business::opencode_server_v2::mapper::{ContextNote, Mapped, QuietKind, ShellResult};
use opencode2_mapper_support::{ended, events, feed, mapper, ours, replay, shape, text_of};
use pretty_assertions::assert_eq;
use serde_json::json;

fn shell_started() -> Event {
    ours(
        "session.shell.started",
        json!({ "shell": { "id": "sh_1", "status": "running", "command": "echo hi", "cwd": "/work/proj" } }),
    )
}

fn shell_ended() -> Event {
    ours(
        "session.shell.ended",
        json!({ "shell": { "id": "sh_1", "status": "exited", "command": "echo hi", "cwd": "/work/proj", "exit": 0 },
            "output": { "output": "hi\n", "truncated": false } }),
    )
}

fn shell_output() -> Event {
    ours(
        "session.inbox.enqueued",
        json!({ "inboxID": "msg_s", "item": { "type": "synthetic",
            "payload": { "text": "The following shell command was executed by the user" }, "delivery": "steer" } }),
    )
}

fn hi() -> Mapped {
    Mapped::ShellDone(ShellResult {
        command: "echo hi".to_owned(),
        exit: Some(0),
        output: "hi\n".to_owned(),
        truncated: false,
    })
}

#[test]
fn the_recorded_command_runs_quietly_and_reports_its_output() {
    let replay = replay("shell_command.jsonl");

    let done = replay.out.iter().position(|item| matches!(item, Mapped::ShellDone(_)));
    let start = shape(&replay.out).iter().position(|kind| kind == "agentStart");
    assert!(done < start, "the command ends before the next prompt's turn");

    let Some(Mapped::ShellDone(result)) = done.map(|index| &replay.out[index]) else {
        panic!("the command's output");
    };
    assert_eq!(
        result,
        &ShellResult {
            command: "echo hi && ls".to_owned(),
            exit: Some(0),
            output: "hi\ndot.png\nnotes.txt\n".to_owned(),
            truncated: false
        }
    );

    // The command's output reaches the model with the next prompt, not the chat.
    let users = ended(&replay.events(), MessageRole::User);
    assert_eq!(users.len(), 1);
    assert!(text_of(&users[0]).starts_with("You are a code reviewer."));
    assert_eq!(replay.count("agentStart"), 1);
}

#[test]
fn a_command_ends_once_answered_and_ended_in_either_order() {
    let mut mapper = mapper();
    mapper.quiet_begin(QuietKind::Shell);

    assert!(feed(&mut mapper, &[shell_started()]).is_empty());
    assert!(mapper.is_working(), "prompts wait while the command runs");
    assert!(!mapper.can_steer());

    assert!(mapper.quiet_answered(true, None).is_empty());
    assert_eq!(mapper.quiet_kind(), Some(QuietKind::Shell));

    assert_eq!(feed(&mut mapper, &[shell_ended(), shell_output()]), [hi()]);
    assert_eq!(mapper.quiet_kind(), None);
    assert!(!mapper.is_working());

    mapper.quiet_begin(QuietKind::Shell);
    assert!(feed(&mut mapper, &[shell_started(), shell_ended()]).is_empty());
    assert_eq!(mapper.quiet_answered(true, None), [hi()]);
    assert!(
        feed(&mut mapper, &[shell_output()]).is_empty(),
        "the output item is not a prompt"
    );
}

#[test]
fn a_refused_or_lost_command_ends_its_run() {
    let mut mapper = mapper();

    mapper.quiet_begin(QuietKind::Shell);
    assert!(mapper.quiet_answered(false, Some("HTTP 409".to_owned())).is_empty());
    assert!(!mapper.is_working());

    let generation = mapper.quiet_begin(QuietKind::Shell);
    feed(&mut mapper, &[shell_started()]);
    mapper.quiet_answered(true, None);

    assert!(mapper.quiet_expire(generation + 1).is_empty());
    assert!(mapper.is_working(), "another run's timer leaves this one alone");

    mapper.quiet_expire(generation);
    assert!(!mapper.is_working());
}

#[test]
fn a_follow_up_held_during_a_quiet_run_is_sent_after_it() {
    let mut mapper = mapper();
    mapper.quiet_begin(QuietKind::Shell);
    mapper.queue_follow_up(Outgoing::new("then this", &[]));

    feed(&mut mapper, &[shell_started(), shell_ended()]);
    let out = mapper.quiet_answered(true, None);

    assert_eq!(shape(&out), ["shell", "queueUpdate", "send:then this"]);
    assert!(mapper.is_working());
}

#[test]
fn the_recorded_compaction_is_not_a_turn() {
    let replay = replay("ops.jsonl");
    // The session's running totals still come through.
    let shapes: Vec<String> = shape(&replay.out).into_iter().filter(|kind| kind != "usage").collect();

    let start = shapes
        .iter()
        .position(|kind| kind == "compaction")
        .expect("compaction start");
    assert_eq!(
        &shapes[start..],
        ["compaction", "context", "compaction"],
        "nothing of the compaction's run shows besides its start and end"
    );

    let compactions: Vec<AgentEvent> = replay
        .events()
        .into_iter()
        .filter(|event| matches!(event, AgentEvent::Compaction { .. }))
        .collect();
    assert_eq!(
        compactions,
        [
            AgentEvent::Compaction {
                phase: "start".to_owned(),
                reason: Some("manual".to_owned()),
                error: None
            },
            AgentEvent::Compaction {
                phase: "end".to_owned(),
                reason: Some("manual".to_owned()),
                error: None
            }
        ]
    );
    assert!(replay.out.contains(&Mapped::Context(ContextNote::Compacted)));
    assert!(!replay.mapper.is_working());
}

#[test]
fn a_failed_or_interrupted_compaction_ends_with_its_error() {
    let mut mapper = mapper();

    mapper.quiet_begin(QuietKind::Compaction);
    mapper.quiet_answered(true, None);
    let out = feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours("session.compaction.started", json!({ "reason": "manual" })),
            ours(
                "session.compaction.failed",
                json!({ "reason": "manual", "error": { "type": "provider", "message": "Context too large\nstack" } }),
            ),
            ours("session.execution.succeeded", json!({})),
        ],
    );
    assert_eq!(
        events(out),
        [AgentEvent::Compaction {
            phase: "end".to_owned(),
            reason: Some("manual".to_owned()),
            error: Some("Context too large".to_owned())
        }]
    );

    mapper.quiet_begin(QuietKind::Compaction);
    mapper.quiet_answered(true, None);
    let out = feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours("session.execution.interrupted", json!({ "reason": "user" })),
        ],
    );
    assert_eq!(
        events(out),
        [AgentEvent::Compaction {
            phase: "end".to_owned(),
            reason: Some("manual".to_owned()),
            error: Some("The compaction was interrupted".to_owned())
        }]
    );
}

#[test]
fn a_compaction_waits_for_its_run_and_expires_only_when_it_never_started() {
    let mut mapper = mapper();

    let generation = mapper.quiet_begin(QuietKind::Compaction);
    mapper.quiet_answered(true, None);
    feed(&mut mapper, &[ours("session.execution.started", json!({}))]);

    assert!(
        mapper.quiet_expire(generation).is_empty(),
        "a long compaction keeps running"
    );
    assert_eq!(mapper.quiet_kind(), Some(QuietKind::Compaction));

    let out = feed(&mut mapper, &[ours("session.execution.succeeded", json!({}))]);
    assert_eq!(shape(&out), ["compaction"]);

    let generation = mapper.quiet_begin(QuietKind::Compaction);
    mapper.quiet_answered(true, None);
    assert_eq!(shape(&mapper.quiet_expire(generation)), ["compaction"]);
    assert!(!mapper.is_working());
}

#[test]
fn a_compaction_opencode_starts_during_a_turn_shows_in_the_turn() {
    let mut mapper = mapper();
    mapper.prompt_sent();

    let out = feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours("session.compaction.started", json!({ "reason": "auto" })),
            ours("session.compaction.delta", json!({ "text": "## Goal" })),
            ours(
                "session.compaction.ended",
                json!({ "reason": "auto", "text": "## Goal" }),
            ),
            ours("session.execution.succeeded", json!({})),
        ],
    );

    assert_eq!(
        shape(&out),
        [
            "taken",
            "agentStart",
            "compaction",
            "compaction",
            "context",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );
    assert!(events(out).contains(&AgentEvent::Compaction {
        phase: "start".to_owned(),
        reason: Some("auto".to_owned()),
        error: None
    }));
}
