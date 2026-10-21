//! The event task of an [`OpenCodeV2Session`](super::OpenCodeV2Session): reads `GET /api/event`
//! and turns each event into the chat's events under the chat's lock, so a prompt's bookkeeping
//! and the events around it keep one order.
//!
//! - An event that cannot be read (too large, not UTF-8, not an event object, a known type without
//!   its fields) becomes a `frameError`.
//! - **Reconnect:** OpenCode 2 ends a subscriber's stream when it falls behind. A stream that ends
//!   (or breaks) while the server runs is opened again and the chat catches up from the session
//!   (`catch_up.rs`), so nothing shows twice. A stream that could not be opened, and one that ended
//!   within [`SHORT_LIVED`] of opening, count as failed attempts: each further one waits twice as
//!   long, and after [`RECONNECT_ATTEMPTS`] in a row the stream is lost. A stream that lived
//!   longer starts the count again. A server that exited cannot be reached: its exit closes the
//!   chat as for any other exit, without a notice.
//! - **Watchdog:** OpenCode 2's heartbeats are SSE comments (`: heartbeat` every 15 s), which the
//!   reader never passes on as events but counts as signs of life
//!   ([`EventStream::last_activity`]). A stream that sent nothing, not even a heartbeat, for
//!   [`SessionOptions::event_silence`](crate::agent::SessionOptions::event_silence) asks
//!   `GET /api/info` whether the server still answers. When it does, the stream stalled: it is
//!   opened again and the chat catches up, as for a stream that ended. When the server does not
//!   answer, or when the stream cannot be opened again while the server runs, the chat can no
//!   longer follow it: an error notice says so, the open dialogs close, a helper chat's session is
//!   deleted and the server is ended, so `exited` closes the chat.
//! - **Approval mode:** OpenCode reloading its agents, and a stream opened again, check the mode
//!   again (`guard.rs`); when it no longer holds, the chat stops the same way with its own notice.
//! - **Resume:** before the first event, the subagent sessions of a resumed session count as the
//!   chat's (`catch_up.rs`), so their asks reach the chat.
//! - **Subagent sessions named by a call:** a `subagent` call that names a session the chat does
//!   not count has the subagent sessions listed again before the next event (`catch_up.rs`): the
//!   session counts only when OpenCode lists it as a child of the chat's sessions.
//! - A permission ask the chat's "Always allow" answers cover is answered without a dialog
//!   (`approvals.rs`).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::AbortHandle;
use tokio::time::Instant;

use super::{Chat, Live, approvals, catch_up, delivery, guard, normalized, runs, shell_output};
use crate::agent::process::Terminator;
use crate::model::{AgentEvent, NoticeLevel};
use crate::opencode_server::EventStream;
use crate::opencode_server_v2::events::{self, EventError, EventKind};
use crate::opencode_server_v2::mapper::SessionChange;
use crate::opencode_server_v2::{Mapped, RouteV2};
use crate::process::ExitInfo;

/// How long a stream that could not be opened again waits for the server's exit before reporting
/// a lost stream.
const EXIT_WAIT: Duration = Duration::from_secs(2);

/// Failed attempts in a row (a stream that did not open or was short-lived) before the chat gives
/// up.
const RECONNECT_ATTEMPTS: u32 = 3;

/// A stream that ends sooner after it opened counts as a failed attempt.
const SHORT_LIVED: Duration = Duration::from_secs(1);

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
    /// The chat resumed its session.
    pub resumed: bool,
}

/// How reading a stream ended.
enum Ended {
    /// The stream ended or broke.
    Closed,
    /// The stream went silent while the server answers.
    Stalled,
    /// The stream went silent and the server does not answer.
    Silent,
    /// The approval mode no longer holds (`guard.rs`).
    Unguarded,
}

/// Starts the event task.
pub(super) fn spawn(chat: Arc<Chat>, events: EventStream, watch: Watch) -> AbortHandle {
    tokio::spawn(run(chat, events, watch)).abort_handle()
}

async fn run(chat: Arc<Chat>, mut events: EventStream, mut watch: Watch) {
    if watch.resumed {
        catch_up::adopt_children(&chat).await;
    }

    let mut failures = 0;

    loop {
        let opened = Instant::now();

        match follow(&chat, &mut events, &watch).await {
            Ended::Silent => return stop(&chat, &watch, LOST_STREAM).await,
            Ended::Unguarded => return stop(&chat, &watch, guard::NOTICE).await,
            Ended::Stalled => failures = 0,
            Ended::Closed if opened.elapsed() < SHORT_LIVED => failures += 1,
            Ended::Closed => failures = 0,
        }

        let Some((reopened, failed)) = reconnect(&chat, &watch, failures).await else {
            return gone(&chat, &mut watch).await;
        };

        events = reopened;
        failures = failed;
        catch_up::run(&chat).await;

        if guard::recheck(&chat).await.is_err() {
            return stop(&chat, &watch, guard::NOTICE).await;
        }
    }
}

