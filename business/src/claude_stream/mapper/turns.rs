//! Turns, their results, compactions, retries and settling.
//!
//! - **Turns:** the first model request of a turn (`system/status requesting`, or the first reply
//!   frame) is `agentStart`; each API message is one model step (`turnStart` … `turnEnd`, the step
//!   ending when the next begins or the turn ends). The turn's `result` ends what is still open
//!   (tool calls fail as unfinished, replies end), then `agentEnd` and one `promptResult`:
//!   `aborted` after an interrupt (the `[Request interrupted by user]` frame, or a failed result
//!   with an `aborted_*` terminal reason or after [`ClaudeMapper::abort_requested`]), `error` for
//!   an `error_*` or `is_error` result, else `completed`.
//! - **Quiet results:** a `result` without a model turn (`num_turns: 0`: a `shouldQuery: false`
//!   message, `/compact`, a local command) shows no turn, only its `promptResult` (`handled`, or the
//!   error), and none at all for a message sent as [`SendKind::Quiet`].
//! - **Settling:** every sent message waits from [`ClaudeMapper::sent`] (or its `queued` lifecycle)
//!   until a result serves it, or it completes or is cancelled. A turn's end settles the chat
//!   (`settled`) once nothing waits; while a follow-up waits for its own turn, the settle waits for
//!   that turn (or the follow-up's withdrawal).
//! - **Compaction:** `system/status compacting` is `compaction` `start` (`manual` when the engine
//!   sent `/compact`), `compact_boundary` (or a failed `compact_result`, or the result) its `end`.
//! - **Retries:** `system/api_retry` is `retry` `start`, the next model step or the result its `end`.
//! - **Errors:** the CLI's sign-in error (an `authentication_failed` message, or a result that asks
//!   for `/login`) becomes [`SIGN_IN_ERROR`]: the app never signs in itself.

use std::collections::HashMap;

use super::notes::{first_line, retry_text};
use super::replies::frame_text;
use super::{ClaudeMapper, Hint, Out, SendKind};
use crate::claude_stream::wire::ResultFrame;
use crate::claude_stream::wire::messages::AssistantFrame;
use crate::claude_stream::wire::system::{ApiRetry, CompactMetadata, Status};
use crate::model::{AgentEvent, PromptStatus};

/// Shown when Claude Code has no login: the user signs in with Claude Code's own flow.
pub const SIGN_IN_ERROR: &str = "Claude Code is not signed in. Sign in from a terminal: run claude, then type /login.";

/// What the CLI's sign-in error asks the user to run.
const SIGN_IN_COMMAND: &str = "/login";

/// The API error kind of a missing or rejected login.
const AUTH_FAILED: &str = "authentication_failed";

/// API error kinds that may pass when the prompt is sent again.
const RETRYABLE_ERRORS: [&str; 3] = ["rate_limit", "overloaded", "server_error"];

/// Diagnostics the CLI lists among a result's errors.
const DIAGNOSTIC: &str = "[ede_diagnostic]";

/// The `reason` of a compaction the user asked for.
const MANUAL: &str = "manual";

/// A turn's `result`, for the engine (statistics, the prompts it served).
#[derive(Debug, Clone, PartialEq)]
pub struct TurnEnd {
    /// The sent messages the turn served.
    pub prompts: Vec<String>,
    /// True for a result without a model turn.
    pub quiet: bool,
    pub status: PromptStatus,
    pub result: Box<ResultFrame>,
}

/// The running turn (see the module docs).
#[derive(Default)]
pub(super) struct Turn {
    active: bool,
    /// The API message of the running model step.
    step: Option<String>,
    /// The CLI reported the turn interrupted.
    interrupted: bool,
    /// The user asked to stop it.
    stop_requested: bool,
    error: Option<TurnError>,
    /// The attempt of a running retry.
    retrying: Option<u32>,
    /// The compaction of the running turn or quiet run.
    compaction: Option<Compaction>,
    /// The last turn ended while messages waited: settle (aborted or not) once none waits.
    deferred: Option<bool>,
}

#[derive(Clone)]
struct TurnError {
    message: String,
    retryable: bool,
}

struct Compaction {
    manual: bool,
    ended: bool,
}

impl Turn {
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// True while a compaction runs.
    pub(super) fn is_compacting(&self) -> bool {
        self.compaction.as_ref().is_some_and(|compaction| !compaction.ended)
    }

    /// True while the running turn or quiet run compacted (its output is the compaction's).
    pub(super) fn is_compacting_run(&self) -> bool {
        self.compaction.is_some()
    }

    pub(super) fn ensure_started(&mut self, events: &mut Vec<AgentEvent>) {
        if self.active {
            return;
        }

        self.active = true;
        self.deferred = None;
        events.push(AgentEvent::AgentStart);
    }

