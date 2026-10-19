//! Turns of the chat's session, the steering sent into them and the follow-ups held for after
//! them.
//!
//! - **Turns:** `busy` (or `retry`) starts a turn (`agentStart`, preceded by
//!   [`Mapped::PromptTaken`] while a prompt is on its way); `idle` ends it: what is still open
//!   ends, then `agentEnd` and a `promptResult` for each prompt of the turn (the first
//!   `session.error`, or an assistant message's `error`; an abort makes it `aborted`).
//! - **Settling:** after a turn the next held follow-up is sent ([`Mapped::Send`], its own prompt
//!   with its own result), and only an empty queue settles the chat (`settled`), so `settled`
//!   still means "the prompt and everything queued behind it are done". An aborted turn settles
//!   at once and keeps the held follow-ups for the user, like OMP.
//! - **Steering** is sent at once and listed in `queueUpdate` (never withdrawable) until a reply
//!   to it appears (an assistant message whose `parentID` is the steering's id). A turn that goes
//!   idle with steering unanswered first asks the engine whether OpenCode left it without a reply
//!   ([`Mapped::CheckStranded`], once every steering message reached OpenCode); each steering
//!   message is sent again at most once.
//! - **Abort** before the turn started is remembered, and the turn's start asks OpenCode to abort
//!   again ([`Mapped::Abort`]): an abort that reached OpenCode before its run started does
//!   nothing there.
//! - **Retries:** a `retry` status reports `retry` `start` with the attempt and reason, the next
//!   `busy` or the end of the turn `retry` `end`.
//! - **Errors:** a `session.error` outside a turn becomes an error notice, unless it repeats the
//!   error the last turn ended with (OpenCode reports some errors twice) or reports an abort.

use serde_json::Value;

use super::message::{error_text, is_abort};
use super::{Mapped, Mapper, error_message, events, first_line};
use crate::model::{AgentEvent, NoticeLevel, PromptStatus};
use crate::opencode_server::wire::Outgoing;
use crate::security::redact_text;

/// The running turn.
#[derive(Default)]
pub(super) struct Turn {
    /// `agentStart` was emitted.
    active: bool,
    /// Prompts sent and not yet answered by a `promptResult`.
    prompts: u32,
    error: Option<TurnError>,
    aborted: bool,
    /// The attempt of a running retry.
    retrying: Option<u32>,
    /// The user aborted before the turn started (see [`Mapper::abort_requested`]).
    abort_pending: bool,
}

struct TurnError {
    message: String,
    retryable: bool,
}

impl Mapper {
    /// True while a turn or a quiet run runs, a prompt is on its way or a turn's end is being
    /// checked.
    pub fn is_working(&self) -> bool {
        self.turn.active || self.turn.prompts > 0 || self.checking || self.quiet.is_some()
    }

