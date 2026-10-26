//! What the OpenCode 2 engine tests read from a chat's events, as the OpenCode 1 engine tests do:
//! prompts with the prompt-taken position, ended messages, queue updates and prompt results.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use harness_business::model::{AgentEvent, AgentReply, ChatBlock, ChatMessage, MessageRole, PromptStatus};

use crate::opencode2_engine_support::{Chat, text_prompt};

/// Sends prompt `message` (it starts a turn); where the prompt-taken call ran (the number of events
/// before it).
pub async fn prompt(chat: &Chat, message: &str) -> Arc<Mutex<Option<usize>>> {
    let taken_at = Arc::new(Mutex::new(None));
    let (sink, record) = (chat.sink.clone(), taken_at.clone());
    chat.session().on_prompt_taken(Box::new(move || {
        *record.lock().unwrap() = Some(sink.events().len());
    }));

    let reply = chat.session().send(text_prompt(message)).await.expect("prompt taken");
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned())
        }
    );

    taken_at
}

/// The messages of `role` that ended.
pub fn message_end(events: &[AgentEvent], role: MessageRole) -> Vec<ChatMessage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == role => Some(message.clone()),
            _ => None,
        })
        .collect()
}

/// The texts of the ended user messages.
pub fn user_texts(events: &[AgentEvent]) -> Vec<String> {
    message_end(events, MessageRole::User)
        .into_iter()
        .filter_map(|message| match message.blocks.first() {
            Some(ChatBlock::Text { text }) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Every `queueUpdate` as `(steering, follow-ups, live steered)`.
pub fn queue_updates(events: &[AgentEvent]) -> Vec<(Vec<String>, Vec<String>, u32)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::QueueUpdate {
                steering,
                follow_up,
                live_steered,
            } => Some((steering.clone(), follow_up.clone(), *live_steered)),
            _ => None,
        })
        .collect()
}

/// The status of every `promptResult`.
pub fn results(events: &[AgentEvent]) -> Vec<PromptStatus> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, .. } => Some(*status),
            _ => None,
        })
        .collect()
}

/// Waits until a reply streams its first delta.
pub async fn streaming(chat: &Chat) {
    chat.wait("delta", |event| matches!(event, AgentEvent::TextDelta { .. }))
        .await;
}

/// The joined text deltas of content index `index`.
pub fn streamed(events: &[AgentEvent], index: u32) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta {
                content_index, delta, ..
            } if *content_index == index => Some(delta.as_str()),
            _ => None,
        })
        .collect()
}
