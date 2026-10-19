//! What the engine ([`super::engine`]) keeps about one run, and its small helpers.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::compose::ResolvedPart;
use super::plan::{Firing, MAX_TOUCHED_BYTES, PlannedPart};
use crate::model::{AgentAction, AgentReply};

/// Most paths a chat remembers for `sessionEnd` ([`TouchedLog`]).
pub(crate) const MAX_CHAT_TOUCHED: usize = 256;

/// What one run (from one settle to the next) did.
#[derive(Debug, Default)]
pub(crate) struct RunState {
    pub(crate) id: u64,
    /// User messages sent into this run that did not come back as handled without a run.
    pub(crate) user_messages: u32,
    pub(crate) tool_succeeded: bool,
    pub(crate) failed: bool,
    pub(crate) aborted: bool,
    /// `toolEnd` `followUp` hooks that matched, once each.
    pub(crate) tool_texts: Vec<ResolvedPart>,
    /// The latest output of each `toolEnd` command hook: (firing seq, output).
    pub(crate) tool_outputs: BTreeMap<String, (u64, Option<ResolvedPart>)>,
    /// `toolEnd` command firings not reported yet.
    pub(crate) outstanding: usize,
    pub(crate) touched: BTreeSet<String>,
}

impl RunState {
    pub(crate) fn new(id: u64) -> Self {
        Self { id, ..Self::default() }
    }

    /// True when a user message started or joined this run.
    pub(crate) fn user_sent(&self) -> bool {
        self.user_messages > 0
    }

    /// The follow-up loop rule.
    pub(crate) fn earned_follow_up(&self) -> bool {
        self.user_sent() && self.tool_succeeded && !self.failed && !self.aborted
    }

    pub(crate) fn record_output(&mut self, seq: u64, hook_id: &str, output: Option<ResolvedPart>) {
        let newer = self
            .tool_outputs
            .get(hook_id)
            .is_none_or(|(previous, _)| *previous < seq);

        if newer {
            self.tool_outputs.insert(hook_id.to_owned(), (seq, output));
        }
    }
}

/// A settled run whose follow-up waits for `toolEnd` commands still running.
#[derive(Debug)]
pub(crate) struct Waiting {
    pub(crate) run: RunState,
    pub(crate) turn_parts: Vec<PlannedPart>,
    /// A user message went out meanwhile: only its `turnEnd` commands still run.
    pub(crate) cancelled: bool,
}

/// The paths a chat's runs changed, for `sessionEnd`: oldest first, each once, at most
/// [`MAX_CHAT_TOUCHED`] paths and, joined by newlines, [`MAX_TOUCHED_BYTES`]. A path changed again
/// moves to the end, and the oldest go first when a limit is reached. A path longer than
/// [`MAX_TOUCHED_BYTES`] on its own is never kept, so it cannot push the others out.
#[derive(Debug, Default)]
pub(crate) struct TouchedLog {
    paths: VecDeque<String>,
    /// The paths' bytes, one newline each included.
    bytes: usize,
}

impl TouchedLog {
    pub(crate) fn add(&mut self, path: &str) {
        if path.len() > MAX_TOUCHED_BYTES {
            return;
        }

        if let Some(index) = self.paths.iter().position(|known| known == path)
            && let Some(known) = self.paths.remove(index)
        {
            self.bytes -= known.len() + 1;
        }

        self.bytes += path.len() + 1;
        self.paths.push_back(path.to_owned());

        while self.paths.len() > MAX_CHAT_TOUCHED || self.bytes > MAX_TOUCHED_BYTES + 1 {
            let Some(oldest) = self.paths.pop_front() else {
                break;
            };

            self.bytes -= oldest.len() + 1;
        }
    }

    pub(crate) fn paths(&self) -> Vec<String> {
        self.paths.iter().cloned().collect()
    }
}

/// The settles the stream delivered and the engine handled, and the user messages sent after a
/// settle the engine has not handled yet: each belongs to the run that starts after its settle.
#[derive(Debug, Default)]
pub(crate) struct SettleClock {
    /// `Settled` events handled.
    settles: u64,
    /// `Settled` events the stream delivered.
    seen: u64,
    /// One entry per deferred message: the settles seen when it went out, oldest first.
    deferred: VecDeque<u64>,
}

impl SettleClock {
    pub(crate) fn saw(&mut self, seen: u64) {
        self.seen = self.seen.max(seen);
    }

    /// A user message goes out: true when it joins the current run, false when a settle the
    /// stream delivered but the engine has not handled yet ends that run first.
    pub(crate) fn user_message(&mut self) -> bool {
        if self.seen > self.settles {
            self.deferred.push_back(self.seen);
            return false;
        }

        true
    }

    /// One more settle handled. Returns whether a user message went out after it (or a later one)
    /// already, and how many messages went out right after it, for the run it starts.
    pub(crate) fn settle(&mut self) -> (bool, u32) {
        self.settles += 1;

        let superseded = self.deferred.back().is_some_and(|after| *after >= self.settles);
        let mut starting = 0;

        while self.deferred.front().is_some_and(|after| *after <= self.settles) {
            if self.deferred.pop_front() == Some(self.settles) {
                starting += 1;
            }
        }

        (superseded, starting)
    }

    pub(crate) fn deferred(&self) -> VecDeque<u64> {
        self.deferred.clone()
    }

    pub(crate) fn restore_deferred(&mut self, deferred: VecDeque<u64>) {
        self.deferred = deferred;
    }
}

/// `firing` with its command parts only.
pub(crate) fn commands_only(firing: Firing) -> Firing {
    Firing {
        parts: firing
            .parts
            .into_iter()
            .filter(|part| matches!(part, PlannedPart::Command(_)))
            .collect(),
        ..firing
    }
}

/// A prompt that fires `sessionStart` and `promptSubmit`: no streaming behaviour, not a command
/// (`/…`) or shell line (`!…`).
pub(crate) fn is_eligible(action: &AgentAction) -> bool {
    match action {
        AgentAction::Prompt {
            message,
            streaming_behavior: None,
            ..
        } => !(message.starts_with('/') || message.starts_with('!')),
        _ => false,
    }
}

/// True when `reply` (to a prompt) says the harness handled the prompt itself, without a run (OMP
/// `agentInvoked: false`, PI `disposition: "handled"`): it reports a `Handled` result for it.
pub fn handled_without_run(reply: &AgentReply) -> bool {
    match reply {
        AgentReply::Prompt {
            agent_invoked,
            disposition,
        } => *agent_invoked == Some(false) || disposition.as_deref() == Some("handled"),
        _ => false,
    }
}