    /// False while a quiet run lasts or the end of a turn is being checked: steering then waits
    /// like a follow-up.
    pub fn can_steer(&self) -> bool {
        self.quiet.is_none() && !self.checking
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

    /// A prompt sent in the background (a queued follow-up, a server command) failed with
    /// `error`. Once a turn runs, the turn reports its own outcome; before that the prompt fails
    /// on its own, and the chat moves on to the next follow-up or settles. `taken` when the agent
    /// took the prompt before it failed (a server command OpenCode answered): the prompt-taken
    /// call runs first.
    pub fn prompt_failed_late(&mut self, error: &str, taken: bool) -> Vec<Mapped> {
        if self.turn.active || self.turn.prompts == 0 {
            return Vec::new();
        }

        self.turn.prompts -= 1;

        if self.turn.prompts == 0 {
            self.turn.abort_pending = false;
        }

        let mut mapped = Vec::new();

        if taken {
            mapped.push(Mapped::PromptTaken);
        }

        mapped.push(Mapped::Event(AgentEvent::PromptResult {
            request_id: None,
            status: PromptStatus::Error,
            error: Some(redact_text(error)),
            retryable: false,
        }));

        if self.turn.prompts == 0 && !self.checking {
            mapped.extend(self.after_turn(false));
        }

        mapped
    }

    /// Steering `message` (named by the app) was sent into the running turn.
    pub fn steer_sent(&mut self, message: Outgoing) -> Vec<AgentEvent> {
        self.undelivered.extend(message.id.clone());
        self.steering.push(message);
        vec![self.queue_update()]
    }

    /// OpenCode took steering `id` (`204`).
    pub fn steer_delivered(&mut self, id: &str) -> Vec<Mapped> {
        self.undelivered.remove(id);
        self.due_check()
    }

    /// Steering `id` was not delivered.
    pub fn steer_failed(&mut self, id: &str) -> Vec<Mapped> {
        self.undelivered.remove(id);
        self.steering.retain(|steering| steering.id.as_deref() != Some(id));

        let update = Mapped::Event(self.queue_update());
        std::iter::once(update).chain(self.due_check()).collect()
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

    /// The user asked to abort the running turn: it ends as aborted, and no steering is sent
    /// again for it. Before the turn started (a prompt on its way, or taken but not yet running)
    /// the abort is remembered: OpenCode may get it before its run starts, so the turn's start
    /// asks for it again ([`Mapped::Abort`]).
    pub fn abort_requested(&mut self) {
        if self.turn.active {
            self.turn.aborted = true;
        } else if self.turn.prompts > 0 {
            self.turn.abort_pending = true;
        }
    }

    /// One answer to [`Mapped::CheckStranded`]: OpenCode left steering `id` without a reply. True
    /// when it may be sent again (the turn goes on); false ends the check like
    /// [`Mapper::stranded_checked`] must then.
    pub fn resend_stranded(&mut self, id: &str) -> bool {
        if !self.checking || self.turn.aborted || self.checked.contains(id) {
            return false;
        }

        self.checking = false;
        self.checked.insert(id.to_owned());

        true
    }

    /// The other answer to [`Mapped::CheckStranded`]: nothing is sent again, the turn ends.
    pub fn stranded_checked(&mut self) -> Vec<Mapped> {
        if !self.checking {
            return Vec::new();
        }

        self.checking = false;

        let checked = self.steering.iter().filter_map(|steering| steering.id.clone());
        self.checked.extend(checked);

        self.ended_idle()
    }

    /// Sending stranded steering again failed: OpenCode stays idle, so the turn ends now.
    pub fn resend_failed(&mut self) -> Vec<Mapped> {
        if self.checking || !self.turn.active {
            return Vec::new();
        }

        self.ended_idle()
    }

    fn ended_idle(&mut self) -> Vec<Mapped> {
        let mut out = Vec::new();
        let after = self.idle(&mut out);

        events(out).chain(after).collect()
    }

    pub(super) fn status(&mut self, properties: &Value, mut out: Vec<AgentEvent>) -> Vec<Mapped> {
        let status = properties.pointer("/status/type").and_then(Value::as_str);
        let mut mapped: Vec<Mapped> = Vec::new();

        match status {
            Some(kind @ ("busy" | "retry")) => {
                if !self.turn.active {
                    self.turn.active = true;
                    self.last_error = None;

                    mapped.extend(events(std::mem::take(&mut out)));
                    if self.turn.prompts > 0 {
                        mapped.push(Mapped::PromptTaken);
                    }

                    out.push(AgentEvent::AgentStart);

                    if std::mem::take(&mut self.turn.abort_pending) {
                        self.turn.aborted = true;
                        mapped.extend(events(std::mem::take(&mut out)));
                        mapped.push(Mapped::Abort);
                    }
                }

                if kind == "retry" {
                    out.push(self.retry_started(properties));
                } else if let Some(attempt) = self.turn.retrying.take() {
                    out.push(retry_ended(attempt, None));
                }
            }
            Some("idle") if self.turn.active && !self.checking => {
                let after = self.idle(&mut out);
                mapped.extend(events(out));
                mapped.extend(after);
                return mapped;
            }
            _ => {}
        }

        mapped.extend(events(out));
        mapped
    }

    fn retry_started(&mut self, properties: &Value) -> AgentEvent {
        let status = properties.get("status").unwrap_or(&Value::Null);
        let attempt = status
            .get("attempt")
            .and_then(Value::as_u64)
            .and_then(|attempt| u32::try_from(attempt).ok());
        self.turn.retrying = Some(attempt.unwrap_or(1));

        AgentEvent::Retry {
            phase: "start".to_owned(),
            attempt,
            max_attempts: None,
            message: status
                .get("message")
                .and_then(Value::as_str)
                .map(|message| first_line(&redact_text(message))),
        }
    }

    /// The turn went idle: ends it, or first has the unanswered steering checked.
    fn idle(&mut self, out: &mut Vec<AgentEvent>) -> Vec<Mapped> {
        let unchecked = self.unchecked();

        if !self.turn.aborted && !unchecked.is_empty() {
            self.checking = true;

            // Steering still on its way may not have reached OpenCode yet: check once it has.
            if !self.undelivered.is_empty() {
                self.check_due = true;
                return Vec::new();
            }

            return vec![Mapped::CheckStranded(unchecked)];
        }

        if !self.steering.is_empty() {
            self.steering.clear();
            out.push(self.queue_update());
        }

        self.end_turn(out)
    }

    /// Steering no stranded check covered yet.
    fn unchecked(&self) -> Vec<Outgoing> {
        self.steering
            .iter()
            .filter(|steering| steering.id.as_ref().is_some_and(|id| !self.checked.contains(id)))
            .cloned()
            .collect()
    }

    /// The check an idle turn put off until its steering was delivered, once it was.
    fn due_check(&mut self) -> Vec<Mapped> {
        if !self.check_due || !self.undelivered.is_empty() {
            return Vec::new();
        }

        self.check_due = false;
        let unchecked = self.unchecked();

        if unchecked.is_empty() {
            self.checking = false;
            return self.ended_idle();
        }

        vec![Mapped::CheckStranded(unchecked)]
    }

    /// Ends the turn (see the module docs); what follows the turn's events.
    fn end_turn(&mut self, out: &mut Vec<AgentEvent>) -> Vec<Mapped> {
        let turn = std::mem::take(&mut self.turn);
        self.close_asks(out);
        self.messages.end_all(turn.aborted, out);

        if let Some(attempt) = turn.retrying {
            out.push(retry_ended(
                attempt,
                turn.error.as_ref().map(|error| error.message.clone()),
            ));
        }

        out.push(AgentEvent::AgentEnd);

        let (status, error, retryable) = match (&turn.error, turn.aborted) {
            (_, true) => (PromptStatus::Aborted, None, false),
            (Some(error), false) => (PromptStatus::Error, Some(error.message.clone()), error.retryable),
            (None, false) => (PromptStatus::Completed, None, false),
        };

        for _ in 0..turn.prompts {
            out.push(AgentEvent::PromptResult {
                request_id: None,
                status,
                error: error.clone(),
                retryable,
            });
        }

        self.last_error = turn.error.map(|error| error.message);
        self.after_turn(turn.aborted)
    }

    /// Sends the next held follow-up, or settles.
    fn after_turn(&mut self, aborted: bool) -> Vec<Mapped> {
        if !aborted && let Some((next, update)) = self.next_follow_up() {
            return events(update).chain([Mapped::Send(next)]).collect();
        }

        vec![Mapped::Event(AgentEvent::Settled { aborted: Some(aborted) })]
    }

    pub(super) fn error(&mut self, properties: &Value, out: &mut Vec<AgentEvent>) {
        let error = properties.get("error").unwrap_or(&Value::Null);

        if is_abort(error) {
            self.turn.aborted |= self.turn.active;
            return;
        }

        let message = error_message(error);

        if self.turn.active {
            self.note_turn_error(message, error);
            return;
        }

        let repeated = self.last_error.as_deref().is_some_and(|last| message.contains(last));

        if repeated {
            self.last_error = None;
            return;
        }

        out.push(AgentEvent::Notice {
            level: NoticeLevel::Error,
            message,
        });
    }

    /// A failed assistant message fails its turn.
    pub(super) fn note_message_error(&mut self, info: &Value) {
        let Some(error) = info.get("error").filter(|_| self.turn.active) else {
            return;
        };

        if is_abort(error) {
            self.turn.aborted = true;
        } else if let Some(message) = error_text(info) {
            self.note_turn_error(first_line(&message), error);
        }
    }

    /// A reply to steering ends its time in the queue (and that of the steering before it).
    pub(super) fn note_reply(&mut self, info: &Value, out: &mut Vec<AgentEvent>) {
        let parent = info
            .get("parentID")
            .and_then(Value::as_str)
            .filter(|_| info.get("role").and_then(Value::as_str) == Some("assistant"));

        let Some(parent) = parent else {
            return;
        };

        let answered = self
            .steering
            .iter()
            .position(|steering| steering.id.as_deref() == Some(parent));

        if let Some(answered) = answered {
            self.steering.drain(..=answered);
            out.push(self.queue_update());
        }
    }

    /// Keeps the first error of the running turn.
    fn note_turn_error(&mut self, message: String, error: &Value) {
        if self.turn.error.is_some() {
            return;
        }

        let retryable = error
            .pointer("/data/isRetryable")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        self.turn.error = Some(TurnError { message, retryable });
    }

    fn queue_update(&self) -> AgentEvent {
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
