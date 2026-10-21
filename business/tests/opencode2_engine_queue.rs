//! Steering, follow-ups and abort of the OpenCode 2 chat engine against the scripted OpenCode 2
//! server, with the queue the OpenCode 1 chat shows.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use std::time::Duration;

use harness_business::model::{AgentAction, AgentEvent, AgentReply, PromptStatus, StreamingBehavior};
use opencode2_engine_helpers::{prompt, queue_updates, results, streaming, user_texts};
use opencode2_engine_support::{count, kind, start, until_marker};
use pretty_assertions::assert_eq;

fn steer(message: &str) -> AgentAction {
    AgentAction::Steer {
        message: message.to_owned(),
        images: Vec::new(),
    }
}

fn follow_up(message: &str) -> AgentAction {
    AgentAction::FollowUp {
        message: message.to_owned(),
        images: Vec::new(),
    }
}

fn strings(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

#[tokio::test]
async fn steering_joins_the_running_turn_under_its_id_and_stays_listed_until_taken() {
    let chat = start("opencode2_steer.jsonl").await;
    prompt(&chat, "Say hello").await;
    streaming(&chat).await;

    let reply = chat.session().send(steer("steered message")).await.expect("steered");
    assert_eq!(reply, AgentReply::Ack);

    let events = until_marker(&chat).await;

    assert_eq!(
        queue_updates(&events),
        [(strings(&["steered message"]), vec![], 1), (vec![], vec![], 0)],
        "steering cannot be withdrawn and leaves the list when the run takes it"
    );
    assert_eq!(user_texts(&events), ["Say hello", "steered message"]);
    assert_eq!(
        results(&events),
        [PromptStatus::Completed],
        "steering is not a prompt of its own"
    );
    assert_eq!((count(&events, "agentStart"), count(&events, "settled")), (1, 1));

    let sent = chat.requests_to("POST /api/session/*/prompt").pop().expect("the steer");
    let id = sent.body["id"].as_str().expect("the steer is named");
    assert!(id.starts_with("msg_"), "{id}");

    let withdraw = AgentAction::RemoveQueued {
        message: "steered message".to_owned(),
        queue: StreamingBehavior::Steer,
    };
    assert!(chat.session().send(withdraw).await.is_err());

    chat.session().stop().await;
}

#[tokio::test]
async fn held_follow_ups_go_out_after_the_turn_and_the_chat_settles_once() {
    let chat = start("opencode2_follow_up.jsonl").await;
    prompt(&chat, "Say hello").await;
    streaming(&chat).await;

    let session = chat.session();
    assert_eq!(session.send(follow_up("next")).await.expect("held"), AgentReply::Ack);
    assert_eq!(session.send(follow_up("drop me")).await.expect("held"), AgentReply::Ack);

    let withdraw = |message: &str| AgentAction::RemoveQueued {
        message: message.to_owned(),
        queue: StreamingBehavior::FollowUp,
    };
    let removed = session.send(withdraw("drop me")).await.expect("withdrawn");
    assert_eq!(removed, AgentReply::QueueRemoval { removed: true });
    let missing = session.send(withdraw("never held")).await.expect("answered");
    assert_eq!(missing, AgentReply::QueueRemoval { removed: false });

    // The scenario's sync point: the run ends once the history was read.
    let history = AgentAction::GetHistory {
        cursor: None,
        limit: 50,
    };
    session.send(history).await.expect("history");

    let events = until_marker(&chat).await;

    assert_eq!(
        queue_updates(&events),
        [
            (vec![], strings(&["next"]), 0),
            (vec![], strings(&["next", "drop me"]), 0),
            (vec![], strings(&["next"]), 0),
            (vec![], vec![], 0),
        ]
    );
    assert_eq!(user_texts(&events), ["Say hello", "next"]);
    assert_eq!(results(&events), [PromptStatus::Completed, PromptStatus::Completed]);
    assert_eq!((count(&events, "agentStart"), count(&events, "settled")), (2, 1));

    let starts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| **event == AgentEvent::AgentStart)
        .map(|(index, _)| index)
        .collect();
    let settled = events
        .iter()
        .position(|event| kind(event) == "settled")
        .expect("settled");
    assert!(
        starts[1] < settled,
        "the chat settles after the follow-up's turn: {events:#?}"
    );

    chat.session().stop().await;
}

#[tokio::test]
async fn an_abort_settles_aborted_and_keeps_the_held_follow_up() {
    let chat = start("opencode2_abort_request.jsonl").await;
    prompt(&chat, "Say hello").await;
    streaming(&chat).await;

    let session = chat.session();
    session.send(follow_up("later")).await.expect("held");
    assert_eq!(
        session.send(AgentAction::Abort).await.expect("aborted"),
        AgentReply::Ack
    );

    let events = until_marker(&chat).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(chat.events().len(), events.len(), "nothing is sent after the abort");
    assert_eq!(chat.requests_to("POST /api/session/*/prompt").len(), 1);

    assert_eq!(results(&events), [PromptStatus::Aborted]);
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(true) }));
    assert_eq!(queue_updates(&events).last(), Some(&(vec![], strings(&["later"]), 0)));
    assert_eq!(count(&events, "agentStart"), 1);
    assert!(session.is_running());

    session.stop().await;
}

#[tokio::test]
async fn an_abort_before_the_run_started_is_sent_again_when_it_starts() {
    let chat = start("opencode2_abort_early.jsonl").await;
    prompt(&chat, "Say hello").await;

    let reply = chat.session().send(AgentAction::Abort).await.expect("aborted");
    assert_eq!(reply, AgentReply::Ack);

    let events = until_marker(&chat).await;
    assert_eq!(results(&events), [PromptStatus::Aborted]);
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(true) }));
    assert_eq!(chat.requests_to("POST /api/session/*/interrupt").len(), 2);
    assert!(chat.session().is_running(), "both interrupts matched the scenario");

    chat.session().stop().await;
}
