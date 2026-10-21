//! The event task of an [`OpenCodeV2Session`](super::OpenCodeV2Session): reads `GET /api/event`
//! and turns each event into the chat's events under the chat's lock, so a prompt's bookkeeping
//! and the events around it keep one order.
//!
//! - An event that cannot be read (too large, not UTF-8, not an event object, a known type without
//!   its fields) becomes a `frameError`.
//! - **Reconnect:** OpenCode 2 ends a subscriber's stream when it falls behind. A stream that ends
//!   (or breaks) while the server runs is opened again, up to [`RECONNECT_ATTEMPTS`] times, and the
//!   chat catches up from the session (`catch_up.rs`), so nothing shows twice. A server that
//!   exited cannot be reached: its exit closes the chat as for any other exit, without a notice.
//! - **Watchdog:** OpenCode 2's heartbeats are SSE comments the reader never passes on, so a
//!   stream silent for [`SessionOptions::event_silence`](crate::agent::SessionOptions::event_silence)
//!   asks `GET /api/info` whether the server still answers. When it does not, or when the stream
//!   cannot be opened again while the server runs, the chat can no longer follow it: an error
//!   notice says so, the open dialogs close, a helper chat's session is deleted and the server is
//!   ended, so `exited` closes the chat.
//! - A permission ask the chat's "Always allow" answers cover is answered without a dialog
//!   (`approvals.rs`).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::AbortHandle;

use super::{Chat, Live, approvals, catch_up, delivery, normalized, runs, shell_output};
use crate::agent::process::Terminator;
use crate::model::{AgentEvent, NoticeLevel};
use crate::opencode_server::EventStream;
use crate::opencode_server_v2::events::{self, EventError};
use crate::opencode_server_v2::mapper::SessionChange;
use crate::opencode_server_v2::{Mapped, RouteV2};
use crate::process::ExitInfo;

/// How long a stream that could not be opened again waits for the server's exit before reporting
/// a lost stream.
const EXIT_WAIT: Duration = Duration::from_secs(2);

/// How often a stream that ended is opened again before the chat gives up.
const RECONNECT_ATTEMPTS: u32 = 3;

/// The wait before the second attempt, doubled for each further one.
const RECONNECT_DELAY: Duration = Duration::from_millis(100);

/// Deadline of the liveness check of a silent stream.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Shown when the event stream is lost while the server runs.
const LOST_STREAM: &str = "The connection to OpenCode was lost, so this chat stops. Start it again to continue.";

/// What the event task watches.
pub(super) struct Watch {
    /// Changes to `Some` once the server exited.
    pub exited: watch::Receiver<Option<ExitInfo>>,
    /// Ends the server when the stream is lost.
    pub terminator: Terminator,
    /// The longest silence before the server is asked whether it still answers.
    pub silence: Duration,
}

/// How reading a stream ended.
enum Ended {
    /// The stream ended or broke.
    Closed,
    /// The stream went silent and the server does not answer.
    Silent,
}

/// Starts the event task.
pub(super) fn spawn(chat: Arc<Chat>, events: EventStream, watch: Watch) -> AbortHandle {
    tokio::spawn(run(chat, events, watch)).abort_handle()
}

async fn run(chat: Arc<Chat>, mut events: EventStream, mut watch: Watch) {
    loop {
        if let Ended::Silent = follow(&chat, &mut events, &watch).await {
            return lost(&chat, &watch).await;
        }

        let Some(reopened) = reconnect(&chat, &watch).await else {
            return gone(&chat, &mut watch).await;
        };

        events = reopened;
        catch_up::run(&chat).await;
    }
}

/// Reads `events` until the stream ends, or goes silent with a server that does not answer.
async fn follow(chat: &Arc<Chat>, events: &mut EventStream, watch: &Watch) -> Ended {
    loop {
        let item = match tokio::time::timeout(watch.silence, events.next()).await {
            Ok(Some(item)) => item,
            Ok(None) => return Ended::Closed,
            Err(_) if answers(chat).await => continue,
            Err(_) => return Ended::Silent,
        };

        match item {
            Ok(data) => take_event(chat, &data),
            Err(fault) => chat.shared.emit(AgentEvent::FrameError {
                original_type: None,
                error: fault.to_string(),
            }),
        }
    }
}

