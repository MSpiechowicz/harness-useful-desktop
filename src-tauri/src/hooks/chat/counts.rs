//! The counts behind [`super::ChatHooks::is_idle`]: what a chat's sink passed on, what its hook
//! task handled, and the follow-up whose run has not settled yet.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// What a chat's sink counted and its hook task did.
#[derive(Default)]
pub(super) struct Counts {
    /// The settles the sink has seen (changed under `progress`).
    settles: AtomicU64,
    progress: Mutex<Progress>,
}

#[derive(Default)]
struct Progress {
    /// Events handed to the hook task.
    passed: u64,
    /// Events the hook task is done with.
    handled: u64,
    /// The settles seen right before the last follow-up went out, while its run has not settled.
    follow_up_at: Option<u64>,
}

impl Counts {
    fn lock(&self) -> MutexGuard<'_, Progress> {
        // Plain counters: a panic cannot leave them half-written.
        self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The sink got an event: a settle (`settled`), and one handed to the hook task (`passed`).
    pub(super) fn arrived(&self, settled: bool, passed: bool) {
        let mut progress = self.lock();

        if settled {
            self.settles.fetch_add(1, Ordering::AcqRel);
        }

        if passed {
            progress.passed += 1;
        }
    }

    /// The settles the sink has seen.
    pub(super) fn settles(&self) -> u64 {
        self.settles.load(Ordering::Acquire)
    }

    /// The hook task is done with one event.
    pub(super) fn handled(&self) {
        self.lock().handled += 1;
    }

    /// A follow-up that starts a run went out after `settles` settles.
    pub(super) fn follow_up_sent(&self, settles: u64) {
        self.lock().follow_up_at = Some(settles);
    }

    /// Every event passed on was handled and the last follow-up's run settled.
    pub(super) fn is_idle(&self) -> bool {
        let progress = self.lock();
        let settles = self.settles();

        progress.passed == progress.handled && progress.follow_up_at.is_none_or(|at| settles > at)
    }
}
