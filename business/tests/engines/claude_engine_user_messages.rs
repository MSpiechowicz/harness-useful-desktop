//! The user's own messages in the Claude Code chat engine: the CLI does not send them back, so the
//! chat shows them itself, an idle prompt at once and a queued one when the CLI takes it, never a
//! withdrawn one and never a `/compact`.

use crate::claude_engine_support::{Chat, Setup, image_prompt, marker, prompt, start, stop_cleanly};

use harness_business::model::{AgentAction, AgentEvent, ChatBlock, ChatMessage, MessageRole, StreamingBehavior};
use pretty_assertions::assert_eq;

const BASH: &str = "Run the shell command `sleep 12 && echo done` with Bash, then say finished.";
const STEER: &str = "Also end your final answer with the word BANANA.";
const FIRST: &str = "Follow-up one: what is 2+2?";
const SECOND: &str = "Follow-up two: this one should be removed.";
const HOOKED: &str = "<harness-hook-context>\n<hook name=\"Style\">Be brief.</hook>\n</harness-hook-context>\n\nReply with exactly the word: pong";

/// The user messages shown so far.
fn shown(events: &[AgentEvent]) -> Vec<ChatMessage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::User => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn text_of(message: &ChatMessage) -> &str {
    match message.blocks.first() {
        Some(ChatBlock::Text { text }) => text,
        other => panic!("not a text block: {other:?}"),
    }
}

fn count_of(events: &[AgentEvent], text: &str) -> usize {
    shown(events).iter().filter(|message| text_of(message) == text).count()
}

/// Starts the queue scenario and sends the Bash prompt, then waits until the tool runs.
async fn running_tool(name: &str) -> Chat {
    let chat = start(name).await;
    chat.send(prompt(BASH)).await;
    chat.wait("tool", |event| matches!(event, AgentEvent::ToolStart { .. }))
        .await;

    chat
}

#[tokio::test]
async fn an_idle_prompt_is_shown_with_its_exact_text_before_the_reply() {
    let chat = start("claude_user_message.jsonl").await;
    chat.send(prompt(HOOKED)).await;

    let events = chat.settled(1).await;
    let starts = events
        .iter()
        .position(|event| {
            matches!(
                event,
                AgentEvent::MessageStart {
                    role: MessageRole::User,
                    ..
                }
            )
        })
        .expect("a user messageStart");
    let ends = events
        .iter()
        .position(|event| matches!(event, AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::User))
        .expect("a user messageEnd");
    let reply = events
        .iter()
        .position(|event| matches!(event, AgentEvent::TextDelta { .. }))
        .expect("a streamed reply");
    assert!(
        starts < ends && ends < reply,
        "the user's message comes first: {events:#?}"
    );

    let messages = shown(&events);
    assert_eq!(messages.len(), 1);
    assert_eq!(text_of(&messages[0]), HOOKED);
    assert_eq!(messages[0].blocks.len(), 1);
    assert!(messages[0].timestamp.is_some_and(|time| time > 0));

    let id = messages[0].id.clone().expect("the sent uuid");
    uuid::Uuid::parse_str(&id).expect("a uuid");
    assert!(matches!(
        &events[starts],
        AgentEvent::MessageStart { message_id, .. } if message_id == &id
    ));
    assert!(matches!(
        &events[ends],
        AgentEvent::MessageEnd { message_id, .. } if message_id == &id
    ));

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_prompt_with_an_image_is_shown_with_its_text_and_image_blocks() {
    let chat = start("claude_image.jsonl").await;

    chat.send(image_prompt()).await;

    let events = marker(&chat).await;
    let messages = shown(&events);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].blocks,
        [
            ChatBlock::Text {
                text: "What is this?".to_owned()
            },
            ChatBlock::Image {
                mime_type: "image/png".to_owned()
            },
        ]
    );

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_steer_is_shown_once_when_the_running_turn_reads_it() {
    let chat = running_tool("claude_steer.jsonl").await;

    chat.send(AgentAction::Steer {
        message: STEER.to_owned(),
        images: Vec::new(),
    })
    .await;

    let events = chat.settled(1).await;
    assert_eq!(count_of(&events, STEER), 1);
    assert_eq!(count_of(&events, BASH), 1);

    let steer = events
        .iter()
        .position(|event| matches!(event, AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::User && text_of(message) == STEER))
        .expect("the steer");
    let queued = events
        .iter()
        .position(|event| matches!(event, AgentEvent::QueueUpdate { steering, .. } if steering == &[STEER.to_owned()]))
        .expect("the steer waited in the queue");
    assert!(queued < steer, "it is shown once the CLI took it");

    chat.stop().await;
}

#[tokio::test]
async fn a_queued_follow_up_is_shown_once_when_taken_and_a_withdrawn_one_never() {
    let chat = running_tool("claude_queue.jsonl").await;

    chat.send(AgentAction::FollowUp {
        message: FIRST.to_owned(),
        images: Vec::new(),
    })
    .await;
    chat.wait(
        "queued",
        |event| matches!(event, AgentEvent::QueueUpdate { follow_up, .. } if follow_up == &[FIRST.to_owned()]),
    )
    .await;
    assert_eq!(count_of(&chat.events(), FIRST), 0, "not shown while it waits");

    chat.send(AgentAction::FollowUp {
        message: SECOND.to_owned(),
        images: Vec::new(),
    })
    .await;
    chat.send(AgentAction::RemoveQueued {
        message: SECOND.to_owned(),
        queue: StreamingBehavior::FollowUp,
    })
    .await;

    let events = chat.settled(1).await;
    assert_eq!(count_of(&events, FIRST), 1);
    assert_eq!(count_of(&events, SECOND), 0);
    assert_eq!(shown(&events).len(), 2, "the Bash prompt and the first follow-up");

    let starts = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                AgentEvent::MessageStart {
                    role: MessageRole::User,
                    ..
                }
            )
        })
        .count();
    assert_eq!(starts, 2);

    chat.stop().await;
}

#[tokio::test]
async fn compaction_shows_no_user_message() {
    let chat = Setup::new(&["claude_compact.jsonl"]).start().await;

    chat.send(AgentAction::Compact {
        instructions: Some(" keep only the words pong and ping ".to_owned()),
    })
    .await;
    chat.wait(
        "compaction ends",
        |event| matches!(event, AgentEvent::Compaction { phase, .. } if phase == "end"),
    )
    .await;

    assert_eq!(shown(&chat.events()), []);

    stop_cleanly(&chat).await;
}
