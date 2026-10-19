//! Turns of the OpenCode chat engine's mapper (`Mapper`): the prompt-taken order, held follow-ups,
//! steering until its reply, the stranded-steering check, aborts, retries and late failures.

mod opencode_mapper_support;

use harness_business::model::{AgentEvent, PromptStatus};
use harness_business::opencode_server::wire::Outgoing;
use harness_business::opencode_server::{Mapped, Mapper};
use opencode_mapper_support::{SESSION, feed, mapper, shape, status};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn steering(text: &str, id: &str) -> Outgoing {
    let mut message = Outgoing::new(text, &[]);
    message.id = Some(id.to_owned());
    message
}

fn reply_to(id: &str, parent: &str) -> Value {
    json!({ "type": "message.updated", "properties": { "sessionID": SESSION, "info": { "id": id,
        "parentID": parent, "role": "assistant", "sessionID": SESSION, "time": { "created": 1 } } } })
}

fn queue(mapped: &[Mapped]) -> Vec<(Vec<String>, Vec<String>, u32)> {
    mapped
        .iter()
        .filter_map(|item| match item {
            Mapped::Event(AgentEvent::QueueUpdate {
                steering,
                follow_up,
                live_steered,
            }) => Some((steering.clone(), follow_up.clone(), *live_steered)),
            _ => None,
        })
        .collect()
}

fn results(mapped: &[Mapped]) -> Vec<PromptStatus> {
    mapped
        .iter()
        .filter_map(|item| match item {
            Mapped::Event(AgentEvent::PromptResult { status, .. }) => Some(*status),
            _ => None,
        })
        .collect()
}

fn events_of(events: Vec<AgentEvent>) -> Vec<Mapped> {
    events.into_iter().map(Mapped::Event).collect()
}

#[test]
fn a_prompt_is_taken_before_its_turn_starts_and_answered_when_it_ends() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    assert!(mapper.is_working());

    let mapped = feed(&mut mapper, &[status("busy"), status("busy"), status("idle")]);

    assert_eq!(
        shape(&mapped),
        ["taken", "agentStart", "agentEnd", "promptResult", "settled"]
    );
    assert!(!mapper.is_working());

    mapper.prompt_sent();
    mapper.prompt_failed();
    assert!(!mapper.is_working(), "a prompt that failed to send is taken back");
}

#[test]
fn held_follow_ups_go_one_by_one_and_only_the_last_turn_settles() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[status("busy")]);

    let held = events_of(mapper.queue_follow_up(Outgoing::new("first", &[])));
    let held2 = events_of(mapper.queue_follow_up(Outgoing::new("second", &[])));
    assert_eq!(queue(&held), [(vec![], vec!["first".to_owned()], 0)]);
    assert_eq!(
        queue(&held2),
        [(vec![], vec!["first".to_owned(), "second".to_owned()], 0)]
    );

    let removed = mapper.remove_follow_up("second").expect("held");
    assert_eq!(queue(&events_of(removed)), [(vec![], vec!["first".to_owned()], 0)]);
    assert!(mapper.remove_follow_up("second").is_none());

    let first_turn = feed(&mut mapper, &[status("idle")]);
    assert_eq!(
        shape(&first_turn),
        ["agentEnd", "promptResult", "queueUpdate", "send:first"],
        "the next follow-up goes out instead of settling"
    );
    assert!(mapper.is_working(), "the follow-up is on its way");

    let second_turn = feed(&mut mapper, &[status("busy"), status("idle")]);
    assert_eq!(
        shape(&second_turn),
        ["taken", "agentStart", "agentEnd", "promptResult", "settled"]
    );
}

#[test]
fn steering_is_listed_until_its_reply_appears() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[status("busy")]);

    let sent = events_of(mapper.steer_sent(steering("look at the tests", "msg_s1")));
    assert!(mapper.steer_delivered("msg_s1").is_empty());
    assert_eq!(queue(&sent), [(vec!["look at the tests".to_owned()], vec![], 1)]);

    let replied = feed(&mut mapper, &[reply_to("msg_r1", "msg_s1")]);
    assert_eq!(queue(&replied), [(vec![], vec![], 0)]);

    let ended = feed(&mut mapper, &[status("idle")]);
    assert_eq!(shape(&ended), ["agentEnd", "promptResult", "settled"]);
}

#[test]
fn unanswered_steering_is_checked_once_before_the_turn_ends() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[status("busy")]);
    mapper.steer_sent(steering("late steer", "msg_s1"));
    mapper.steer_delivered("msg_s1");

    let idle = feed(&mut mapper, &[status("idle")]);
    assert_eq!(shape(&idle), ["check:late steer"]);
    assert!(mapper.is_working());
    assert!(
        feed(&mut mapper, &[status("idle")]).is_empty(),
        "a second idle waits for the check"
    );

    assert!(mapper.resend_stranded("msg_s1"), "stranded: sent again");
    assert!(!mapper.resend_stranded("msg_s1"));

    let resent_turn = feed(&mut mapper, &[status("busy"), status("idle")]);
    assert_eq!(
        shape(&resent_turn),
        ["queueUpdate", "agentEnd", "promptResult", "settled"],
        "the turn goes on with the resent steering and is not checked again"
    );
}

