//! Paging of the Sessions helper (`super`): every page of a paged request within the run's read
//! budget, and the cursors of a transcript's pages, which hold their offset and a fingerprint of
//! the messages before it.

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

/// A hash of `messages` (their ids, roles and blocks).
fn fingerprint(messages: &[ChatMessage]) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();

    for message in messages {
        message.id.hash(&mut hasher);
        serde_json::to_string(&(message.role, &message.blocks))
            .unwrap_or_default()
            .hash(&mut hasher);
    }

    hasher.finish()
}

/// Every page of a paged request (`request` for a cursor), up to `max` entries, until `budget`
/// bytes were read.
pub(super) fn pages<T: DeserializeOwned>(
    server: &mut AppServer,
    max: usize,
    budget: usize,
    request: impl Fn(Option<String>) -> Request,
) -> std::result::Result<Vec<T>, CallError> {
    let mut entries = Vec::new();
    let mut cursor = None;

    loop {
        let page: Page<T> = server.call(&request(cursor))?;
        let empty = page.data.is_empty();
        entries.extend(page.data);

        cursor = page.next_cursor;

        if cursor.is_none() || empty || entries.len() >= max {
            break;
        }

        if server.read_bytes() >= budget {
            tracing::warn!(budget, "codex sessions: paging stopped at the read budget");
            break;
        }
    }

    entries.truncate(max);

    Ok(entries)
}
