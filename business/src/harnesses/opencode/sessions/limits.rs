//! Limits on OpenCode CLI runs: concurrent callers that want the same result share one run
//! ([`Flights`]), and only a few commands run at once ([`Runs`]).

use std::sync::{Arc, Condvar, Mutex, PoisonError};

use super::lock;
use crate::error::{BusinessError, Result};

/// Runs in flight, one per key; a caller asking for a key that is running waits for that run's
/// result instead of starting another (or, with [`Flights::share_fresh`], for a newer run).
#[derive(Debug)]
pub(super) struct Flights<K, T> {
    running: Mutex<Vec<(K, Arc<Flight<T>>)>>,
}

impl<K: PartialEq + Clone, T> Flights<K, T> {
    pub(super) const fn new() -> Self {
        Self {
            running: Mutex::new(Vec::new()),
        }
    }

    /// The result of `work` for `key`: of a run already in flight for it, or of a new run.
    pub(super) fn share(&self, key: &K, work: impl FnOnce() -> Result<Arc<T>>) -> Result<Arc<T>> {
        let flight = {
            let mut running = lock(&self.running);

            if let Some((_, flight)) = running.iter().find(|(running, _)| running == key) {
                let flight = Arc::clone(flight);
                drop(running);

                return flight.wait();
            }

            let flight = Arc::new(Flight::new());
            running.push((key.clone(), Arc::clone(&flight)));
            flight
        };

        // Lands the flight even if `work` panics, so no waiter is left behind.
        let landing = Landing {
            flights: self,
            flight: &flight,
        };

        let result = work();
        landing.land(shared(&result));

        result
    }

    /// The result of `work` for `key` from a run that started after this call: a run already in
    /// flight may have started before something the caller needs to see, so it is waited out
    /// (its result ignored) and then [`Self::share`] joins or starts a newer one.
    ///
    /// A landed run leaves the flight list before its waiters wake, and while one is in flight
    /// no other run for the key starts, so any run in the list after the wait started after
    /// this call.
    pub(super) fn share_fresh(&self, key: &K, work: impl FnOnce() -> Result<Arc<T>>) -> Result<Arc<T>> {
        let older = lock(&self.running)
            .iter()
            .find(|(running, _)| running == key)
            .map(|(_, flight)| Arc::clone(flight));

        if let Some(older) = older {
            older.wait().ok();
        }

        self.share(key, work)
    }
}

/// One run that concurrent callers share: its result, once it is there.
#[derive(Debug)]
struct Flight<T> {
    result: Mutex<Option<Result<Arc<T>>>>,
    landed: Condvar,
}

impl<T> Flight<T> {
    const fn new() -> Self {
        Self {
            result: Mutex::new(None),
            landed: Condvar::new(),
        }
    }

    /// Waits for the run's result.
    fn wait(&self) -> Result<Arc<T>> {
        let mut result = lock(&self.result);

        loop {
            if let Some(result) = result.as_ref() {
                return shared(result);
            }

            result = self.landed.wait(result).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Hands `result` to the waiters; only the first result counts.
    fn finish(&self, result: Result<Arc<T>>) {
        let mut slot = lock(&self.result);

        if slot.is_none() {
            *slot = Some(result);
        }

        self.landed.notify_all();
    }
}

/// Ends a [`Flight`]: forgets it, so the next caller runs (or reads a cache) again, then hands its
/// result to the waiters. When dropped without landing (the run panicked), the waiters get an
/// error.
struct Landing<'a, K, T> {
    flights: &'a Flights<K, T>,
    flight: &'a Arc<Flight<T>>,
}

impl<K, T> Landing<'_, K, T> {
    /// Forgets the flight first, so no caller woken by `result` can join it again.
    fn land(&self, result: Result<Arc<T>>) {
        lock(&self.flights.running).retain(|(_, flight)| !Arc::ptr_eq(flight, self.flight));

        self.flight.finish(result);
    }
}

impl<K, T> Drop for Landing<'_, K, T> {
    fn drop(&mut self) {
        self.land(Err(BusinessError::Internal(
            "the OpenCode command did not finish".to_owned(),
        )));
    }
}

/// The same result again, for another caller that shared the run.
fn shared<T>(result: &Result<Arc<T>>) -> Result<Arc<T>> {
    let error = match result {
        Ok(value) => return Ok(Arc::clone(value)),
        Err(error) => error,
    };

    Err(match error {
        BusinessError::InvalidInput(message) => BusinessError::InvalidInput(message.clone()),
        BusinessError::NotFound(message) => BusinessError::NotFound(message.clone()),
        BusinessError::Forbidden(message) => BusinessError::Forbidden(message.clone()),
        BusinessError::Unsupported(message) => BusinessError::Unsupported(message.clone()),
        BusinessError::Io { context, source } => BusinessError::Io {
            context: context.clone(),
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        BusinessError::Process(message) => BusinessError::Process(message.clone()),
        BusinessError::Timeout(message) => BusinessError::Timeout(message.clone()),
        BusinessError::Protocol(message) => BusinessError::Protocol(message.clone()),
        BusinessError::Internal(message) => BusinessError::Internal(message.clone()),
    })
}

/// Counts the commands running, at most `max`; more wait for a free slot.
#[derive(Debug)]
pub(super) struct Runs {
    max: usize,
    running: Mutex<usize>,
    freed: Condvar,
}

impl Runs {
    pub(super) const fn new(max: usize) -> Self {
        Self {
            max,
            running: Mutex::new(0),
            freed: Condvar::new(),
        }
    }

    /// Waits for a free slot and holds it until the returned guard is dropped.
    pub(super) fn acquire(&self) -> RunSlot<'_> {
        let mut running = lock(&self.running);

        while *running >= self.max {
            running = self.freed.wait(running).unwrap_or_else(PoisonError::into_inner);
        }

        *running += 1;

        RunSlot(self)
    }
}

/// A held [`Runs`] slot, freed on drop.
pub(super) struct RunSlot<'a>(&'a Runs);

impl Drop for RunSlot<'_> {
    fn drop(&mut self) {
        *lock(&self.0.running) -= 1;
        self.0.freed.notify_one();
    }
}
