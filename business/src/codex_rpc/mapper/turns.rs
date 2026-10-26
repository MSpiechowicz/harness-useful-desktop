//! Turns, their outcome, retries, compactions and settling.
//!
//! - **Turns:** `turn/started` begins one (`agentStart`, `turnStart`); an item of a turn whose
//!   start was missed begins it too, an item of the turn that ended last is dropped.
//!   `turn/completed` ends tool cards still running (as failed, [`UNFINISHED_TOOL`]) and replies
//!   still open (as `aborted` after an interrupt), then `turnEnd`, `agentEnd` and one
//!   `promptResult`: `aborted` for an interrupted turn (or a failed one after
//!   [`CodexMapper::abort_requested`]), `error` for a failed one, else `completed`. A quiet turn
//!   ([`StartKind::Compact`], [`StartKind::Shell`]) shows no `agentStart` … `agentEnd`: a
//!   compaction ends with a `handled` result, a `!command` only shows a failure (its command is
//!   no tool card: the engine answers the `!command` with its output, see [`Hint::Shell`]).
//! - **Errors:** `error{willRetry}` is a `retry` `start` (its attempt counted), the turn's next item
//!   or its end the retry's `end`. Without `willRetry` it is the turn's failure, unless the
//!   completed turn names its own. Codex's `unauthorized` becomes [`SIGN_IN_ERROR`]: the app never
//!   signs in itself.
//! - **Compaction:** a `contextCompaction` item is a `compaction` `start` … `end`, `manual` in a
//!   turn of [`StartKind::Compact`], else `auto`.
//! - **Settling:** a turn's end settles the chat (`settled`) once no turn runs, none the engine
//!   started waits, and (unless the turn was aborted) the engine's follow-up queue is empty.
//!
//! [`UNFINISHED_TOOL`]: super::UNFINISHED_TOOL

use super::{CodexMapper, Hint, Out, StartKind, notice};
use crate::codex_rpc::wire::items::{TurnError, TurnStatus};
use crate::codex_rpc::wire::notifications::ErrorNotification;
use crate::model::{AgentEvent, NoticeLevel, PromptStatus};
use crate::security::redact_text;

/// Shown when Codex has no login: the user signs in with Codex's own flow.
pub const SIGN_IN_ERROR: &str = "Codex is not signed in. Sign in from a terminal: run codex login.";

/// The error shown when Codex names none.
const GENERIC_ERROR: &str = "Codex reported an error.";

/// The error kind of a missing or rejected login.
const UNAUTHORIZED: &str = "unauthorized";

/// Error kinds that may pass when the prompt is sent again.
const RETRYABLE: [&str; 7] = [
    "rateLimitExceeded",
    "serverOverloaded",
    "internalServerError",
    "httpConnectionFailed",
    "responseStreamConnectionFailed",
    "responseStreamDisconnected",
    "responseTooManyFailedAttempts",
];

/// A turn's end, for the engine (statistics, its queue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnEnd {
    pub turn_id: String,
    pub kind: StartKind,
    pub status: PromptStatus,
    pub error: Option<String>,
}

/// The running turn (see the module docs).
#[derive(Default)]
pub(super) struct Turn {
    id: Option<String>,
    active: bool,
    kind: StartKind,
    /// The user asked to stop it.
    stop_requested: bool,
    /// The failure an `error` notification reported.
    failure: Option<Failure>,
    /// The attempt of a running retry.
    retrying: Option<u32>,
    /// The compaction of the turn: `false` while it runs, `true` once it ended.
    compaction: Option<bool>,
    /// The `clientUserMessageId` of the turn's first user message.
    request_id: Option<String>,
    /// The turn that ended last.
    ended: Option<String>,
}

#[derive(Clone)]
struct Failure {
    message: String,
    retryable: bool,
}

impl Turn {
    pub(super) fn id(&self) -> Option<&str> {
        self.id.as_deref().filter(|_| self.active)
    }

    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    pub(super) fn is_compacting(&self) -> bool {
        self.compaction == Some(false)
    }

    pub(super) fn abort_requested(&mut self) {
        self.stop_requested = true;
    }

    /// Nothing the engine started waits any more: a stop asked for meanwhile is forgotten.
    pub(super) fn forget_stop(&mut self) {
        if !self.active {
            self.stop_requested = false;
        }
    }

