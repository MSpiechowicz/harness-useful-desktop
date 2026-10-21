//! Turns of the chat's session, the steering sent into them and the follow-ups held for after
//! them, as the OpenCode 1 mapper shows them.
//!
//! - **Turns:** `session.execution.started` (or a `busy` / `retry` status) starts a turn
//!   (`agentStart`, preceded by [`Mapped::PromptTaken`] while a prompt is on its way); the
//!   execution's end (`succeeded`, `failed`, `interrupted`, or an `idle` status) ends it: what is
//!   still open ends, then `agentEnd` and a `promptResult` for each prompt of the turn (the first
//!   error of the execution or of a step; an interruption makes it `aborted`).
//! - **Settling:** after a turn the next held follow-up is sent ([`Mapped::Send`], its own prompt
//!   with its own result), and only an empty queue settles the chat (`settled`), so `settled`
//!   still means "the prompt and everything queued behind it are done". An aborted turn settles
//!   at once and keeps the held follow-ups for the user.
//! - **Steering** is sent at once (`delivery: steer`) and listed in `queueUpdate` (never
//!   withdrawable) until the run takes it (`session.inbox.delivered`). OpenCode admits it as an
//!   inbox item: the item is matched to the steering by its id ([`Mapper::steer_accepted`], or an
//!   id the app chose) or else by its text. Steering the run never took is dropped from the list
//!   when the turn ends.
//! - **Abort** before the turn started is remembered, and the turn's start asks OpenCode to
//!   interrupt again ([`Mapped::Abort`]).
//! - **Retries:** `session.retry.scheduled` (or a `retry` status) reports `retry` `start` with the
//!   attempt and reason, the next step (or `busy`, or the end of the turn) `retry` `end`.
//! - **Errors:** a failed execution outside a turn becomes an error notice, unless it repeats the
//!   error the last turn ended with.

use super::{Mapped, Mapper, error_message, events, first_line};
use crate::model::{AgentEvent, NoticeLevel, PromptStatus};
use crate::opencode_server::wire::Outgoing;
use crate::opencode_server_v2::events::{InboxEnqueued, RetryScheduled, SessionStatus};
use crate::opencode_server_v2::wire::ErrorInfo;
use crate::security::redact_text;

/// The error kind of an interruption.
const ABORTED: &str = "aborted";

/// The running turn.
#[derive(Default)]
pub(super) struct Turn {
    /// `agentStart` was emitted.
    active: bool,
    /// Prompts sent and not yet answered by a `promptResult`.
    prompts: u32,
    /// The first error of the turn.
    error: Option<String>,
    aborted: bool,
    /// The attempt of a running retry.
    retrying: Option<u32>,
    /// The user aborted before the turn started (see [`Mapper::abort_requested`]).
    abort_pending: bool,
}

/// How an execution ended.
pub(super) enum Outcome {
    Succeeded,
    Failed(ErrorInfo),
    Interrupted,
}

impl Mapper {
    /// True while a turn or a quiet run runs or a prompt is on its way.
    pub fn is_working(&self) -> bool {
        self.turn.active || self.turn.prompts > 0 || self.quiet.is_some()
    }

    /// False while a quiet run lasts: steering then waits like a follow-up.
    pub fn can_steer(&self) -> bool {
        self.quiet.is_none()
    }

    /// A prompt is about to be sent.
    pub fn prompt_sent(&mut self) {
        self.turn.prompts += 1;
    }

    /// The prompt announced by [`Mapper::prompt_sent`] was not delivered; its sender reports why.
    pub fn prompt_failed(&mut self) {
        self.turn.prompts = self.turn.prompts.saturating_sub(1);

        if self.turn.prompts == 0 {
            self.turn.abort_pending = false;
        }
    }

    /// A prompt sent in the background (a held follow-up, a server command) failed with `error`.
    /// Once a turn runs, the turn reports its own outcome; before that the prompt fails on its
    /// own, and the chat moves on to the next follow-up or settles. `taken` when the agent took
    /// the prompt before it failed: the prompt-taken call runs first.
    pub fn prompt_failed_late(&mut self, error: &str, taken: bool) -> Vec<Mapped> {
        if self.turn.active || self.turn.prompts == 0 {
            return Vec::new();
        }

        self.prompt_failed();

        let mut mapped = Vec::new();

        if taken {
            mapped.push(Mapped::PromptTaken);
        }

        mapped.push(Mapped::event(AgentEvent::PromptResult {
            request_id: None,
            status: PromptStatus::Error,
            error: Some(redact_text(error)),
            retryable: false,
        }));

        if self.turn.prompts == 0 {
            mapped.extend(self.after_turn(false));
        }

        mapped
    }

    /// Steering `message` is about to be sent into the running turn.
    pub fn steer_sent(&mut self, message: Outgoing) -> Vec<AgentEvent> {
        self.steering.push(message);
        vec![self.queue_update()]
    }

    /// OpenCode admitted steering with `text` as inbox item `id` (the prompt's answer).
    pub fn steer_accepted(&mut self, text: &str, id: &str) {
        self.bind_steering(id, text);
    }

