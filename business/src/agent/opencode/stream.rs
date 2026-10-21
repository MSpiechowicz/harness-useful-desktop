//! The event task of an [`OpenCodeSession`](super::OpenCodeSession): reads `GET /global/event`
//! and turns each event into the chat's events under the chat's lock, so a prompt's bookkeeping
//! and the events around it keep one order.
//!
//! A skipped event (too large, not UTF-8, not an event object) becomes a `frameError`.
//!
//! **Watchdog:** OpenCode sends a heartbeat every 10 s. When the stream stays silent for
//! [`SessionOptions::event_silence`](crate::agent::SessionOptions::event_silence) (35 s), or ends
//! while the server still runs, the chat can no longer follow it: an error notice says so, the
//! open dialogs close, a helper chat's session is deleted and the server is ended, so
//! `exited` closes the chat as for any other exit.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::AbortHandle;

use super::{Chat, Live, delivery, reports};
use crate::agent::process::Terminator;
use crate::model::{AgentEvent, NoticeLevel};
use crate::opencode_server::wire::{SessionInfo, envelope};
use crate::opencode_server::{EventStream, Mapped};
use crate::process::ExitInfo;

/// How long the end of the stream waits for the server's exit before reporting a lost stream.
const EXIT_WAIT: Duration = Duration::from_secs(2);

/// Shown when the event stream is lost while the server runs.
const LOST_STREAM: &str = "The connection to OpenCode was lost, so this chat stops. Start it again to continue.";

/// What the event task watches.
pub(super) struct Watch {
    /// Changes to `Some` once the server exited.
    pub exited: watch::Receiver<Option<ExitInfo>>,
    /// Ends the server when the stream is lost.
    pub terminator: Terminator,
    /// The longest silence of a live stream.
    pub silence: Duration,
}

/// Starts the event task.
pub(super) fn spawn(chat: Arc<Chat>, events: EventStream, watch: Watch) -> AbortHandle {
    tokio::spawn(run(chat, events, watch)).abort_handle()
}

async fn run(chat: Arc<Chat>, mut events: EventStream, mut watch: Watch) {
    loop {
        let item = match tokio::time::timeout(watch.silence, events.next()).await {
            Ok(Some(item)) => item,
            Ok(None) => break,
            Err(_) => return lost(&chat, &watch).await,
        };

        match item {
            Ok(data) => take_event(&chat, &data),
            Err(fault) => chat.shared.emit(AgentEvent::FrameError {
                original_type: None,
                error: fault.to_string(),
            }),
        }
    }

    let exit = watch.exited.wait_for(Option::is_some);

    if tokio::time::timeout(EXIT_WAIT, exit).await.is_err() {
        lost(&chat, &watch).await;
    }
}

/// The stream is lost while the server runs: the chat stops (see the module docs).
async fn lost(chat: &Chat, watch: &Watch) {
    chat.shared.emit(AgentEvent::Notice {
        level: NoticeLevel::Error,
        message: LOST_STREAM.to_owned(),
    });

    let cancelled = chat.lock().mapper.cancel_asks();
    cancelled.into_iter().for_each(|event| chat.shared.emit(event));

    chat.delete_helper_session().await;
    watch.terminator.terminate().await;
}

/// Maps one event's `data:` and delivers the result.
fn take_event(chat: &Arc<Chat>, data: &str) {
    let Some(envelope) = envelope(data) else {
        chat.shared.emit(AgentEvent::FrameError {
            original_type: None,
            error: "an OpenCode event is not a JSON event object".to_owned(),
        });
        return;
    };

    let mut live = chat.lock();
    reports::note_context(&mut live, &envelope);
    let mapped = live.mapper.map(&envelope);

    apply(chat, &mut live, mapped);
    chat.event_taken.notify_waiters();
}

/// Carries out what the mapper decided, in order, under the chat's lock.
pub(super) fn apply(chat: &Arc<Chat>, live: &mut Live, mapped: Vec<Mapped>) {
    for item in mapped {
        match item {
            Mapped::Event(event) => chat.shared.emit(event),
            Mapped::PromptTaken => chat.shared.prompt_taken_now(),
            Mapped::Session(info) => chat.session_changed(live, info),
            Mapped::Send(message) => delivery::send(chat, live, message),
            Mapped::CheckStranded(steering) => delivery::check_stranded(chat, steering),
            Mapped::Abort => delivery::abort(chat, live),
        }
    }
}

impl Chat {
    /// Takes in the session's new title, and the model OpenCode uses while the user has not picked
    /// one (a picked one goes with every prompt); a change is delivered as a fresh `state`.
    fn session_changed(&self, live: &mut Live, info: SessionInfo) {
        let mut changed = live.title != info.title;
        live.title = info.title;

        if !live.model_chosen && info.model.is_some() && live.model != info.model {
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