    /// The turn's user message with `client_id` (the first names the turn's prompt).
    pub(super) fn user_message(&mut self, client_id: Option<&str>) {
        if self.request_id.is_none() {
            self.request_id = client_id.map(str::to_owned);
        }
    }

    fn is_quiet(&self) -> bool {
        self.kind != StartKind::Prompt
    }

    /// True while a prompt's turn (not a quiet one) runs.
    pub(super) fn is_prompt_running(&self) -> bool {
        self.active && !self.is_quiet()
    }

    /// True while a `!command`'s turn runs.
    pub(super) fn is_shell(&self) -> bool {
        self.active && self.kind == StartKind::Shell
    }
}

impl CodexMapper {
    /// `turn/started` of turn `id`.
    pub(super) fn turn_started(&mut self, id: &str, out: &mut Out) {
        if self.turn.active {
            if id.is_empty() || self.turn.id.as_deref() == Some(id) {
                return;
            }

            // Codex never reported the running turn done.
            self.finish(TurnStatus::Completed, None, out);
        }

        let kind = self.pending.pop_front().unwrap_or_default();

        self.turn = Turn {
            id: Some(id.to_owned()).filter(|id| !id.is_empty()),
            active: true,
            kind,
            stop_requested: self.turn.stop_requested,
            ..Turn::default()
        };

        if kind == StartKind::Prompt {
            self.deferred = None;
            out.event(AgentEvent::AgentStart);
            out.event(AgentEvent::TurnStart);
        }

        out.hint(Hint::TurnStarted(id.to_owned()));
    }

    /// `turn/completed` of `turn`.
    pub(super) fn turn_completed(&mut self, turn: &crate::codex_rpc::wire::items::Turn, out: &mut Out) {
        if !self.turn.active {
            if turn.id.is_empty() || self.turn.ended.as_deref() == Some(turn.id.as_str()) {
                return;
            }

            self.turn_started(&turn.id, out);
        }

        let other = matches!(&self.turn.id, Some(running) if !turn.id.is_empty() && *running != turn.id);

        if !other {
            self.finish(turn.status, turn.error.as_ref(), out);
        }
    }

    /// An item or delta of turn `turn_id`: false when it belongs to the turn that ended last. The
    /// turn runs (it begins when its start was missed) and a running retry ends.
    pub(super) fn step(&mut self, turn_id: &str, out: &mut Out) -> bool {
        if !self.ensure_turn(turn_id, out) {
            return false;
        }

        if let Some(attempt) = self.turn.retrying.take() {
            out.event(retry_ended(attempt, None));
        }

        true
    }

    fn ensure_turn(&mut self, turn_id: &str, out: &mut Out) -> bool {
        if self.turn.active {
            return true;
        }

        if !turn_id.is_empty() && self.turn.ended.as_deref() == Some(turn_id) {
            return false;
        }

        self.turn_started(turn_id, out);
        true
    }

    /// `error`: a retry, or the turn's failure.
    pub(super) fn error(&mut self, error: &ErrorNotification, out: &mut Out) {
        let failure = failure(&error.error);

        if error.will_retry {
            if !self.ensure_turn(&error.turn_id, out) {
                return;
            }

            let attempt = self.turn.retrying.map_or(1, |attempt| attempt + 1);
            self.turn.retrying = Some(attempt);

            out.event(AgentEvent::Retry {
                phase: "start".to_owned(),
                attempt: Some(attempt),
                max_attempts: None,
                message: Some(failure.message),
            });
            return;
        }

        if self.turn.active {
            self.turn.failure.get_or_insert(failure);
        } else if self.turn.ended.as_deref() != Some(error.turn_id.as_str()) || error.turn_id.is_empty() {
            notice(NoticeLevel::Error, &failure.message, out);
        }
    }

