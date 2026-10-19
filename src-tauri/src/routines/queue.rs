//! The places of routine runs: one run per routine, at most [`MAX_AGENT_RUNS`] agent and
//! [`MAX_SHELL_RUNS`] command or script runs at once, the others waiting in the order they were
//! due. Pure bookkeeping, used by [`super::runner`] under the runtime's lock.
//!
//! - Once the app closes ([`Runs::stop_all`]) and while a routine is being deleted
//!   ([`Runs::begin_delete`]) no run is taken in ([`Reserved::Refused`]).
//! - A stop asked for a run that has a place but is not recorded yet (it is starting) is kept and
//!   applied the moment it is ([`Runs::set_live`]), so a starting run never escapes a stop.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use harness_business::model::{HookOrigin, RoutineAction, RoutineRun, RunSession, RunTrigger};
use tokio::sync::watch;

use super::job::StopReason;

/// Agent runs at once.
pub const MAX_AGENT_RUNS: usize = 2;

/// Command and script runs at once.
pub const MAX_SHELL_RUNS: usize = 4;

/// Which limit a run counts against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunClass {
    /// An agent chat.
    Agent,
    /// A command or a script.
    Shell,
}

impl RunClass {
    /// The class of `action`.
    pub fn of(action: &RoutineAction) -> Self {
        match action {
            RoutineAction::Agent { .. } => Self::Agent,
            RoutineAction::Command { .. } | RoutineAction::Script { .. } => Self::Shell,
        }
    }

    fn limit(self) -> usize {
        match self {
            Self::Agent => MAX_AGENT_RUNS,
            Self::Shell => MAX_SHELL_RUNS,
        }
    }
}

/// What [`Runs::reserve`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reserved {
    /// The routine waits or runs already.
    Busy,
    /// A place was taken: start it now.
    Now,
    /// No place is free: it waits.
    Queued,
    /// The app is closing, or the routine is being deleted: it does not run.
    Refused,
}

/// The routines waiting or running, by routine id.
#[derive(Default)]
pub struct Runs {
    entries: HashMap<String, Entry>,
    queue: VecDeque<String>,
    /// The app is closing: nothing starts any more.
    closing: bool,
    /// The routines being deleted: they do not run until the delete is over.
    deleting: HashSet<String>,
}

struct Entry {
    class: RunClass,
    trigger: RunTrigger,
    live: Option<Live>,
    started: bool,
    /// A stop asked for before the run was recorded.
    stop: Option<StopReason>,
}

/// A run that was recorded.
struct Live {
    run_id: String,
    origin: HookOrigin,
    stop: watch::Sender<Option<StopReason>>,
    needs_you: bool,
    /// The run as its record says while it runs.
    run: RoutineRun,
}

impl Runs {
    /// Takes a place for a run of routine `id` (`class`, `trigger`), or queues it.
    pub fn reserve(&mut self, id: &str, class: RunClass, trigger: RunTrigger) -> Reserved {
        if self.closing || self.deleting.contains(id) {
            return Reserved::Refused;
        }

        if self.entries.contains_key(id) {
            return Reserved::Busy;
        }

        let started = self.has_room(class);

        self.entries.insert(
            id.to_owned(),
            Entry {
                class,
                trigger,
                live: None,
                started,
                stop: None,
            },
        );

        if started {
            Reserved::Now
        } else {
            self.queue.push_back(id.to_owned());
            Reserved::Queued
        }
    }

    fn has_room(&self, class: RunClass) -> bool {
        let taken = self
            .entries
            .values()
            .filter(|entry| entry.started && entry.class == class)
            .count();

        taken < class.limit()
    }

    /// Frees the place (or the queue entry) of routine `id`.
    pub fn release(&mut self, id: &str) {
        self.entries.remove(id);
        self.queue.retain(|queued| queued != id);
    }

    /// The waiting routines that have a place now, in the order they were due; their places are
    /// taken.
    pub fn next_ready(&mut self) -> Vec<(String, RunTrigger)> {
        let mut ready = Vec::new();
        let waiting: Vec<String> = self.queue.iter().cloned().collect();

        for id in waiting {
            let Some(class) = self.entries.get(&id).map(|entry| entry.class) else {
                continue;
            };

            if !self.has_room(class) {
                continue;
            }

            if let Some(entry) = self.entries.get_mut(&id) {
                entry.started = true;
                ready.push((id.clone(), entry.trigger));
            }

            self.queue.retain(|queued| *queued != id);
        }

        ready
    }

    /// The routines waiting or running.
    pub fn busy(&self) -> BTreeSet<String> {
        self.entries.keys().cloned().collect()
    }

    /// The routines with a run going on (started, not waiting).
    pub fn running(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.started)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The routines whose run waits for a free place.
    pub fn queued(&self) -> BTreeSet<String> {
        self.queue.iter().cloned().collect()
    }

    /// Whether run `run_id` is live and waits for an answer.
    pub fn needs_you(&self, run_id: &str) -> bool {
        self.live_of(run_id).is_some_and(|live| live.needs_you)
    }

