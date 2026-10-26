//! Steering, follow-ups, withdrawing a queued message and abort of the Claude Code chat engine
//! against the scripted `claude` replaying the recorded queue and interrupt traffic.

use crate::claude_engine_support;

use claude_engine_support::{Chat, prompt, start};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, PromptStatus, StreamingBehavior};
use pretty_assertions::assert_eq;

const STEER: &str = "Also end your final answer with the word BANANA.";
const FIRST: &str = "Follow-up one: what is 2+2?";
const SECOND: &str = "Follow-up two: this one should be removed.";

/// Every `queueUpdate` as (steering, follow-ups).
fn queue_updates(events: &[AgentEvent]) -> Vec<(Vec<String>, Vec<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::QueueUpdate {
                steering, follow_up, ..
            } => Some((steering.clone(), follow_up.clone())),
            _ => None,
        })
        .collect()
}

fn texts(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn count(events: &[AgentEvent], predicate: impl Fn(&AgentEvent) -> bool) -> usize {
    events.iter().filter(|event| predicate(event)).count()
}

/// Starts `name` and sends the recorded Bash prompt, then waits until the tool runs.
async fn running_tool(name: &str) -> Chat {
    let chat = start(name).await;
    chat.send(prompt(
        "Run the shell command `sleep 12 && echo done` with Bash, then say finished.",
    ))
    .await;
    chat.wait("tool", |event| matches!(event, AgentEvent::ToolStart { .. }))
        .await;

    chat
}

#[tokio::test]
async fn a_steer_waits_in_the_queue_until_the_running_turn_reads_it() {
    let chat = running_tool("claude_steer.jsonl").await;

    let reply = chat
        .send(AgentAction::Steer {
            message: STEER.to_owned(),
            images: Vec::new(),
        })
        .await;
    assert_eq!(reply, AgentReply::Ack);

    let events = chat.settled(1).await;
    assert_eq!(queue_updates(&events), [(texts(&[STEER]), vec![]), (vec![], vec![])]);
    assert_eq!(
        count(&events, |event| matches!(event, AgentEvent::AgentStart)),
        1,
        "one turn"
    );
    assert!(events.iter().any(|event| {
        matches!(event, AgentEvent::MessageEnd { message, .. } if format!("{message:?}").contains("BANANA"))
    }));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Settled { aborted: Some(false) }))
    );

    chat.stop().await;
}

#[tokio::test]
async fn follow_ups_wait_for_their_own_turn_and_one_can_be_withdrawn() {
    let chat = running_tool("claude_queue.jsonl").await;

    for text in [FIRST, SECOND] {
        let reply = chat
            .send(AgentAction::FollowUp {
                message: text.to_owned(),
                images: Vec::new(),
            })
            .await;
        assert_eq!(reply, AgentReply::Ack);
    }

    let missing = AgentAction::RemoveQueued {
        message: SECOND.to_owned(),
        queue: StreamingBehavior::Steer,
    };
    assert_eq!(
        chat.send(missing).await,
        AgentReply::QueueRemoval { removed: false },
        "it waits as a follow-up, not as steering"
    );

    let removed = AgentAction::RemoveQueued {
        message: SECOND.to_owned(),
        queue: StreamingBehavior::FollowUp,
    };
    assert_eq!(chat.send(removed).await, AgentReply::QueueRemoval { removed: true });

    let events = chat.settled(1).await;
    assert_eq!(
        queue_updates(&events),
        [
            (vec![], texts(&[FIRST])),
            (vec![], texts(&[FIRST, SECOND])),
            (vec![], texts(&[FIRST])),
            (vec![], vec![]),
        ]
    );
    assert_eq!(
        count(&events, |event| matches!(event, AgentEvent::AgentStart)),
        2,
        "the follow-up ran as its own turn"
    );
    assert_eq!(
        count(&events, |event| matches!(event, AgentEvent::Settled { .. })),
        1,
        "settled once, after it"
    );
    assert!(
        events
            .iter()
            .any(|event| { matches!(event, AgentEvent::TextDelta { delta, .. } if delta.contains("2 + 2")) })
    );

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.queued_count, 0);

    chat.stop().await;
}

#[tokio::test]
async fn a_prompt_sent_while_working_is_queued_as_a_follow_up() {
    let chat = running_tool("claude_queue.jsonl").await;

    let reply = chat.send(prompt(FIRST)).await;
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("queued".to_owned())
        }
    );

    chat.wait(
        "queued",
        |event| matches!(event, AgentEvent::QueueUpdate { follow_up, .. } if follow_up == &texts(&[FIRST])),
    )
    .await;

    chat.session().stop().await;
}

#[tokio::test]
async fn abort_interrupts_the_turn_and_the_chat_takes_the_next_prompt() {
    let chat = start("claude_abort.jsonl").await;
    chat.send(prompt("Write a 600 word story about a lighthouse.")).await;
    chat.wait(
        "streaming",
        |event| matches!(event, AgentEvent::TextDelta { delta, .. } if delta == "ch"),
    )
    .await;

    assert_eq!(chat.send(AgentAction::Abort).await, AgentReply::Ack);

    let events = chat.settled(1).await;
    assert!(events.iter().any(|event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Aborted,
                ..
            }
        )
    }));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Settled { aborted: Some(true) }))
    );

    chat.send(prompt("Reply with exactly: after")).await;
    let events = chat.settled(2).await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::TextDelta { delta, .. } if delta == "after"))
    );
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Settled { aborted: Some(false) })
    ));

    let stopped = chat.stop().await;
    assert_eq!(
        stopped.last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        })
    );
}

#[tokio::test]
async fn abort_while_permission_prompts_wait_denies_the_ones_claude_did_not_withdraw() {
    let chat = start("claude_abort_asks.jsonl").await;
    chat.send(prompt("Read, write, then list")).await;
    chat.wait(
        "marker",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;

    assert_eq!(chat.send(AgentAction::Abort).await, AgentReply::Ack);

    let events = chat.settled(1).await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Settled { aborted: Some(true) }))
    );

    let stopped = chat.stop().await;
    assert_eq!(
        stopped.last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        }),
        "the scripted CLI got the denial it expected"
    );
}