    /// Ends the running turn as `status`, with the failure Codex names (see the module docs).
    fn finish(&mut self, status: TurnStatus, error: Option<&TurnError>, out: &mut Out) {
        let kind = self.turn.kind;
        let quiet = self.turn.is_quiet();
        let (status, error, retryable) = self.outcome(status, error);
        let aborted = status == PromptStatus::Aborted;

        if self.turn.is_compacting() {
            self.compaction_ended(error.clone(), out);
        }

        if let Some(attempt) = self.turn.retrying.take() {
            out.event(retry_ended(attempt, error.clone()));
        }

        self.tools.end_unfinished(&mut out.events);
        self.replies.end_all(aborted, &mut out.events);
        self.tools.forget();

        if !quiet {
            out.event(AgentEvent::TurnEnd);
            out.event(AgentEvent::AgentEnd);
        }

        if !quiet || kind == StartKind::Compact || status == PromptStatus::Error {
            out.event(AgentEvent::PromptResult {
                request_id: self.turn.request_id.clone(),
                status,
                error: error.clone(),
                retryable,
            });
        }

        let turn_id = self.turn.id.take();

        out.hint(Hint::TurnEnded(TurnEnd {
            turn_id: turn_id.clone().unwrap_or_default(),
            kind,
            status,
            error,
        }));

        self.turn = Turn {
            ended: turn_id,
            ..Turn::default()
        };

        if !quiet {
            self.deferred = Some(aborted);
        }

        self.settle_if_idle(out);
    }

    /// How the turn ended: its prompt status, error and whether sending again may help.
    fn outcome(&self, status: TurnStatus, error: Option<&TurnError>) -> (PromptStatus, Option<String>, bool) {
        match status {
            TurnStatus::Interrupted => (PromptStatus::Aborted, None, false),
            TurnStatus::Failed if self.turn.stop_requested => (PromptStatus::Aborted, None, false),
            TurnStatus::Failed => {
                let failure = error
                    .filter(|error| !error.message.trim().is_empty())
                    .map(failure)
                    .or_else(|| self.turn.failure.clone())
                    .unwrap_or_else(|| Failure {
                        message: GENERIC_ERROR.to_owned(),
                        retryable: false,
                    });

                (PromptStatus::Error, Some(failure.message), failure.retryable)
            }
            _ if self.turn.is_quiet() => (PromptStatus::Handled, None, false),
            _ => (PromptStatus::Completed, None, false),
        }
    }

    /// Settles a turn that ended while the chat was not idle, once it is.
    pub(super) fn settle_if_idle(&mut self, out: &mut Out) {
        if self.turn.active || !self.pending.is_empty() {
            return;
        }

        if self.held && self.deferred == Some(false) {
            return;
        }

        if let Some(aborted) = self.deferred.take() {
            out.event(AgentEvent::Settled { aborted: Some(aborted) });
        }
    }

    pub(super) fn compaction_started(&mut self, out: &mut Out) {
        if self.turn.compaction.is_some() {
            return;
        }

        self.turn.compaction = Some(false);

        out.event(AgentEvent::Compaction {
            phase: "start".to_owned(),
            reason: Some(self.compaction_reason().to_owned()),
            error: None,
        });
    }

    pub(super) fn compaction_completed(&mut self, out: &mut Out) {
        self.compaction_started(out);
        self.compaction_ended(None, out);
    }

    fn compaction_ended(&mut self, error: Option<String>, out: &mut Out) {
        if !self.turn.is_compacting() {
            return;
        }

        self.turn.compaction = Some(true);

        out.event(AgentEvent::Compaction {
            phase: "end".to_owned(),
            reason: Some(self.compaction_reason().to_owned()),
            error,
        });
    }

    fn compaction_reason(&self) -> &'static str {
        match self.turn.kind {
            StartKind::Compact => "manual",
            _ => "auto",
        }
    }
}

/// What a turn error says: its first redacted line (or the sign-in error), and whether sending
/// again may help.
fn failure(error: &TurnError) -> Failure {
    let kind = error.kind().unwrap_or_default();

    if kind == UNAUTHORIZED {
        return Failure {
            message: SIGN_IN_ERROR.to_owned(),
            retryable: false,
        };
    }

    let line = redact_text(&error.message)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();

    Failure {
        message: if line.is_empty() {
            GENERIC_ERROR.to_owned()
        } else {
            line
        },
        retryable: RETRYABLE.contains(&kind),
    }
}

fn retry_ended(attempt: u32, message: Option<String>) -> AgentEvent {
    AgentEvent::Retry {
        phase: "end".to_owned(),
        attempt: Some(attempt),
        max_attempts: None,
        message,
    }
}