    /// A frame of API message `id`: a new model step when it is not the running one.
    pub(super) fn step(&mut self, id: &str, events: &mut Vec<AgentEvent>) {
        if id.is_empty() || self.step.as_deref() == Some(id) {
            return;
        }

        if let Some(attempt) = self.retrying.take() {
            events.push(retry_ended(attempt, None));
        }

        if self.step.is_some() {
            events.push(AgentEvent::TurnEnd);
        }

        self.step = Some(id.to_owned());
        events.push(AgentEvent::TurnStart);
    }

    pub(super) fn abort_requested(&mut self) {
        self.stop_requested = true;
    }

    pub(super) fn interrupted(&mut self) {
        self.interrupted = true;
    }

    /// A synthetic message reporting an API error: the turn's first error.
    pub(super) fn api_error(&mut self, frame: &AssistantFrame) {
        let text = first_line(&frame_text(frame));
        let kind = frame.error.as_deref().unwrap_or_default();

        let error = if kind == AUTH_FAILED || text.contains(SIGN_IN_COMMAND) {
            TurnError {
                message: SIGN_IN_ERROR.to_owned(),
                retryable: false,
            }
        } else {
            TurnError {
                message: if text.is_empty() { generic_error(kind) } else { text },
                retryable: RETRYABLE_ERRORS.contains(&kind),
            }
        };

        self.error.get_or_insert(error);
    }
}

/// The user messages the engine sent and the CLI has not finished with.
#[derive(Default)]
pub(super) struct Sends {
    /// How each was sent, until a result serves it.
    kinds: HashMap<String, SendKind>,
    /// Those still waiting, in order.
    waiting: Vec<String>,
}

impl Sends {
    pub(super) fn sent(&mut self, uuid: &str, kind: SendKind) {
        self.kinds.insert(uuid.to_owned(), kind);
        self.queued(uuid);
    }

    pub(super) fn queued(&mut self, uuid: &str) {
        if !uuid.is_empty() && !self.waiting.iter().any(|waiting| waiting == uuid) {
            self.waiting.push(uuid.to_owned());
        }
    }

    pub(super) fn done(&mut self, uuid: &str) {
        self.waiting.retain(|waiting| waiting != uuid);
    }

    pub(super) fn waiting(&self) -> bool {
        !self.waiting.is_empty()
    }

    fn compact_sent(&self) -> bool {
        self.kinds.values().any(|kind| *kind == SendKind::Compact)
    }

    /// `prompts` were served: how each was sent (when the engine said).
    fn served(&mut self, prompts: &[String]) -> Vec<SendKind> {
        prompts
            .iter()
            .filter_map(|uuid| {
                self.done(uuid);
                self.kinds.remove(uuid)
            })
            .collect()
    }
}

impl ClaudeMapper {
    /// A `result`: the turn (or quiet run) ends.
    pub(super) fn result(&mut self, result: &ResultFrame, out: &mut Out) {
        let prompts = result.prompts();
        let kinds = self.sends.served(&prompts);
        let quiet = !self.turn.active;
        let (status, error, retryable) = self.outcome(result, quiet);

        if self.turn.is_compacting() {
            self.compaction_ended(None, error.clone(), out);
        }

        if let Some(attempt) = self.turn.retrying.take() {
            out.event(retry_ended(attempt, error.clone()));
        }

        if !quiet {
            self.tools.end_unfinished(&mut out.events);
            self.replies
                .end_all(status == PromptStatus::Aborted, &self.tools, &mut out.events);
            self.tools.forget();

            if self.turn.step.is_some() {
                out.event(AgentEvent::TurnEnd);
            }

            out.event(AgentEvent::AgentEnd);
        }

        // The output of a quiet message is the engine's to show; only its failure is the chat's.
        let quiet_send = quiet && !kinds.is_empty() && kinds.iter().all(|kind| *kind == SendKind::Quiet);
        let shown = !quiet_send || status == PromptStatus::Error;

        if shown {
            out.event(AgentEvent::PromptResult {
                request_id: result.user_message_uuid.clone(),
                status,
                error,
                retryable,
            });
        }

        out.hint(Hint::TurnEnded(TurnEnd {
            prompts,
            quiet,
            status,
            result: Box::new(result.clone()),
        }));

        let deferred = self.turn.deferred.take();
        self.turn = Turn::default();

        if quiet {
            self.turn.deferred = deferred;
        } else {
            self.turn.deferred = Some(status == PromptStatus::Aborted);
        }

        self.settle_if_idle(out);
    }

    /// Settles a turn that ended while messages waited, once none waits.
    pub(super) fn settle_if_idle(&mut self, out: &mut Out) {
        if self.turn.active || self.sends.waiting() {
            return;
        }

        if let Some(aborted) = self.turn.deferred.take() {
            out.event(AgentEvent::Settled { aborted: Some(aborted) });
        }
    }

