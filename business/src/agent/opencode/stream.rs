//! The event task of an [`OpenCodeSession`](super::OpenCodeSession): reads `GET /global/event`
//! and turns each event into the chat's events under the chat's lock, so a prompt's bookkeeping
//! and the events around it keep one order.
//!
//! A skipped event (too large, not UTF-8, not an event object) becomes a `frameError`. When the
//! stream ends while the server still runs, the chat can no longer follow it: an error notice
//! says so.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::AbortHandle;

use super::{Chat, Live};
use crate::model::{AgentEvent, NoticeLevel};
use crate::opencode_server::wire::{SessionInfo, envelope};
use crate::opencode_server::{EventStream, Mapped};
use crate::process::ExitInfo;

/// How long the end of the stream waits for the server's exit before reporting a lost stream.
const EXIT_WAIT: Duration = Duration::from_secs(2);

/// Shown when the event stream ends while the server runs.
const LOST_STREAM: &str = "The connection to OpenCode's event stream was lost, so this chat no longer updates. \
                           Restart the chat to continue.";

/// Starts the event task.
pub(super) fn spawn(chat: Arc<Chat>, events: EventStream, exited: watch::Receiver<Option<ExitInfo>>) -> AbortHandle {
    tokio::spawn(run(chat, events, exited)).abort_handle()
}

async fn run(chat: Arc<Chat>, mut events: EventStream, mut exited: watch::Receiver<Option<ExitInfo>>) {
    while let Some(item) = events.next().await {
        match item {
            Ok(data) => chat.take_event(&data),
            Err(fault) => chat.shared.emit(AgentEvent::FrameError {
                original_type: None,
                error: fault.to_string(),
            }),
        }
    }

    let exit = exited.wait_for(Option::is_some);

    if tokio::time::timeout(EXIT_WAIT, exit).await.is_err() {
        chat.shared.emit(AgentEvent::Notice {
            level: NoticeLevel::Error,
            message: LOST_STREAM.to_owned(),
        });
    }
}

impl Chat {
    /// Maps one event's `data:` and delivers the result.
    fn take_event(&self, data: &str) {
        let Some(envelope) = envelope(data) else {
            self.shared.emit(AgentEvent::FrameError {
                original_type: None,
                error: "an OpenCode event is not a JSON event object".to_owned(),
            });
            return;
        };

        let mut live = self.lock();

        for mapped in live.mapper.map(&envelope) {
            match mapped {
                Mapped::Event(event) => self.shared.emit(event),
                Mapped::PromptTaken => self.shared.prompt_taken_now(),
                Mapped::Session(info) => self.session_changed(&mut live, info),
            }
        }
    }

    /// Takes in the session's new title, and its model while the chat does not know one yet
    /// (the chat's own choice is sent with every prompt, so it is never overridden); a change is
    /// delivered as a fresh `state`.
    fn session_changed(&self, live: &mut Live, info: SessionInfo) {
        let mut changed = live.title != info.title;
        live.title = info.title;

        if live.model.is_none() && info.model.is_some() {
            live.model = info.model;
            live.variant = info.variant;
            changed = true;
        }

        if changed {
            self.shared.emit(AgentEvent::State {
                state: self.state(live),
            });
        }
    }
}