/// True when the server answers `GET /api/info`.
async fn answers(chat: &Chat) -> bool {
    let answer = chat.client.send_with_timeout(RouteV2::Info, None, PING_TIMEOUT).await;

    answer.is_ok_and(|response| response.is_success())
}

/// Opens the event stream again; `None` once the server exited or after the last attempt.
async fn reconnect(chat: &Chat, watch: &Watch) -> Option<EventStream> {
    let mut delay = RECONNECT_DELAY;

    for attempt in 1..=RECONNECT_ATTEMPTS {
        if watch.exited.borrow().is_some() {
            return None;
        }

        match chat.client.stream(RouteV2::Event).await {
            Ok(events) => return Some(events),
            Err(error) => tracing::debug!(%error, attempt, "the OpenCode event stream did not open again"),
        }

        if attempt < RECONNECT_ATTEMPTS {
            tokio::time::sleep(delay).await;
            delay *= 2;
        }
    }

    None
}

/// The stream cannot be opened again: the server's exit closes the chat, else the stream is lost.
async fn gone(chat: &Chat, watch: &mut Watch) {
    let exit = watch.exited.wait_for(Option::is_some);

    if tokio::time::timeout(EXIT_WAIT, exit).await.is_err() {
        lost(chat, watch).await;
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
    let event = match events::parse(data) {
        Ok(event) => event,
        Err(error) => {
            let original_type = match &error {
                EventError::Data { kind } => Some(kind.clone()),
                EventError::Envelope => None,
            };

            return chat.shared.emit(AgentEvent::FrameError {
                original_type,
                error: error.to_string(),
            });
        }
    };

    let mut live = chat.lock();

    if let Some(created) = event.created {
        live.mark = live.mark.max(created);
    }

    if approvals::answered_by_always(chat, &live, &event) {
        return;
    }

    let mapped = live.mapper.map(&event);
    apply(chat, &mut live, mapped);
    shell_output::follow(chat, &mut live);
}

/// Carries out what the mapper decided, in order, under the chat's lock.
pub(super) fn apply(chat: &Arc<Chat>, live: &mut Live, mapped: Vec<Mapped>) {
    for item in mapped {
        match item {
            Mapped::Event(event) => chat.shared.emit(*event),
            Mapped::PromptTaken => chat.shared.prompt_taken_now(),
            Mapped::Session(change) => chat.session_changed(live, change),
            Mapped::Send(message) => delivery::send(chat, live, message),
            Mapped::Abort => delivery::abort(chat, live),
            Mapped::Context(note) => live.context = Some(note),
            Mapped::ShellDone(result) => runs::shell_done(live, result),
            // The statistics read the session's totals when asked.
            Mapped::Usage(_) => {}
        }
    }
}

impl Chat {
    /// Takes in a change of the session's title, agent or model; one that differs from what the
    /// chat shows is delivered as a fresh `state`. OpenCode's confirmation of the chat's own pick
    /// changes nothing: the pick already shows, and a later pick may have replaced it.
    pub(super) fn session_changed(&self, live: &mut Live, change: SessionChange) {
        let change = match change {
            SessionChange::Model(model) => SessionChange::Model(normalized(model)),
            other => other,
        };

        if live.confirms(&change) {
            return;
        }

        let changed = match change {
            SessionChange::Title(title) => replace(&mut live.title, Some(title)),
            SessionChange::Agent(agent) => replace(&mut live.agent, agent),
            SessionChange::Model(model) => replace(&mut live.model, Some(model)),
        };

        if changed {
            self.shared.emit(AgentEvent::State {
                state: self.state(live),
            });
        }
    }
}

/// Puts `value` into `slot`; true when that changed it.
fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }

    *slot = value;
    true
}
