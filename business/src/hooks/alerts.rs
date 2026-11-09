//! When a chat tells the user something: the alert state of one chat, a pure state machine with
//! no IO that [`super::HookSession`] feeds from the same events the hooks see.
//!
//! - `Finished`: a run a user message started (or joined) ended for good. A run that earns a hook
//!   follow-up does not end for good: its alert is held until the follow-up's run settles, or is
//!   released at once when the follow-up never runs (nothing composed, a user message cancelled
//!   it, the send failed or the harness handled it without a run), or when the chat moves on to
//!   another session (a new, switched, forked or cloned one). The agent's exit drops it.
//! - `Failed`: the run ended with an error ([`RunState::failed`]), or the agent exited unexpectedly
//!   ([`exit_alert`]).
//! - `NeedsYou`: the chat opened a dialog. A chat raises it once per [`NEEDS_YOU_GAP`], so a burst
//!   of approvals is one alert.
//! - Nothing for a run the user stopped (also a stopped follow-up), for a run a user message went
//!   out after (the chat is busy again: the later settle alerts) and for a run only the engine's
//!   own prompt started.
//!
//! The shell reads the result with [`super::HookSession::take_alert`], and decides whether the
//! chat may raise alerts at all.

use std::time::{Duration, Instant};

use super::run::RunState;
use crate::model::ChatAlertKind;

/// A chat does not raise `NeedsYou` twice within this time.
pub const NEEDS_YOU_GAP: Duration = Duration::from_secs(5);

/// The alert of an agent exit: `Failed` unless the process ended cleanly (code 0, no signal). Call
/// it only for an exit nobody asked for, since a stop, a page reload and the app quitting end the
/// agent the same way.
pub fn exit_alert(code: Option<i32>, signal: Option<i32>) -> Option<ChatAlertKind> {
    let clean = code == Some(0) && signal.is_none();

    (!clean).then_some(ChatAlertKind::Failed)
}

/// What one chat has to tell the user, and what it already told.
#[derive(Debug, Default)]
pub(crate) struct Alerts {
    /// The alert due now, until the shell takes it.
    ready: Option<ChatAlertKind>,
    /// The alert of a run whose follow-up has not settled yet.
    held: Option<ChatAlertKind>,
    /// The follow-up's prompt went out as a run that has not settled.
    follow_up_running: bool,
    /// When `NeedsYou` was last raised.
    needs_you_at: Option<Instant>,
}

impl Alerts {
    /// The alert due now, once.
    pub(crate) fn take(&mut self) -> Option<ChatAlertKind> {
        self.ready.take()
    }

    /// The chat opened a dialog at `now`.
    pub(crate) fn dialog_opened(&mut self, now: Instant) {
        let quiet = self
            .needs_you_at
            .is_none_or(|last| now.saturating_duration_since(last) >= NEEDS_YOU_GAP);

        if !quiet {
            return;
        }

        self.needs_you_at = Some(now);
        self.ready = Some(ChatAlertKind::NeedsYou);
    }

    /// A run settled. `superseded`: a user message went out after the settle. `holds`: the run
    /// earned a hook follow-up, so its alert waits for it.
    pub(crate) fn run_settled(&mut self, run: &RunState, superseded: bool, holds: bool) {
        let ends_follow_up = std::mem::take(&mut self.follow_up_running);

        if ends_follow_up {
            self.held = None;
        }

        // A run only the engine's own prompt started has nothing to tell, unless it is the
        // follow-up the held alert waits for.
        if !ends_follow_up && !run.user_sent() {
            return;
        }

        if superseded || run.aborted {
            return;
        }

        let kind = if run.failed {
            ChatAlertKind::Failed
        } else {
            ChatAlertKind::Finished
        };

        if holds {
            self.held = Some(kind);
        } else {
            self.ready = Some(kind);
        }
    }

    /// The follow-up's prompt was accepted and runs: the held alert waits for that run.
    pub(crate) fn follow_up_started(&mut self) {
        self.follow_up_running = self.held.is_some();
    }

    /// The follow-up will not run: the held alert is due now.
    pub(crate) fn release(&mut self) {
        if let Some(kind) = self.held.take() {
            self.ready = Some(kind);
        }
    }

    /// The runs were dropped (a new, other, forked or cloned session): the alert of a run that
    /// waited for its follow-up is due now, since the chat moved on and the run did end.
    pub(crate) fn runs_dropped(&mut self) {
        self.release();
        self.follow_up_running = false;
    }

    /// The agent exited: nothing waits any more, and what it held is dropped (the shell decides
    /// about the exit itself).
    pub(crate) fn exited(&mut self) {
        self.held = None;
        self.follow_up_running = false;
    }
}