/// Reads `events` until the stream ends, goes silent or the approval mode no longer holds.
async fn follow(chat: &Arc<Chat>, events: &mut EventStream, watch: &Watch) -> Ended {
    loop {
        let quiet_until = events.last_activity() + watch.silence;

        let item = match tokio::time::timeout_at(quiet_until, events.next()).await {
            Ok(Some(item)) => item,
            Ok(None) => return Ended::Closed,
            // A heartbeat came meanwhile.
            Err(_) if events.last_activity() + watch.silence > Instant::now() => continue,
            Err(_) if answers(chat).await => {
                tracing::debug!("the OpenCode event stream went silent: it is opened again");
                return Ended::Stalled;
            }
            Err(_) => return Ended::Silent,
        };

        let recheck = match item {
            Ok(data) => take_event(chat, &data),
            Err(fault) => {
                chat.shared.emit(AgentEvent::FrameError {
                    original_type: None,
                    error: fault.to_string(),
                });
                false
            }
        };

        if recheck && guard::recheck(chat).await.is_err() {
            return Ended::Unguarded;
        }

        let unlisted = chat.lock().mapper.take_unlisted();

        if !unlisted.is_empty() {
            catch_up::confirm_children(chat).await;
        }
    }
}

/// True when the server answers `GET /api/info`.
async fn answers(chat: &Chat) -> bool {
    let answer = chat.client.send_with_timeout(RouteV2::Info, None, PING_TIMEOUT).await;

    answer.is_ok_and(|response| response.is_success())
}

/// Opens the event stream again after `failures` failed attempts in a row (see the module docs);
/// the stream and the failed attempts so far, or `None` once the server exited or the attempts
/// ran out.
async fn reconnect(chat: &Chat, watch: &Watch, mut failures: u32) -> Option<(EventStream, u32)> {
    loop {
        if failures >= RECONNECT_ATTEMPTS {
            return None;
        }

        if failures > 0 {
            tokio::time::sleep(RECONNECT_DELAY * 2_u32.pow(failures - 1)).await;
        }

        if watch.exited.borrow().is_some() {
            return None;
        }

        match chat.client.stream(RouteV2::Event).await {
            Ok(events) => return Some((events, failures)),
            Err(error) => tracing::debug!(%error, failures, "the OpenCode event stream did not open again"),
        }

        failures += 1;
    }
}

/// The stream cannot be opened again: the server's exit closes the chat, else the stream is lost.
async fn gone(chat: &Chat, watch: &mut Watch) {
    let exit = watch.exited.wait_for(Option::is_some);

    if tokio::time::timeout(EXIT_WAIT, exit).await.is_err() {
        stop(chat, watch, LOST_STREAM).await;
    }
}

/// The chat stops while the server runs, `message` says why (see the module docs).
async fn stop(chat: &Chat, watch: &Watch, message: &str) {
    chat.shared.emit(AgentEvent::Notice {
        level: NoticeLevel::Error,
        message: message.to_owned(),
    });

    let cancelled = chat.lock().mapper.cancel_asks();
    cancelled.into_iter().for_each(|event| chat.shared.emit(event));

    chat.delete_helper_session().await;
    watch.terminator.terminate().await;
}

/// Maps one event's `data:` and delivers the result; true when the approval mode must be checked
/// again (`guard.rs`).
fn take_event(chat: &Arc<Chat>, data: &str) -> bool {
    let event = match events::parse(data) {
        Ok(event) => event,
        Err(error) => {
            let original_type = match &error {
                EventError::Data { kind } => Some(kind.clone()),
                EventError::Envelope => None,
            };

            chat.shared.emit(AgentEvent::FrameError {
                original_type,
                error: error.to_string(),
            });
            return false;
        }
    };

    let recheck = match &event.kind {
        EventKind::PermissionAsked(permission) => guard::unknown_agent(chat, permission),
        kind => guard::rechecks(kind),
    };

    let mut live = chat.lock();

    if let Some(created) = event.created {
        live.mark = live.mark.max(created);
    }

    if approvals::answered_by_always(chat, &live, &event) {
        return recheck;
    }

    let mapped = live.mapper.map(&event);
    apply(chat, &mut live, mapped);
    shell_output::follow(chat, &mut live);

    recheck
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