    fn live_of(&self, run_id: &str) -> Option<&Live> {
        self.entries
            .values()
            .filter_map(|entry| entry.live.as_ref())
            .find(|live| live.run_id == run_id)
    }
}

impl Runs {
    /// Why the starting run of routine `id` is to stop before it was recorded, if it is.
    pub fn stop_before_start(&self, id: &str) -> Option<StopReason> {
        self.entries.get(id).and_then(|entry| entry.stop)
    }

    /// Marks the started `run` of its routine, which `stop` stops. A stop asked for while it
    /// started is sent at once.
    pub fn set_live(&mut self, run: &RoutineRun, stop: watch::Sender<Option<StopReason>>) {
        let Some(entry) = self.entries.get_mut(&run.routine_id) else {
            return;
        };

        if let Some(reason) = entry.stop {
            stop.send_replace(Some(reason));
        }

        entry.live = Some(Live {
            run_id: run.record.id.clone(),
            origin: run.record.origin,
            stop,
            needs_you: false,
            run: run.clone(),
        });
    }

    /// Live run `run_id` of routine `id` ended: it is not listed as going on any more, while its
    /// place stays taken until [`Runs::release`].
    pub fn end_live(&mut self, id: &str, run_id: &str) {
        if let Some(entry) = self.entries.get_mut(id)
            && entry.live.as_ref().is_some_and(|live| live.run_id == run_id)
        {
            entry.live = None;
        }
    }

    /// Keeps the harness session of live run `run_id` of routine `id`.
    pub fn set_session(&mut self, id: &str, run_id: &str, session: RunSession) {
        let live = self
            .entries
            .get_mut(id)
            .and_then(|entry| entry.live.as_mut())
            .filter(|live| live.run_id == run_id);

        if let Some(live) = live {
            live.run.record.session = Some(session);
        }
    }

    /// The recorded runs going on, as their records say, with whether each waits for an answer.
    pub fn live_records(&self) -> Vec<RoutineRun> {
        self.entries
            .values()
            .filter_map(|entry| entry.live.as_ref())
            .map(|live| RoutineRun {
                needs_you: live.needs_you,
                ..live.run.clone()
            })
            .collect()
    }

    /// The recorded runs going on.
    pub fn live_runs(&self) -> BTreeSet<String> {
        self.entries
            .values()
            .filter_map(|entry| entry.live.as_ref())
            .map(|live| live.run_id.clone())
            .collect()
    }

    /// True when no routine waits or runs.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The routines whose team run is going on, and those whose run is still starting (its origin
    /// is not known yet).
    pub fn team_runs(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.started)
            .filter(|(_, entry)| entry.live.as_ref().is_none_or(|live| live.origin == HookOrigin::Team))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Routines `ids` are being deleted: no run is taken in for them until [`Runs::end_delete`].
    pub fn begin_delete(&mut self, ids: &[String]) {
        self.deleting.extend(ids.iter().cloned());
    }

    /// The delete of `ids` is over.
    pub fn end_delete(&mut self, ids: &[String]) {
        for id in ids {
            self.deleting.remove(id);
        }
    }

    /// Sets whether run `run_id` of routine `id` waits for an answer; true when that changed.
    pub fn set_needs_you(&mut self, id: &str, run_id: &str, waiting: bool) -> bool {
        let live = self
            .entries
            .get_mut(id)
            .and_then(|entry| entry.live.as_mut())
            .filter(|live| live.run_id == run_id);

        live.is_some_and(|live| std::mem::replace(&mut live.needs_you, waiting) != waiting)
    }

    /// Stops run `run_id` for `reason`; false when it is not live.
    pub fn stop_run(&self, run_id: &str, reason: StopReason) -> bool {
        let Some(live) = self.live_of(run_id) else {
            return false;
        };

        live.stop.send_replace(Some(reason));
        true
    }

    /// Stops the live runs of `ids` for `reason` and drops the waiting ones. True while one of
    /// them still has a place (it is ending, or starting).
    pub fn stop_routines(&mut self, ids: &[String], reason: StopReason) -> bool {
        for id in ids {
            let Some(entry) = self.entries.get(id) else {
                continue;
            };

            if !entry.started {
                self.release(id);
            } else if let Some(entry) = self.entries.get_mut(id) {
                stop_entry(entry, reason);
            }
        }

        ids.iter().any(|id| self.entries.contains_key(id))
    }

    /// The app is closing: nothing starts from now on, every live or starting run stops for
    /// `reason` and the queue empties.
    pub fn stop_all(&mut self, reason: StopReason) {
        self.closing = true;
        self.queue.clear();
        self.entries.retain(|_, entry| entry.started);

        for entry in self.entries.values_mut() {
            stop_entry(entry, reason);
        }
    }
}

/// Stops the run of `entry` for `reason`: at once when it is recorded, else as soon as it is.
fn stop_entry(entry: &mut Entry, reason: StopReason) {
    entry.stop.get_or_insert(reason);

    if let Some(live) = &entry.live {
        live.stop.send_replace(Some(reason));
    }
}
