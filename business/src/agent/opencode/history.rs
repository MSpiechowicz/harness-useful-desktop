//! The history of an [`OpenCodeSession`] (`getHistory`): every message of the chat's session, read
//! newest page first with `GET /session/{id}/message?limit=&before=` (the next page's cursor is
//! `x-next-cursor`), converted like the CLI transcript ([`record_messages`]) and served in the
//! pages every engine serves ([`slice_history`]). Subagent sessions keep their own messages. The
//! statistics, the last reply and the HTML export read the messages the same way ([`all_messages`]).

use serde_json::Value;

use super::{Chat, OpenCodeSession};
use crate::error::{BusinessError, Result};
use crate::model::AgentReply;
use crate::opencode_server::mapper::history_messages;
use crate::opencode_server::{Route, SessionId};
use crate::pi_family::rpc::shared::slice_history;

/// Messages per server page.
const HISTORY_PAGE: u32 = 256;

/// Server pages read at most.
const MAX_HISTORY_PAGES: usize = 64;

impl OpenCodeSession {
    /// The history page ending before `cursor` (see the module docs).
    pub(super) async fn history(&self, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
        let (session, working) = {
            let live = self.chat.lock();
            (live.session.clone(), live.mapper.is_working())
        };
        let records = all_messages(&self.chat, &session).await?;
        let messages = history_messages(&records, working);

        slice_history(messages, cursor, limit)
    }
}

/// Every message record (`{info, parts}`) of `session`, oldest first.
pub(super) async fn all_messages(chat: &Chat, session: &SessionId) -> Result<Vec<Value>> {
    let mut records: Vec<Value> = Vec::new();
    let mut before: Option<String> = None;

    for _ in 0..MAX_HISTORY_PAGES {
        let route = Route::Messages {
            session,
            limit: Some(HISTORY_PAGE),
            before: before.as_deref(),
        };
        let response = chat.client.send(route, None).await?.error_for_status()?;
        let page: Value = response.json()?;

        let Value::Array(page) = page else {
            return Err(BusinessError::Protocol(
                "OpenCode sent messages that are not a list".to_owned(),
            ));
        };

        // Each page is oldest first and older than the one before it.
        records.splice(0..0, page);
        before = response.next_cursor().map(str::to_owned);

        if before.is_none() {
            break;
        }
    }

    Ok(records)
}
