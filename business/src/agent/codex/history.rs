//! The saved messages of a [`CodexSession`]'s thread: the history, the last reply, the HTML export
//! and the message counts of a thread the chat enters with saved turns.
//!
//! - **Read:** every page of `thread/turns/list` and `thread/items/list`, oldest first, on the
//!   chat's own process (the thread is loaded there), turned into chat messages the way the
//!   Sessions view reads them ([`transcript`]). A thread without saved turns (a new chat before its
//!   first turn, a helper chat) has none and is not asked.
//! - **History:** its first page reads the thread again, later pages page that read.
//! - **Last reply:** the newest of the chat, else the newest saved one.
//! - **Export:** the shared HTML export ([`write_export`]) as `codex-<thread id>.html`, titled with
//!   the thread's name. Refused in a helper chat.
//! - **Counts:** a resumed, forked or cloned thread's saved messages count in the statistics
//!   (read in the background, best effort), and its newest reply is the last reply.

use std::sync::Arc;

use serde::de::DeserializeOwned;

use super::{Chat, CodexSession};
use crate::agent::export::{Export, write_export};
use crate::claude_stream::stats::reply_text;
use crate::codex_rpc::stats::MessageCounts;
use crate::codex_rpc::wire::items::Turn;
use crate::codex_rpc::wire::outbound::Request;
use crate::codex_rpc::wire::results::{ItemEntry, Page};
use crate::error::{BusinessError, Result};
use crate::harnesses::codex::is_thread_id;
use crate::harnesses::codex::sessions::{
    MAX_ITEMS, MAX_TURNS, Origin, Transcript, UNTITLED, items_page, transcript, turns_page,
};
use crate::model::{AgentAction, AgentReply, ChatMessage, MessageRole};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::page_history;

impl CodexSession {
    /// Runs a history, last reply or export action.
    pub(super) async fn report(&self, action: AgentAction) -> Result<AgentReply> {
        match action {
            AgentAction::GetHistory { cursor, limit } => self.history(cursor.as_deref(), limit).await,
            AgentAction::GetLastAssistantText => self.last_reply().await,
            AgentAction::ExportHtml => self.export_html().await,
            _ => Err(BusinessError::Internal("not a report action".to_owned())),
        }
    }

    /// The history page ending before `cursor`.
    async fn history(&self, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
        let read = self.chat.saved_messages(cursor.is_none()).await?;
        let messages = read.as_ref().map(|read| read.messages.as_slice()).unwrap_or_default();

        page_history(messages, cursor, limit)
    }

    /// `getLastAssistantText` (see the module docs).
    async fn last_reply(&self) -> Result<AgentReply> {
        if let Some(text) = self.chat.lock().last_reply.clone() {
            return Ok(AgentReply::Text { text: Some(text) });
        }

        let read = self.chat.saved_messages(true).await?;
        let messages = read.as_ref().map(|read| read.messages.as_slice()).unwrap_or_default();

        Ok(AgentReply::Text {
            text: newest_reply(messages),
        })
    }

    /// `exportHtml` (see the module docs).
    async fn export_html(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        chat.saves_threads()?;

        let (thread, title) = {
            let live = chat.lock();
            (live.thread_id.clone(), live.title.clone())
        };

        // The id names the export file.
        let Some(thread) = thread.filter(|id| is_thread_id(id)) else {
            return Err(BusinessError::Internal("the chat's thread id is not valid".to_owned()));
        };

        let read = chat.saved_messages(true).await?;
        let export = Export {
            harness: "Codex",
            prefix: "codex",
            title: title.as_deref().unwrap_or(UNTITLED),
            session_id: &thread,
            messages: read.as_ref().map(|read| read.messages.as_slice()).unwrap_or_default(),
        };

        write_export(&chat.shared, chat.export_dir.as_ref(), &export)
    }
}

impl Chat {
    /// The thread's saved messages: the cached read unless `fresh`; `None` when it has none.
    async fn saved_messages(&self, fresh: bool) -> Result<Option<Arc<Transcript>>> {
        let (thread, cached) = {
            let live = self.lock();

            if self.helper || !live.saved {
                return Ok(None);
            }

            (live.thread_id.clone(), live.transcript.clone())
        };

        let Some(thread) = thread else {
            return Ok(None);
        };

        if let Some(cached) = cached.filter(|_| !fresh) {
            return Ok(Some(cached));
        }

        let read = self.read_thread(&thread).await?;

        let mut live = self.lock();
        if live.thread_id.as_deref() == Some(thread.as_str()) {
            live.transcript = Some(read.clone());
        }

        Ok(Some(read))
    }

    /// Reads every saved message of `thread` (see the module docs).
    pub(super) async fn read_thread(&self, thread: &str) -> Result<Arc<Transcript>> {
        let turns: Vec<Turn> = self.all_pages(MAX_TURNS, |cursor| turns_page(thread, cursor)).await?;
        let items: Vec<ItemEntry> = self.all_pages(MAX_ITEMS, |cursor| items_page(thread, cursor)).await?;

        let (model, provider) = {
            let live = self.lock();
            (live.thread_model.clone(), live.provider.clone())
        };
        let origin = Origin {
            model: model.as_deref(),
            provider: Some(provider.as_str()).filter(|provider| !provider.is_empty()),
        };

        Ok(Arc::new(transcript(&turns, &items, origin, &self.project)))
    }

    /// The entries of every page `request` asks for (by cursor), at most `max`.
    async fn all_pages<T: DeserializeOwned>(
        &self,
        max: usize,
        request: impl Fn(Option<String>) -> Request,
    ) -> Result<Vec<T>> {
        let mut entries: Vec<T> = Vec::new();
        let mut cursor: Option<String> = None;

        while entries.len() < max {
            let page: Page<T> = self.request(&request(cursor.take()), DEFAULT_TIMEOUT).await?;
            let more = !page.data.is_empty();
            entries.extend(page.data);

            match page.next_cursor {
                Some(next) if more => cursor = Some(next),
                _ => break,
            }
        }

        entries.truncate(max);

        Ok(entries)
    }

    /// Counts the saved messages of the thread the chat just entered, in the background (see the
    /// module docs).
    pub(super) fn count_saved(self: &Arc<Self>) {
        let chat = self.clone();

        tokio::spawn(async move {
            let Some(thread) = chat.lock().thread_id.clone() else {
                return;
            };

            match chat.read_thread(&thread).await {
                Ok(read) => chat.counted(&thread, &read.messages),
                Err(error) => tracing::debug!(%error, "the saved messages of a Codex thread were not counted"),
            }
        });
    }

    /// Adds the saved `messages` of `thread` to its counts, unless the chat moved on meanwhile.
    fn counted(&self, thread: &str, messages: &[ChatMessage]) {
        let mut live = self.lock();

        if live.thread_id.as_deref() != Some(thread) {
            return;
        }

        let saved = MessageCounts::of(messages);
        let since = live.counts;
        live.counts = MessageCounts {
            user: saved.user + since.user,
            assistant: saved.assistant + since.assistant,
            tool_calls: saved.tool_calls + since.tool_calls,
            total: saved.total + since.total,
        };

        if live.last_reply.is_none() {
            live.last_reply = newest_reply(messages);
        }

        self.emit_state(&live);
    }
}

/// The text of the newest reply among `messages`.
fn newest_reply(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == MessageRole::Assistant)
        .find_map(|message| reply_text(&message.blocks))
}