    /// Steering with `text` was not delivered.
    pub fn steer_failed(&mut self, text: &str) -> Vec<AgentEvent> {
        let position = self
            .steering
            .iter()
            .position(|steering| steering.id.is_none() && steering.text == text);

        if let Some(position) = position {
            self.steering.remove(position);
        }

        vec![self.queue_update()]
    }

    /// Holds `message` until the chat is idle.
    pub fn queue_follow_up(&mut self, message: Outgoing) -> Vec<AgentEvent> {
        self.follow_ups.push_back(message);
        vec![self.queue_update()]
    }

    /// True while follow-ups are held.
    pub fn has_follow_ups(&self) -> bool {
        !self.follow_ups.is_empty()
    }

    /// Withdraws the first held follow-up with `text`; `None` when none is held.
    pub fn remove_follow_up(&mut self, text: &str) -> Option<Vec<AgentEvent>> {
        let position = self.follow_ups.iter().position(|held| held.text == text)?;
        self.follow_ups.remove(position);

        Some(vec![self.queue_update()])
    }

    /// Takes the first held follow-up to send it now (the chat is idle); it counts as a prompt on
    /// its way.
    pub fn next_follow_up(&mut self) -> Option<(Outgoing, Vec<AgentEvent>)> {
        let next = self.follow_ups.pop_front()?;
        self.turn.prompts += 1;

        Some((next, vec![self.queue_update()]))
    }

    /// The user asked to abort the running turn: it ends as aborted. Before the turn started (a
    /// prompt on its way) the abort is remembered, and the turn's start asks for it again
    /// ([`Mapped::Abort`]).
    pub fn abort_requested(&mut self) {
        if self.turn.active {
            self.turn.aborted = true;
        } else if self.turn.prompts > 0 {
            self.turn.abort_pending = true;
        }
    }

    /// The run started (`session.execution.started`, a `busy` or `retry` status).
    pub(super) fn turn_started(&mut self, out: Vec<AgentEvent>) -> Vec<Mapped> {
        let mut mapped: Vec<Mapped> = events(out).collect();

        if self.turn.active {
            return mapped;
        }

        self.turn.active = true;
        self.last_error = None;

        if self.turn.prompts > 0 {
            mapped.push(Mapped::PromptTaken);
        }

        mapped.push(Mapped::event(AgentEvent::AgentStart));

        if std::mem::take(&mut self.turn.abort_pending) {
            self.turn.aborted = true;
            mapped.push(Mapped::Abort);
        }

        mapped
    }

    /// A `session.status` (or `session.idle`) of the chat's session.
    pub(super) fn status(&mut self, status: &SessionStatus, mut out: Vec<AgentEvent>) -> Vec<Mapped> {
        match status {
            SessionStatus::Busy => {
                self.retry_settled(&mut out);
                self.turn_started(out)
            }
            SessionStatus::Retry { attempt, message, .. } => {
                let mut mapped = self.turn_started(out);
                let attempt = u32::try_from(*attempt).ok().filter(|attempt| *attempt > 0);
                mapped.push(Mapped::event(self.retry_started(attempt, message)));
                mapped
            }
            SessionStatus::Idle if self.turn.active => {
                let mut mapped: Vec<Mapped> = events(out).collect();
                mapped.extend(self.ended_idle());
                mapped
            }
            _ => events(out).collect(),
        }
    }

    /// The run ended with `outcome`.
    pub(super) fn execution_ended(&mut self, outcome: Outcome) -> Vec<Mapped> {
        if !self.turn.active {
            return self.failed_outside_turn(&outcome);
        }

        match outcome {
            Outcome::Succeeded => {}
            Outcome::Interrupted => self.turn.aborted = true,
            Outcome::Failed(error) if error.kind == ABORTED => self.turn.aborted = true,
            Outcome::Failed(error) => self.note_turn_error(error_message(&error.message)),
        }

        self.ended_idle()
    }

    /// `session.retry.scheduled`: the model call is tried again.
    pub(super) fn retry_scheduled(&mut self, retry: &RetryScheduled) -> AgentEvent {
        let attempt = u32::try_from(retry.attempt).ok().filter(|attempt| *attempt > 0);

        self.retry_started(attempt, &retry.error.message)
    }

    /// A running retry is over (a step started): reports its `end`.
    pub(super) fn retry_settled(&mut self, out: &mut Vec<AgentEvent>) {
        if let Some(attempt) = self.turn.retrying.take() {
            out.push(retry_ended(attempt, None));
        }
    }

    /// A failed step fails its turn; an interrupted one aborts it.
    pub(super) fn note_step_error(&mut self, error: &ErrorInfo) {
        if !self.turn.active {
            return;
        }

        if error.kind == ABORTED {
            self.turn.aborted = true;
        } else {
            self.note_turn_error(error_message(&error.message));
        }
    }

    /// `session.inbox.enqueued`: a prompt is admitted. A user item binds to the steering it is.
    pub(super) fn inbox_enqueued(&mut self, enqueued: &InboxEnqueued, created: Option<u64>) {
        if enqueued.item.kind == "user" {
            let text = enqueued.item.payload.text.as_deref().unwrap_or_default();
            self.bind_steering(&enqueued.inbox_id, text);
        }

        self.inbox
            .insert(enqueued.inbox_id.clone(), (enqueued.clone(), created));
    }

