//! A session's messages as the chat shows them on resume.

use crate::model::ChatMessage;
use crate::opencode_server_v2::messages::SessionMessage;

/// The chat messages of `messages` (a session's messages, oldest first) as the chat shows them on
/// resume: the conversion the CLI transcript uses. A reply is shown as it stands: a call still
/// running in it gets no result (see [`settled_history`] for finished turns).
pub fn history(messages: &[SessionMessage]) -> Vec<ChatMessage> {
    messages.iter().flat_map(SessionMessage::chat_messages).collect()
}

/// Like [`history`], but a call that never finished in a reply whose turn is over gets a failed
/// result (the one the live chat gives it), so it no longer shows as not run.
///
/// A reply is over when it completed or failed (`time.completed`, `error`), a later reply or an
/// idle marker exists, or `open_tail` is false. With `open_tail` (a busy chat, or a session another
/// process may be running) the newest reply stays open until one of those holds. Later user
/// messages (queued prompts) do not close it.
pub fn settled_history(messages: &[SessionMessage], open_tail: bool) -> Vec<ChatMessage> {
    let newest = messages
        .iter()
        .rposition(|message| matches!(message, SessionMessage::Assistant(_)));
    let idle_after = |position: usize| {
        messages[position + 1..]
            .iter()
            .any(|message| matches!(message, SessionMessage::Idle(_)))
    };

    messages
        .iter()
        .enumerate()
        .flat_map(|(position, message)| {
            let settled = match message {
                SessionMessage::Assistant(reply) => {
                    reply.time.completed.is_some()
                        || reply.error.is_some()
                        || !open_tail
                        || Some(position) != newest
                        || idle_after(position)
                }
                _ => false,
            };

            message.chat_messages_settled(settled)
        })
        .collect()
}
