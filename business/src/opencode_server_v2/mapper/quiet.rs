//! Quiet runs: work the app starts in the chat's session that is not a turn of the chat, a
//! `!command` (`POST …/shell`) or a compaction (`POST …/compact`), shown as the OpenCode 1 mapper
//! shows them.
//!
//! - While a quiet run lasts the chat counts as working (prompts wait as follow-ups, steering
//!   too) and the run's own events are not shown.
//! - **`!command`:** OpenCode runs it without a turn (`session.shell.started`,
//!   `session.shell.ended` with the output, then a synthetic inbox item that reaches the model
//!   with the next prompt). The run ends once its call answered and `session.shell.ended`
//!   arrived, in either order, with [`Mapped::ShellDone`] (the redacted output and exit code).
//! - **Compaction:** OpenCode admits it as an inbox item and runs it as an execution
//!   (`session.compaction.started`, `delta`, `ended` or `failed`). The engine shows `compaction`
//!   `start` ([`compaction_started`]) when it asks; the run ends once its call answered and the
//!   execution ended, with `compaction` `end` (with the call's error, else the run's first
//!   error) and the context marked as compacted.
//! - A call OpenCode refused or that never reached it ends the run at once. A call answered
//!   whose run does not follow ends with [`Mapper::quiet_expire`]: a `!command` whose end never
//!   came, a compaction whose execution never started. Then the next held follow-up is sent.

use super::{ContextNote, Mapped, Mapper, error_message, events};
use crate::model::AgentEvent;
use crate::opencode_server_v2::events::{Event, EventKind, SessionStatus, ShellEnded};
use crate::security::redact_text;

/// The `reason` of the compaction events.
const MANUAL: &str = "manual";

/// The error of a compaction that was interrupted.
const INTERRUPTED: &str = "The compaction was interrupted";

/// The error kind of an interruption.
const ABORTED: &str = "aborted";

/// What a quiet run does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietKind {
    /// A `!command`.
    Shell,
    /// A compaction.
    Compaction,
}

/// How a `!command` ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellResult {
    /// The command line.
    pub command: String,
    /// Its exit code, when it exited.
    pub exit: Option<i64>,
    /// Its merged output, redacted.
    pub output: String,
    /// True when OpenCode kept only the start of the output.
    pub truncated: bool,
}

/// The running quiet run.
#[derive(Debug)]
pub(super) struct QuietRun {
    kind: QuietKind,
    generation: u64,
    /// The command or the execution started.
    started: bool,
    /// The command or the execution ended.
    ended: bool,
    /// Its call answered.
    answered: bool,
    /// The first error the run reported.
    error: Option<String>,
    /// How the command ended.
    shell: Option<ShellResult>,
}

