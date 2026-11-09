//! The delivery worker: alerts arrive in the order they were raised, one delivery that panics does
//! not end the worker, and a worker that is stuck never blocks the one that raises an alert.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use harness_business::model::AppPrefs;

use super::*;
use crate::test_support::wait_until;

const FIRST: &str = "0f4c8a1e-7d52-4c1b-9a3e-5b6d7e8f9a0b";
const SECOND: &str = "9b8a7c6d-1234-4f5e-8a9b-0c1d2e3f4a5b";

/// A host that records the chats it notified. Its first notification is slow, or panics, or waits
/// for the gate to open, as the test sets it up.
#[derive(Default)]
struct ScriptedHost {
    notified: Mutex<Vec<String>>,
    attentions: AtomicUsize,
    first_is_slow: bool,
    first_panics: bool,
    gate: Gate,
}

/// Blocks every delivery until it is opened.
struct Gate {
    open: Mutex<bool>,
    changed: Condvar,
}

impl Default for Gate {
    fn default() -> Self {
        Self::opened(true)
    }
}

impl Gate {
    fn opened(open: bool) -> Self {
        Self {
            open: Mutex::new(open),
            changed: Condvar::new(),
        }
    }

    fn open(&self) {
        *self.open.lock().unwrap() = true;
        self.changed.notify_all();
    }

    fn pass(&self) {
        let open = self.open.lock().unwrap();
        drop(self.changed.wait_while(open, |open| !*open).unwrap());
    }
}

impl ScriptedHost {
    fn notified(&self) -> Vec<String> {
        self.notified.lock().unwrap().clone()
    }
}

impl Host for Arc<ScriptedHost> {
    fn prefs(&self) -> AppPrefs {
        AppPrefs::default()
    }

    fn in_front(&self) -> bool {
        false
    }

    fn attention(&self) {
        let nth = self.attentions.fetch_add(1, Ordering::SeqCst);

        self.gate.pass();

        if nth == 0 && self.first_is_slow {
            std::thread::sleep(Duration::from_millis(200));
        }

        assert!(!(nth == 0 && self.first_panics), "the first delivery panics");
    }

    fn notify(&self, _: &str, _: &str, key: &str) {
        self.notified.lock().unwrap().push(key.to_owned());
    }

    fn chime(&self) {}
}

fn attached(host: ScriptedHost) -> (ChatAlerts, Arc<ScriptedHost>) {
    let alerts = ChatAlerts::default();
    let host = Arc::new(host);

    alerts.attach_host(Arc::new(host.clone()));

    (alerts, host)
}

#[test]
fn alerts_are_delivered_in_the_order_they_were_raised_even_when_the_first_is_slow() {
    let (alerts, host) = attached(ScriptedHost {
        first_is_slow: true,
        ..ScriptedHost::default()
    });

    alerts.raise(FIRST, ChatAlertKind::Finished);
    alerts.raise(SECOND, ChatAlertKind::Failed);
    alerts.raise(FIRST, ChatAlertKind::NeedsYou);

    wait_until("every delivery", || host.notified().len() == 3);

    assert_eq!(host.notified(), [FIRST, SECOND, FIRST]);
}

#[test]
fn a_delivery_that_panics_does_not_end_the_worker() {
    let (alerts, host) = attached(ScriptedHost {
        first_panics: true,
        ..ScriptedHost::default()
    });

    alerts.raise(FIRST, ChatAlertKind::Finished);
    alerts.raise(SECOND, ChatAlertKind::Failed);
    alerts.raise(FIRST, ChatAlertKind::Failed);

    wait_until("the deliveries after the panic", || host.notified().len() == 2);

    assert_eq!(host.notified(), [SECOND, FIRST]);
}

#[test]
fn a_stuck_worker_never_blocks_the_task_that_raises_and_the_queue_is_bounded() {
    let (alerts, host) = attached(ScriptedHost {
        gate: Gate::opened(false),
        ..ScriptedHost::default()
    });

    // Would never return if raising waited for the worker.
    for _ in 0..QUEUE_LEN * 3 {
        alerts.raise(FIRST, ChatAlertKind::Finished);
    }

    assert_eq!(alerts.raised().len(), QUEUE_LEN * 3, "every alert is recorded");

    host.gate.open();
    wait_until("the queued deliveries", || host.notified().len() >= QUEUE_LEN);

    // The delivery in hand and a full queue at most: the rest was dropped.
    std::thread::sleep(Duration::from_millis(200));
    assert!(host.notified().len() <= QUEUE_LEN + 1, "{}", host.notified().len());
}
