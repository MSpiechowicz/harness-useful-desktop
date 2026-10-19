//! Quiet runs: work the app starts in the chat's session that is not a turn of the chat, a
//! `!command` (`POST /session/{id}/shell`) or a compaction (`POST /session/{id}/summarize`).
//!
//! - OpenCode runs both like a turn (`busy`, a message pair, `idle`), but the chat shows them its
//!   own way, as OMP does: the command's output comes with the reply, a compaction as
//!   `compaction` `start` and `end`. So while a quiet run lasts, the session's status, message and
//!   error events are not shown, and the chat counts as working: prompts wait as follow-ups.
//! - The run ends once its call answered and its `idle` arrived, in either order (the event stream
//!   and the call's answer race). A call OpenCode refused (`409`) or that never reached it ends
//!   the run at once; a call answered without an `idle` ends with [`Mapper::quiet_expire`].
//! - At the end a compaction reports `end` (with the call's error, else the first error of the
//!   run), then the next held follow-up is sent.

use serde_json::Value;

use super::message::error_text;
use super::{Mapped, Mapper, error_message, events, first_line};
use crate::model::AgentEvent;
use crate::opencode_server::wire::Envelope;

/// The `reason` of the compaction events.
const MANUAL: &str = "manual";

/// What a quiet run does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietKind {
    /// A `!command`.
    Shell,
    /// A compaction.
    Compaction,
}

/// The running quiet run.
#[derive(Debug)]
pub(super) struct QuietRun {
    kind: QuietKind,
    generation: u64,
    /// Its `idle` arrived.
    idle: bool,
    /// Its call answered.
    answered: bool,
    /// The first error the run reported.
    error: Option<String>,
}

impl Mapper {
    /// Starts a quiet run of `kind` (the chat is idle); its generation, for
    /// [`Mapper::quiet_expire`].
    pub fn quiet_begin(&mut self, kind: QuietKind) -> u64 {
        self.quiet_generation += 1;
        self.quiet = Some(QuietRun {
            kind,
            generation: self.quiet_generation,
            idle: false,
            answered: false,
            error: None,
        });

        self.quiet_generation
    }

    /// The kind of the running quiet run.
    pub fn quiet_kind(&self) -> Option<QuietKind> {
        self.quiet.as_ref().map(|run| run.kind)
    }

    /// The run's call answered. `ran` is false when OpenCode never ran it (refused, or not
    /// reached), `error` says why the call failed.
    pub fn quiet_answered(&mut self, ran: bool, error: Option<String>) -> Vec<Mapped> {
        let Some(run) = self.quiet.as_mut() else {
            return Vec::new();
        };

        run.answered = true;
        if error.is_some() {
            run.error = error;
        }

        if run.idle || !ran {
            return self.quiet_end();
        }

        Vec::new()
    }

    /// Ends quiet run `generation` if it still waits for its `idle`.
    pub fn quiet_expire(&mut self, generation: u64) -> Vec<Mapped> {
        match &self.quiet {
            Some(run) if run.generation == generation && run.answered => self.quiet_end(),
            _ => Vec::new(),
        }
    }

    /// Takes in an event of the chat's session while a quiet run lasts; `None` when it is not one
    /// the run hides.
    pub(super) fn quiet_event(&mut self, envelope: &Envelope) -> Option<Vec<Mapped>> {
        let run = self.quiet.as_mut()?;
        let properties = &envelope.properties;

        match envelope.kind.as_str() {
            "session.status" => {
                if properties.pointer("/status/type").and_then(Value::as_str) == Some("idle") {
                    run.idle = true;

                    if run.answered {
                        return Some(self.quiet_end());
                    }
                }
            }
            "session.error" => {
                let error = properties.get("error").unwrap_or(&Value::Null);
                run.error.get_or_insert_with(|| error_message(error));
            }
            "message.updated" => {
                let info = properties.get("info").unwrap_or(&Value::Null);

                if let Some(error) = error_text(info) {
                    run.error.get_or_insert_with(|| first_line(&error));
                }
            }
            "message.part.updated" | "message.part.delta" => {}
            _ => return None,
        }

        Some(Vec::new())
    }

    /// Ends the quiet run: a compaction's `end`, then the next held follow-up.
    fn quiet_end(&mut self) -> Vec<Mapped> {
        let Some(run) = self.quiet.take() else {
            return Vec::new();
        };

        let mut mapped = Vec::new();

        if run.kind == QuietKind::Compaction {
            mapped.push(Mapped::Event(AgentEvent::Compaction {
                phase: "end".to_owned(),
                reason: Some(MANUAL.to_owned()),
                error: run.error,
            }));
        }

        if let Some((next, update)) = self.next_follow_up() {
            mapped.extend(events(update));
            mapped.push(Mapped::Send(next));
        }

        mapped
    }
}

/// The `compaction` `start` a compaction run begins with.
pub fn compaction_started() -> AgentEvent {
    AgentEvent::Compaction {
        phase: "start".to_owned(),
        reason: Some(MANUAL.to_owned()),
        error: None,
    }
}
