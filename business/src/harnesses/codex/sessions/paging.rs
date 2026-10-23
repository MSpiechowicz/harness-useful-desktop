//! Paging of the Sessions helper (`super`): every page of a paged request within the run's read
//! budget, and the cursors of a transcript's pages, which hold their offset and a fingerprint of
//! the messages before it (their ids and roles, so a turn that is still running and grows its
//! messages keeps the cursors valid, while a rollback or another prompt does not).

use std::hash::{Hash, Hasher};

use serde::de::DeserializeOwned;

use super::app_server::{AppServer, CallError};
use crate::codex_rpc::wire::outbound::Request;
use crate::codex_rpc::wire::results::Page;
use crate::error::{BusinessError, Result};
use crate::model::ChatMessage;

/// The cursor of the page starting at `offset` of `messages`: `<offset>.<fingerprint>`.
pub(super) fn page_cursor(messages: &[ChatMessage], offset: usize) -> String {
    format!("{offset}.{:016x}", fingerprint(&messages[..offset]))
}

/// The offset of `cursor` in `messages`; `Changed` when they no longer start with the messages
/// the cursor was made after, `invalidInput` for a cursor this module did not make.
pub(super) fn page_offset(cursor: &str, messages: &[ChatMessage]) -> Result<String> {
    let unknown = || BusinessError::InvalidInput("unknown transcript cursor".to_owned());
    let (offset, print) = cursor.split_once('.').ok_or_else(unknown)?;
    let start = offset
        .parse::<usize>()
        .ok()
        .filter(|start| *start <= messages.len())
        .ok_or_else(changed)?;

    if format!("{:016x}", fingerprint(&messages[..start])) != print {
        return Err(changed());
    }

    Ok(offset.to_owned())
}

fn changed() -> BusinessError {
    BusinessError::Changed("the session changed while it was read: open it again".to_owned())
}

/// A hash of `messages` (their ids and roles, never their contents).
fn fingerprint(messages: &[ChatMessage]) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();

    for message in messages {
        message.id.hash(&mut hasher);
        std::mem::discriminant(&message.role).hash(&mut hasher);
    }

    hasher.finish()
}

/// The entries of a paged request, and whether more were left unread.
pub(super) struct Pages<T> {
    pub entries: Vec<T>,
    /// The paging stopped at the most entries or at the read budget.
    pub cut: bool,
}

/// Every page of a paged request (`request` for a cursor), up to `max` entries, until `budget`
/// bytes were read.
pub(super) fn pages<T: DeserializeOwned>(
    server: &mut AppServer,
    max: usize,
    budget: usize,
    request: impl Fn(Option<String>) -> Request,
) -> std::result::Result<Pages<T>, CallError> {
    let mut entries = Vec::new();
    let mut cursor = None;

    let mut cut = loop {
        let page: Page<T> = server.call(&request(cursor))?;
        let empty = page.data.is_empty();
        entries.extend(page.data);

        cursor = page.next_cursor;

        if cursor.is_none() || empty {
            break false;
        }

        if entries.len() >= max {
            break true;
        }

        if server.read_bytes() >= budget {
            tracing::warn!(budget, "codex sessions: paging stopped at the read budget");
            break true;
        }
    };

    if entries.len() > max {
        entries.truncate(max);
        cut = true;
    }

    Ok(Pages { entries, cut })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChatBlock, MessageRole};

    fn message(id: &str, role: MessageRole, text: &str) -> ChatMessage {
        ChatMessage {
            id: Some(id.to_owned()),
            role,
            blocks: vec![ChatBlock::Text { text: text.to_owned() }],
            model: None,
            provider: None,
            stop_reason: None,
            error_message: None,
            usage: None,
            cost_usd: None,
            timestamp: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
        }
    }

    #[test]
    fn a_cursor_survives_a_turn_that_grows_and_not_a_rollback() {
        let read = [
            message("t1", MessageRole::User, "Run the tests"),
            message("t1", MessageRole::Assistant, "Running"),
            message("t2", MessageRole::User, "Now the linter"),
        ];
        let cursor = page_cursor(&read, 2);

        // The running turn wrote more: its reply has other blocks now, and a message follows.
        let mut grown = read.to_vec();
        grown[1].blocks.push(ChatBlock::Text {
            text: "Done".to_owned(),
        });
        grown.push(message("t2", MessageRole::Assistant, "Linting"));
        assert_eq!(page_offset(&cursor, &grown).unwrap(), "2");

        // A rollback and another prompt: the turn is a new one.
        let mut rolled_back = read.to_vec();
        rolled_back[1].id = Some("t9".to_owned());
        let refused = page_offset(&cursor, &rolled_back).unwrap_err();
        assert!(matches!(refused, BusinessError::Changed(_)), "{refused:?}");

        // So is a message whose role changed, and a transcript shorter than the cursor.
        let mut other_role = read.to_vec();
        other_role[1].role = MessageRole::ToolResult;
        assert!(page_offset(&cursor, &other_role).is_err());
        assert!(page_offset(&cursor, &read[..1]).is_err());
    }
}
