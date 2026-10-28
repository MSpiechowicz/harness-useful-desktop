//! The history of an [`OpenCodeV2Session`] (`getHistory`): every message of the chat's session,
//! read newest page first with `GET /api/session/{id}/message` (`order=desc`, then each page's
//! `cursor.next`), converted like the CLI transcript ([`history`]) and served in the pages every
//! engine serves ([`slice_history`]). Startup reads a resumed session's messages the same way
//! ([`all_messages`]), and its newest reply sizes the context meter ([`last_context`]).

use serde_json::Value;

use super::OpenCodeV2Session;
use crate::error::Result;
use crate::model::AgentReply;
use crate::opencode_server::OpenCodeClient;
use crate::opencode_server_v2::mapper::settled_history;
use crate::opencode_server_v2::messages::SessionMessage;
use crate::opencode_server_v2::routes::MAX_MESSAGE_PAGE;
pub(super) use crate::opencode_server_v2::stats::last_context;
use crate::opencode_server_v2::wire::Envelope;
use crate::opencode_server_v2::{MessageOrder, RouteV2, SessionId};
use crate::pi_family::rpc::shared::slice_history;

/// Server pages read at most.
const MAX_HISTORY_PAGES: usize = 64;

impl OpenCodeV2Session {
    /// The history page ending before `cursor` (see the module docs).
    pub(super) async fn history(&self, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
        let chat = &self.chat;
        let (session, working) = {
            let live = chat.lock();
            (live.session.clone(), live.mapper.is_working())
        };
        let records = all_messages(&chat.client, &session, &chat.directory).await?;
        let messages: Vec<SessionMessage> = records.iter().map(SessionMessage::parse).collect();

        slice_history(settled_history(&messages, working), cursor, limit)
    }
}

/// Every message of `session` (in `directory`), oldest first, as the server sent them.
pub(super) async fn all_messages(client: &OpenCodeClient, session: &SessionId, directory: &str) -> Result<Vec<Value>> {
    let mut records: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;

    for _ in 0..MAX_HISTORY_PAGES {
        let route = RouteV2::Messages {
            session,
            directory,
            limit: MAX_MESSAGE_PAGE,
            cursor: cursor.as_deref(),
            order: cursor.is_none().then_some(MessageOrder::Descending),
        };
        let response = client.send(route, None).await?.error_for_status()?;
        let page: Envelope<Vec<Value>> = response.json()?;

        // Each page is newest first and older than the one before it.
        let next = page.next_cursor().map(str::to_owned);
        let last = page.data.is_empty() || next.is_none() || next == cursor;
        records.extend(page.data);

        if last {
            break;
        }

        cursor = next;
    }

    records.reverse();
    Ok(records)
}