    /// How the turn ended: its prompt status, error and whether sending again may help.
    fn outcome(&self, result: &ResultFrame, quiet: bool) -> (PromptStatus, Option<String>, bool) {
        let aborted_end = result
            .terminal_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("aborted"));
        let stopped = !result.succeeded() && (aborted_end || self.turn.stop_requested);

        if self.turn.interrupted || stopped {
            return (PromptStatus::Aborted, None, false);
        }

        if !result.succeeded() {
            let error = self.failure(result);
            return (PromptStatus::Error, Some(error.message), error.retryable);
        }

        let status = if quiet {
            PromptStatus::Handled
        } else {
            PromptStatus::Completed
        };

        (status, None, false)
    }

    /// The error a failed `result` reports.
    fn failure(&self, result: &ResultFrame) -> TurnError {
        let text = result.result.as_deref().map(first_line).unwrap_or_default();

        if text.contains(SIGN_IN_COMMAND) {
            return TurnError {
                message: SIGN_IN_ERROR.to_owned(),
                retryable: false,
            };
        }

        if let Some(error) = &self.turn.error {
            return error.clone();
        }

        let listed = result
            .errors
            .iter()
            .map(|error| first_line(error))
            .find(|line| !line.is_empty() && !line.starts_with(DIAGNOSTIC));

        let message = listed
            .or_else(|| (!text.is_empty()).then_some(text))
            .unwrap_or_else(|| generic_error(&result.subtype));
        let retryable = matches!(result.api_error_status, Some(429 | 500..=599));

        TurnError { message, retryable }
    }

    /// `system/status`.
    pub(super) fn status(&mut self, status: &Status, out: &mut Out) {
        match status.status.as_deref() {
            Some("requesting") => {
                self.turn.ensure_started(&mut out.events);

                if !status.user_message_uuids.is_empty() {
                    out.hint(Hint::PromptsTaken(status.user_message_uuids.clone()));
                }
            }
            Some("compacting") => self.compaction_started(out),
            _ => {}
        }

        if status.compact_result.as_deref() == Some("failed") {
            let error = status
                .compact_error
                .as_deref()
                .map(first_line)
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "The compaction failed.".to_owned());
            self.compaction_ended(None, Some(error), out);
        }

        if let Some(mode) = &status.permission_mode {
            out.hint(Hint::PermissionMode(mode.clone()));
        }
    }

    /// `system/compact_boundary`: the compaction ends (it starts too when no status announced it).
    pub(super) fn compact_boundary(&mut self, metadata: &CompactMetadata, out: &mut Out) {
        if self.turn.compaction.is_none() {
            self.compaction_started(out);
        }

        self.compaction_ended(metadata.trigger.clone(), None, out);
        out.hint(Hint::Compacted(metadata.clone()));
    }

    fn compaction_started(&mut self, out: &mut Out) {
        if self.turn.is_compacting() {
            return;
        }

        let manual = self.sends.compact_sent();
        self.turn.compaction = Some(Compaction { manual, ended: false });

        out.event(AgentEvent::Compaction {
            phase: "start".to_owned(),
            reason: manual.then(|| MANUAL.to_owned()),
            error: None,
        });
    }

    fn compaction_ended(&mut self, reason: Option<String>, error: Option<String>, out: &mut Out) {
        let Some(compaction) = self.turn.compaction.as_mut().filter(|compaction| !compaction.ended) else {
            return;
        };

        compaction.ended = true;
        let reason = reason.or_else(|| compaction.manual.then(|| MANUAL.to_owned()));

        out.event(AgentEvent::Compaction {
            phase: "end".to_owned(),
            reason,
            error,
        });
    }

    /// `system/api_retry`.
    pub(super) fn retry_started(&mut self, retry: &ApiRetry, out: &mut Out) {
        self.turn.ensure_started(&mut out.events);
        self.turn.retrying = Some(retry.attempt.unwrap_or(1));

        out.event(AgentEvent::Retry {
            phase: "start".to_owned(),
            attempt: retry.attempt,
            max_attempts: retry.max_retries,
            message: retry_text(retry),
        });
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

/// The error shown when the CLI names none, by error kind or result subtype.
fn generic_error(kind: &str) -> String {
    let message = match kind {
        "error_max_turns" => "Claude Code stopped after the most turns it may take.",
        "error_max_budget_usd" => "Claude Code stopped at the session's budget limit.",
        "rate_limit" => "Claude Code hit a rate limit.",
        "overloaded" => "The model is overloaded. Try again in a moment.",
        _ => "Claude Code reported an error.",
    };

    message.to_owned()
}