impl Mapper {
    /// Starts a quiet run of `kind` (the chat is idle); its generation, for
    /// [`Mapper::quiet_expire`].
    pub fn quiet_begin(&mut self, kind: QuietKind) -> u64 {
        self.quiet_generation += 1;
        self.quiet = Some(QuietRun {
            kind,
            generation: self.quiet_generation,
            started: false,
            ended: false,
            answered: false,
            error: None,
            shell: None,
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

        if run.ended || !ran {
            return self.quiet_end();
        }

        Vec::new()
    }

    /// Ends quiet run `generation` if its call answered and its run did not follow (see the
    /// module docs).
    pub fn quiet_expire(&mut self, generation: u64) -> Vec<Mapped> {
        let expired = self.quiet.as_ref().is_some_and(|run| {
            let waiting = run.kind == QuietKind::Shell || !run.started;
            run.generation == generation && run.answered && waiting
        });

        if expired {
            return self.quiet_end();
        }

        Vec::new()
    }

    /// Takes in an event of the chat's session while a quiet run lasts; `None` when it is not one
    /// the run hides.
    pub(super) fn quiet_event(&mut self, event: &Event) -> Option<Vec<Mapped>> {
        let run = self.quiet.as_mut()?;

        let mapped = match run.kind {
            QuietKind::Shell => shell_event(run, &event.kind)?,
            QuietKind::Compaction => compaction_event(run, &event.kind)?,
        };

        if run.ended && run.answered {
            return Some(mapped.into_iter().chain(self.quiet_end()).collect());
        }

        Some(mapped)
    }

    /// Ends the quiet run: a compaction's `end` or a command's output, then the next held
    /// follow-up.
    fn quiet_end(&mut self) -> Vec<Mapped> {
        let Some(run) = self.quiet.take() else {
            return Vec::new();
        };

        let mut mapped = Vec::new();

        match run.kind {
            QuietKind::Compaction => mapped.push(Mapped::event(AgentEvent::Compaction {
                phase: "end".to_owned(),
                reason: Some(MANUAL.to_owned()),
                error: run.error,
            })),
            QuietKind::Shell => mapped.extend(run.shell.map(Mapped::ShellDone)),
        }

        if let Some((next, update)) = self.next_follow_up() {
            mapped.extend(events(update));
            mapped.push(Mapped::Send(next));
        }

        mapped
    }
}

/// An event while a `!command` runs: its start and end, and the synthetic item of its output.
fn shell_event(run: &mut QuietRun, kind: &EventKind) -> Option<Vec<Mapped>> {
    match kind {
        EventKind::ShellStarted(_) => run.started = true,
        EventKind::ShellEnded(ended) => {
            run.ended = true;
            run.shell = Some(shell_result(ended));
        }
        EventKind::InboxEnqueued(enqueued) if enqueued.item.kind == "synthetic" => {}
        _ => return None,
    }

    Some(Vec::new())
}

/// An event while a compaction runs: its execution, its inbox item, its compaction events and
/// any step of it.
fn compaction_event(run: &mut QuietRun, kind: &EventKind) -> Option<Vec<Mapped>> {
    let mut mapped = Vec::new();

    match kind {
        EventKind::ExecutionStarted(_) => run.started = true,
        EventKind::ExecutionSucceeded(_) => run.ended = true,
        EventKind::ExecutionFailed(failed) => {
            let error = if failed.error.kind == ABORTED {
                INTERRUPTED.to_owned()
            } else {
                error_message(&failed.error.message)
            };
            run.error.get_or_insert(error);
            run.ended = true;
        }
        EventKind::ExecutionInterrupted(_) => {
            run.error.get_or_insert_with(|| INTERRUPTED.to_owned());
            run.ended = true;
        }
        EventKind::Status(changed) => match changed.status {
            SessionStatus::Idle => run.ended |= run.started,
            SessionStatus::Busy | SessionStatus::Retry { .. } => run.started = true,
            SessionStatus::Other => {}
        },
        EventKind::Idle(_) => run.ended |= run.started,
        EventKind::CompactionFailed(failed) => {
            run.error.get_or_insert_with(|| error_message(&failed.error.message));
        }
        EventKind::CompactionEnded(_) => mapped.push(Mapped::Context(ContextNote::Compacted)),
        EventKind::CompactionStarted(_)
        | EventKind::CompactionDelta(_)
        | EventKind::InboxEnqueued(_)
        | EventKind::InboxDelivered(_)
        | EventKind::InboxCancelled(_)
        | EventKind::StepStarted(_)
        | EventKind::StepStreamed(_)
        | EventKind::StepEnded(_)
        | EventKind::StepFailed(_)
        | EventKind::TextStarted(_)
        | EventKind::TextDelta(_)
        | EventKind::TextEnded(_)
        | EventKind::ReasoningStarted(_)
        | EventKind::ReasoningDelta(_)
        | EventKind::ReasoningEnded(_)
        | EventKind::RetryScheduled(_) => {}
        _ => return None,
    }

    Some(mapped)
}

fn shell_result(ended: &ShellEnded) -> ShellResult {
    ShellResult {
        command: ended.shell.command.clone(),
        exit: ended.shell.exit,
        output: redact_text(&ended.output.output),
        truncated: ended.output.truncated,
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
