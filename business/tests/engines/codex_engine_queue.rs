//! Steering, the app's follow-up queue and abort of a Codex chat: a steer goes into the running
//! turn (or, when that turn is over, runs as the next), follow-ups wait in the app and one can be
//! withdrawn, and an abort cancels the open approval and interrupts the turn. A turn Codex takes
//! after the chat gave up waiting still counts its message.

use crate::codex_engine_support;

use codex_engine_support::{prompt, replies, start};
use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, PromptStatus, StreamingBehavior};
use pretty_assertions::assert_eq;

fn steer(text: &str) -> AgentAction {
    AgentAction::Steer {
        message: text.to_owned(),
        images: Vec::new(),
    }
}

/// The steering and follow-up texts of every `queueUpdate`, in order.
fn queues(events: &[AgentEvent]) -> Vec<(Vec<String>, Vec<String>)> {
    let mut shown = Vec::new();

    for event in events {
        if let AgentEvent::QueueUpdate {
            steering, follow_up, ..
        } = event
        {
            shown.push((steering.clone(), follow_up.clone()));
        }
    }

    shown
}

fn count(events: &[AgentEvent], wanted: impl Fn(&AgentEvent) -> bool) -> usize {
    events.iter().filter(|event| wanted(event)).count()
}

#[tokio::test]
async fn a_steer_goes_into_the_running_turn() {
    let chat = start("codex_engine_steer.jsonl").await;
    chat.act(prompt("Run `sleep 6 && echo done` and report the result."))
        .await;
    chat.wait("the command", |event| matches!(event, AgentEvent::ToolStart { .. }))
        .await;

    let text = "Also mention the word STEERED in your answer.";
    assert_eq!(chat.act(steer(text)).await, AgentReply::Ack);

    let approval = chat.dialog(0).await;
    chat.pick(&approval.id, "Allow once").await.expect("allowed");

    let events = chat.settled(1).await;
    assert_eq!(
        queues(&events),
        [(vec![text.to_owned()], vec![]), (vec![], vec![])],
        "listed until Codex took it"
    );
    assert_eq!(
        replies(&events).last().map(String::as_str),
        Some("STEERED — result: `done`")
    );
    assert_eq!(count(&events, |event| matches!(event, AgentEvent::AgentStart)), 1);

    chat.finish().await;
}

#[tokio::test]
async fn a_steer_after_the_turn_ended_runs_as_the_next_turn() {
    let chat = start("codex_engine_steer_late.jsonl").await;
    chat.act(prompt("Start.")).await;
    chat.wait("the turn", |event| matches!(event, AgentEvent::TurnStart))
        .await;

    chat.act(steer("Too late for that turn.")).await;

    chat.wait("the second answer", |_| {
        replies(&chat.events()).iter().any(|text| text == "Second answer.")
    })
    .await;
    let events = chat
        .wait("the last settle", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;
    assert_eq!(events, AgentEvent::Settled { aborted: Some(false) });

    chat.finish().await;
}

#[tokio::test]
async fn follow_ups_wait_in_the_app_and_can_be_withdrawn() {
    let chat = start("codex_engine_queue.jsonl").await;
    chat.act(prompt("First")).await;

    let follow_up = AgentAction::FollowUp {
        message: "Second".to_owned(),
        images: Vec::new(),
    };
    assert_eq!(chat.act(follow_up).await, AgentReply::Ack);

    let queued = chat.act(prompt("Third")).await;
    assert_eq!(
        queued,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("queued".to_owned())
        }
    );

    let remove = |text: &str, queue| AgentAction::RemoveQueued {
        message: text.to_owned(),
        queue,
    };
    let removed = chat.act(remove("Third", StreamingBehavior::FollowUp)).await;
    assert_eq!(removed, AgentReply::QueueRemoval { removed: true });
    let missing = chat.act(remove("Nope", StreamingBehavior::FollowUp)).await;
    assert_eq!(missing, AgentReply::QueueRemoval { removed: false });

    // The model list is the scenario's cue to end the first turn.
    chat.act(AgentAction::GetAvailableModels).await;

    chat.wait("the second answer", |_| {
        replies(&chat.events()).iter().any(|text| text == "Second answer.")
    })
    .await;
    let events = chat.settled(1).await;
    assert_eq!(count(&events, |event| matches!(event, AgentEvent::Settled { .. })), 1);
    assert_eq!(
        queues(&events),
        [
            (vec![], vec!["Second".to_owned()]),
            (vec![], vec!["Second".to_owned(), "Third".to_owned()]),
            (vec![], vec!["Second".to_owned()]),
            (vec![], vec![]),
        ]
    );

    chat.finish().await;
}

#[tokio::test]
async fn abort_cancels_the_open_approval_and_interrupts_the_turn() {
    let chat = start("codex_engine_abort.jsonl").await;
    chat.act(prompt("Run `sleep 30 && echo late` and report the result."))
        .await;

    let approval = chat.dialog(0).await;
    assert_eq!(chat.act(AgentAction::Abort).await, AgentReply::Ack);

    let events = chat.settled(1).await;
    assert!(events.contains(&AgentEvent::UiCancel {
        request_id: approval.id.clone()
    }));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::PromptResult {
            status: PromptStatus::Aborted,
            ..
        }
    )));
    assert_eq!(events.last(), Some(&AgentEvent::Settled { aborted: Some(true) }));

    assert_eq!(
        chat.act(AgentAction::Abort).await,
        AgentReply::Ack,
        "nothing left to stop"
    );
    chat.finish().await;
}

#[tokio::test]
async fn a_stop_before_a_turn_codex_refuses_ends_with_it() {
    let chat = start("codex_engine_abort_refused.jsonl").await;

    let (refused, stopped) = tokio::join!(chat.session.send(prompt("Never starts.")), async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        chat.session.send(AgentAction::Abort).await
    });
    assert!(refused.is_err(), "Codex refused the turn");
    assert_eq!(stopped.unwrap(), AgentReply::Ack);

    chat.act(prompt("Ask me about colours.")).await;
    let events = chat
        .wait("the next turn's end", |event| {
            matches!(event, AgentEvent::PromptResult { .. })
        })
        .await;
    assert!(matches!(
        events,
        AgentEvent::PromptResult {
            status: PromptStatus::Completed,
            ..
        }
    ));

    let AgentReply::Stats { stats } = chat.act(AgentAction::GetSessionStats).await else {
        panic!("not stats");
    };
    assert_eq!(stats.user_messages, 1, "the refused message is not counted");

    // The scripted Codex fails on any turn/interrupt.
    chat.finish().await;
}

#[tokio::test]
async fn a_turn_codex_takes_after_the_chat_gave_up_waiting_counts_its_message() {
    let slow = codex_engine_support::Setup {
        prompt_ack_timeout: Some(std::time::Duration::from_millis(200)),
        ..codex_engine_support::Setup::default()
    };
    let chat = codex_engine_support::start_with("codex_engine_prompt_late.jsonl", slow, |_| {}).await;

    let waited = chat.session.send(prompt("Slow to start.")).await;
    assert!(matches!(waited, Err(BusinessError::Timeout(_))), "{waited:?}");

    chat.wait("the late answer", |_| {
        replies(&chat.events()).iter().any(|text| text == "Late answer.")
    })
    .await;
    chat.wait("the settle", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;

    let AgentReply::Stats { stats } = chat.act(AgentAction::GetSessionStats).await else {
        panic!("not stats");
    };
    assert_eq!((stats.user_messages, stats.assistant_messages), (1, 1));

    chat.finish().await;
}
