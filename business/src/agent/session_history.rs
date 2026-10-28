//! Reading an RPC session's whole history.
//!
//! A paged harness (OMP) keeps a page cursor valid only while the session is unchanged, and
//! refuses to page while it works. So a read that meets a refusal starts over rather than
//! stitching pages of two snapshots together:
//!
//! - `stale_cursor` (a message was appended between two pages) or pages that disagree on the total:
//!   read again from the newest page, at most [`MAX_RESTARTS`] times.
//! - `session_busy` (a turn or a compaction runs): read again after a short pause, within
//!   [`BUSY_BACKOFF`].
//!
//! Still refused, the history is read with the unpaged command, which OMP answers while busy (its
//! own client does the same). That snapshot lacks the message being streamed (OMP adds a message
//! at its `message_end`) but lists the turn's finished ones. A chat that watched the turn shows
//! them already, so the UI skips history messages it shows (same role and timestamp). If the
//! unpaged read fails too the read gives up without an error, see
//! [`RpcSession::unreadable_history`].

use std::time::Duration;

use serde_json::Value;

use super::session::{RpcSession, refusal};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, AgentReply, NoticeLevel};
use crate::pi_family::normalize::string;
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::items;
use crate::pi_family::rpc::{RequestKind, RpcResponse};

/// History pages read at most (256 messages each).
const MAX_HISTORY_PAGES: usize = 64;

/// How often a read starts over after a stale cursor.
const MAX_RESTARTS: usize = 3;

/// Pauses before the successive retries of a busy session (750 ms in all).
const BUSY_BACKOFF: [Duration; 4] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
];

/// Shown when the history stays unreadable.
const UNREADABLE: &str =
    "The earlier messages could not be read while the harness was busy. Reload the chat once it is idle.";

/// How one pass over the pages ended.
enum Pass {
    /// Every message, oldest first.
    Read(Vec<Value>),
    /// The session changed under the cursor.
    Stale,
    /// The session refuses to page while it works.
    Busy,
}

impl RpcSession {
    /// Every message of the history, oldest first, or `None` when the harness keeps refusing to
    /// give it. A failure of the transport or of a harness that is not busy is an error.
    pub(super) async fn read_history(&self) -> Result<Option<Vec<Value>>> {
        if !self.dialect.history_paged() {
            return self.read_whole().await.map(Some);
        }

        let mut restarts = 0;
        let mut pauses = BUSY_BACKOFF.iter();

        loop {
            match self.read_pages().await? {
                Pass::Read(raw) => return Ok(Some(raw)),
                Pass::Stale if restarts < MAX_RESTARTS => restarts += 1,
                Pass::Busy => match pauses.next() {
                    Some(pause) => tokio::time::sleep(*pause).await,
                    None => break,
                },
                Pass::Stale => break,
            }
        }

        match self.read_whole().await {
            Ok(raw) => Ok(Some(raw)),
            // The harness refused this read too (e.g. a reply over its frame limit).
            Err(BusinessError::Protocol(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The empty answer for a history that stayed unreadable, with a notice in the chat. Only the
    /// newest page degrades: asking for older ones fails, so the chat keeps its cursor.
    pub(super) fn unreadable_history(&self, cursor: Option<&str>) -> Result<AgentReply> {
        if cursor.is_some() {
            return Err(BusinessError::Protocol(UNREADABLE.to_owned()));
        }

        self.shared.emit(AgentEvent::Notice {
            level: NoticeLevel::Warning,
            message: UNREADABLE.to_owned(),
        });

        Ok(AgentReply::History {
            messages: Vec::new(),
            next_cursor: None,
            total: None,
        })
    }

    /// Every message in one reply (a harness that does not page, or the fallback of one that does).
    async fn read_whole(&self) -> Result<Vec<Value>> {
        let id = self.next_id();
        let frame = self.dialect.whole_history_request(&id);
        let response = self.request(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT).await?;

        Ok(items(&response.data.unwrap_or(Value::Null), "messages"))
    }

    /// One pass over the pages from the newest, against a single snapshot of the session.
    async fn read_pages(&self) -> Result<Pass> {
        let mut raw: Vec<Value> = Vec::new();
        let mut cursor: Option<String> = None;
        let mut total: Option<u64> = None;

        for _ in 0..MAX_HISTORY_PAGES {
            let id = self.next_id();
            let frame = self.dialect.history_request(cursor.as_deref(), &id);
            let response = self
                .request_any(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT)
                .await?;

            if !response.success {
                return match paging_refusal(&response) {
                    Some(pass) => Ok(pass),
                    None => Err(refusal(&frame, &response)),
                };
            }

            let data = response.data.unwrap_or(Value::Null);
            let page_total = data.get("totalMessages").and_then(Value::as_u64);

            if total.is_some() && page_total != total {
                return Ok(Pass::Stale);
            }

            total = page_total;
            raw.extend(items(&data, "messages"));
            cursor = string(&data, "nextCursor");

            if cursor.is_none() {
                break;
            }
        }

        // Pages left unread (the cap) are not a gap to retry: they were always dropped.
        let complete = cursor.is_some() || total.is_none_or(|total| total == raw.len() as u64);

        Ok(if complete { Pass::Read(raw) } else { Pass::Stale })
    }
}

/// How the harness refused a page, if it did so for a reason a new pass can cure.
fn paging_refusal(response: &RpcResponse) -> Option<Pass> {
    match response.code.as_deref() {
        Some("session_busy") => Some(Pass::Busy),
        Some("stale_cursor") => Some(Pass::Stale),
        _ => None,
    }
}