    /// `session.inbox.delivered`: the run took item `id`. Steering leaves the queue, and a
    /// prompt shows as the user's message.
    pub(super) fn inbox_delivered(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let item = self.inbox.remove(id);
        self.unlist_steering(id, out);

        if let Some((enqueued, created)) = item.filter(|(enqueued, _)| enqueued.item.kind == "user") {
            self.replies.user_delivered(&enqueued, created, out);
        }
    }

    /// `session.inbox.cancelled`: item `id` will not run.
    pub(super) fn inbox_cancelled(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        self.inbox.remove(id);
        self.unlist_steering(id, out);
    }

    /// Binds inbox item `id` with `text` to the first steering waiting for its id.
    fn bind_steering(&mut self, id: &str, text: &str) {
        if self.steering.iter().any(|steering| steering.id.as_deref() == Some(id)) {
            return;
        }

        let waiting = self
            .steering
            .iter_mut()
            .find(|steering| steering.id.is_none() && steering.text == text);

        if let Some(steering) = waiting {
            steering.id = Some(id.to_owned());
        }
    }

    /// Takes the steering of inbox item `id` off the queue.
    fn unlist_steering(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let position = self
            .steering
            .iter()
            .position(|steering| steering.id.as_deref() == Some(id));

        if let Some(position) = position {
            self.steering.remove(position);
            out.push(self.queue_update());
        }
    }

    /// The run is over: steering it never took leaves the queue, then the turn ends.
    fn ended_idle(&mut self) -> Vec<Mapped> {
        let mut out = Vec::new();

        if !self.steering.is_empty() {
            self.steering.clear();
            out.push(self.queue_update());
        }

        let after = self.end_turn(&mut out);

        events(out).chain(after).collect()
    }

    /// A failed execution the chat did not see start: an error notice, unless it repeats the
    /// last turn's error or reports an interruption.
    fn failed_outside_turn(&mut self, outcome: &Outcome) -> Vec<Mapped> {
        let Outcome::Failed(error) = outcome else {
            return Vec::new();
        };

        if error.kind == ABORTED {
            return Vec::new();
        }

        let message = error_message(&error.message);
        let repeated = self.last_error.as_deref().is_some_and(|last| message.contains(last));

        if repeated {
            self.last_error = None;
            return Vec::new();
        }

        vec![Mapped::event(AgentEvent::Notice {
            level: NoticeLevel::Error,
            message,
        })]
    }

    /// Ends the turn (see the module docs); what follows the turn's events.
    fn end_turn(&mut self, out: &mut Vec<AgentEvent>) -> Vec<Mapped> {
        let turn = std::mem::take(&mut self.turn);
        self.close_asks(out);
        self.tools.end_unfinished(out);
        self.replies.end_all(turn.aborted, &self.tools, out);
        self.tools.forget();

        if let Some(attempt) = turn.retrying {
            out.push(retry_ended(attempt, turn.error.clone()));
        }

        out.push(AgentEvent::AgentEnd);

        let (status, error) = match (&turn.error, turn.aborted) {
            (_, true) => (PromptStatus::Aborted, None),
            (Some(error), false) => (PromptStatus::Error, Some(error.clone())),
            (None, false) => (PromptStatus::Completed, None),
        };

        for _ in 0..turn.prompts {
            out.push(AgentEvent::PromptResult {
                request_id: None,
                status,
                error: error.clone(),
                retryable: false,
            });
        }

        self.last_error = turn.error;
        self.after_turn(turn.aborted)
    }

    /// Sends the next held follow-up, or settles.
    fn after_turn(&mut self, aborted: bool) -> Vec<Mapped> {
        if !aborted && let Some((next, update)) = self.next_follow_up() {
            return events(update).chain([Mapped::Send(next)]).collect();
        }

        vec![Mapped::event(AgentEvent::Settled { aborted: Some(aborted) })]
    }

    fn retry_started(&mut self, attempt: Option<u32>, message: &str) -> AgentEvent {
        self.turn.retrying = Some(attempt.unwrap_or(1));
        let message = first_line(&redact_text(message));

        AgentEvent::Retry {
            phase: "start".to_owned(),
            attempt,
            max_attempts: None,
            message: (!message.is_empty()).then_some(message),
        }
    }

    /// Keeps the first error of the running turn.
    fn note_turn_error(&mut self, message: String) {
        self.turn.error.get_or_insert(message);
    }

    pub(super) fn queue_update(&self) -> AgentEvent {
        let texts =
            |messages: &mut dyn Iterator<Item = &Outgoing>| messages.map(|message| message.text.clone()).collect();

        AgentEvent::QueueUpdate {
            steering: texts(&mut self.steering.iter()),
            follow_up: texts(&mut self.follow_ups.iter()),
            live_steered: u32::try_from(self.steering.len()).unwrap_or(u32::MAX),
        }
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