#[test]
fn the_check_waits_until_the_steering_reached_opencode() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[status("busy")]);
    mapper.steer_sent(steering("late steer", "msg_s1"));
    mapper.steer_sent(steering("lost steer", "msg_s2"));

    assert!(
        feed(&mut mapper, &[status("idle")]).is_empty(),
        "both still on their way"
    );
    assert!(mapper.is_working());
    assert!(mapper.steer_delivered("msg_s1").is_empty());

    let failed = mapper.steer_failed("msg_s2");
    assert_eq!(shape(&failed), ["queueUpdate", "check:late steer"]);
}

#[test]
fn a_check_that_finds_nothing_ends_the_turn() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[status("busy")]);
    mapper.steer_sent(steering("late steer", "msg_s1"));
    mapper.steer_delivered("msg_s1");

    assert_eq!(shape(&feed(&mut mapper, &[status("idle")])), ["check:late steer"]);
    assert_eq!(
        shape(&mapper.stranded_checked()),
        ["queueUpdate", "agentEnd", "promptResult", "settled"]
    );
    assert!(!mapper.is_working());

    let mut failed = self::mapper();
    failed.prompt_sent();
    feed(&mut failed, &[status("busy")]);
    failed.steer_sent(steering("late steer", "msg_s2"));
    failed.steer_delivered("msg_s2");
    feed(&mut failed, &[status("idle")]);
    assert!(failed.resend_stranded("msg_s2"));
    assert_eq!(
        shape(&failed.resend_failed()),
        ["queueUpdate", "agentEnd", "promptResult", "settled"],
        "a failed resend ends the turn at once"
    );
}

#[test]
fn an_abort_settles_at_once_and_keeps_the_held_follow_ups() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[status("busy")]);
    mapper.steer_sent(steering("steer", "msg_s1"));
    mapper.queue_follow_up(Outgoing::new("later", &[]));
    mapper.abort_requested();

    let ended = feed(&mut mapper, &[status("idle")]);
    assert_eq!(shape(&ended), ["queueUpdate", "agentEnd", "promptResult", "settled"]);
    assert_eq!(results(&ended), [PromptStatus::Aborted]);
    assert_eq!(queue(&ended), [(vec![], vec!["later".to_owned()], 0)]);
    assert!(ended.contains(&Mapped::Event(AgentEvent::Settled { aborted: Some(true) })));

    assert!(mapper.has_follow_ups());
    let (next, _) = mapper.next_follow_up().expect("held follow-up");
    assert_eq!(next.text, "later");
}

#[test]
fn retries_start_and_end_within_the_turn() {
    let mut mapper = mapper();
    let retry = json!({ "type": "session.status", "properties": { "sessionID": SESSION,
        "status": { "type": "retry", "attempt": 2, "message": "Rate limited\nstack", "next": 5 } } });

    let mapped = feed(&mut mapper, &[status("busy"), retry, status("busy"), status("idle")]);

    assert_eq!(shape(&mapped), ["agentStart", "retry", "retry", "agentEnd", "settled"]);
    assert_eq!(
        mapped[1],
        Mapped::Event(AgentEvent::Retry {
            phase: "start".to_owned(),
            attempt: Some(2),
            max_attempts: None,
            message: Some("Rate limited".to_owned()),
        })
    );
    assert!(matches!(&mapped[2], Mapped::Event(AgentEvent::Retry { phase, .. }) if phase == "end"));
}

#[test]
fn a_prompt_that_fails_before_its_turn_fails_alone_and_the_queue_moves_on() {
    let mut mapper = Mapper::new(SESSION, std::path::Path::new("/work"), std::path::Path::new("/work"));
    mapper.queue_follow_up(Outgoing::new("next", &[]));
    mapper.prompt_sent();

    let failed = mapper.prompt_failed_late("connection refused", false);
    assert_eq!(shape(&failed), ["promptResult", "queueUpdate", "send:next"]);
    assert_eq!(results(&failed), [PromptStatus::Error]);

    let answered = mapper.prompt_failed_late("the OpenCode server answered HTTP 400", true);
    assert_eq!(
        shape(&answered),
        ["taken", "promptResult", "settled"],
        "a command OpenCode answered was taken before it failed"
    );

    feed(&mut mapper, &[status("busy")]);
    assert!(
        mapper.prompt_failed_late("timed out", false).is_empty(),
        "once the turn runs, the turn reports"
    );
}
