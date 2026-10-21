//! The saves Harness Memory holds for the user's review (see
//! `harness_business::ecosystem::memory_held`): the page that called `memory_held_listen` gets
//! `changed` whenever the held saves change (`held/<chatKey>/<heldId>.json`, one folder per chat).
//! The memory request thread ([`crate::memory_requests::spawn`]) looks every `POLL_INTERVAL`; only
//! names, sizes and times are read, never through a symbolic link, the saves themselves come from
//! `harness-memory held list`.
//!
//! The changes of one burst (a save claimed, written and removed by the server) make one
//! `changed`: it goes once the folders stayed the same for [`HELD_QUIET`], or [`HELD_MAX_WAIT`]
//! after the first change while they keep changing. So the page hears at most one `changed` per
//! [`HELD_QUIET`].

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use harness_business::ecosystem::{held_stamp, memory_data_dir};
use harness_business::env::Env;
use harness_business::model::MemoryHeldEvent;
use tauri::ipc::Channel;

/// How long the held saves stay the same before their changes are sent.
pub const HELD_QUIET: Duration = Duration::from_millis(500);

/// The longest a change waits while the held saves keep changing.
pub const HELD_MAX_WAIT: Duration = Duration::from_secs(2);

/// What the held folders looked like at the last look: chat key and name, size and modification
/// time of each held save.
type Stamp = Vec<(String, u64, Option<SystemTime>)>;

/// The listener and the last look. See the module docs.
#[derive(Default)]
pub struct HeldWatcher(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    listener: Option<Channel<MemoryHeldEvent>>,
    stamp: Option<Stamp>,
    pending: Option<Pending>,
}

/// Changes seen and not sent yet: when the first and the last were seen.
#[derive(Clone, Copy)]
struct Pending {
    first: Instant,
    last: Instant,
}

impl HeldWatcher {
    /// Replaces the listener.
    pub fn listen(&self, channel: Channel<MemoryHeldEvent>) {
        self.lock().listener = Some(channel);
        tracing::info!("a page listens for held memory saves");
    }

    /// One look at the held folders of `env`'s data folder at `now`: a change from the last look
    /// waits (the first look only remembers them), and `changed` goes to the listener once the
    /// changes seen settled (see the module docs). True when it went.
    pub fn tick(&self, env: &Env, now: Instant) -> bool {
        let Some(data_dir) = memory_data_dir(env) else {
            return false;
        };

        let stamp = held_stamp(&data_dir);
        let mut inner = self.lock();

        let changed = inner.stamp.as_ref().is_some_and(|last| *last != stamp);
        inner.stamp = Some(stamp);

        if changed {
            let first = inner.pending.map_or(now, |pending| pending.first);
            inner.pending = Some(Pending { first, last: now });
        }

        let Some(pending) = inner.pending else {
            return false;
        };

        let quiet = now.saturating_duration_since(pending.last) >= HELD_QUIET;
        let overdue = now.saturating_duration_since(pending.first) >= HELD_MAX_WAIT;

        if !quiet && !overdue {
            return false;
        }

        inner.pending = None;
        inner.send(MemoryHeldEvent::Changed);

        true
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    /// Sends `event` to the listener; a closed channel means its page is gone.
    fn send(&mut self, event: MemoryHeldEvent) {
        if self
            .listener
            .as_ref()
            .is_some_and(|channel| channel.send(event).is_err())
        {
            self.listener = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::*;
    use crate::test_support::{collecting_channel, memory_data_setup as setup};

    const CHAT: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";
    const HELD: &str = "6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d";
    const OTHER: &str = "7d2e4f3b-0c9f-4a8b-9d7e-6f5a4b3c2d1e";

    fn held_file(data: &Path, id: &str) -> PathBuf {
        let chat = data.join("held").join(CHAT);
        std::fs::create_dir_all(&chat).unwrap();

        chat.join(format!("{id}.json"))
    }

    /// A watcher with a listener that keeps every event it hears.
    fn listening() -> (HeldWatcher, Arc<Mutex<Vec<Value>>>) {
        let watcher = HeldWatcher::default();
        let (channel, received) = collecting_channel();
        watcher.listen(channel);

        (watcher, received)
    }

    #[test]
    fn the_listener_hears_when_a_save_is_held_or_settled_in_a_chat_folder() {
        let (_dir, env, data) = setup("held-watch");
        let (watcher, received) = listening();

        let start = Instant::now();
        let at = |millis: u64| start + Duration::from_millis(millis);

        assert!(!watcher.tick(&env, at(0)), "the first look only remembers the folders");

        std::fs::write(held_file(&data, HELD), "{}").unwrap();
        assert!(!watcher.tick(&env, at(500)), "a change waits for the folders to settle");
        assert!(watcher.tick(&env, at(1000)), "a save was held");
        assert!(!watcher.tick(&env, at(1500)), "nothing changed since");

        std::fs::remove_file(held_file(&data, HELD)).unwrap();
        assert!(!watcher.tick(&env, at(2000)));
        assert!(watcher.tick(&env, at(2500)), "the save was settled");

        let received = received.lock().unwrap();
        assert_eq!(*received, [json!({ "kind": "changed" }), json!({ "kind": "changed" })]);
    }

    #[test]
    fn a_burst_of_changes_makes_one_event() {
        let (_dir, env, data) = setup("held-burst");
        let (watcher, received) = listening();

        let start = Instant::now();
        let at = |millis: u64| start + Duration::from_millis(millis);

        assert!(!watcher.tick(&env, at(0)));

        std::fs::write(held_file(&data, HELD), "{}").unwrap();
        assert!(!watcher.tick(&env, at(100)));
        std::fs::write(held_file(&data, OTHER), "{}").unwrap();
        assert!(!watcher.tick(&env, at(200)));
        std::fs::remove_file(held_file(&data, HELD)).unwrap();
        assert!(!watcher.tick(&env, at(300)));
        assert!(!watcher.tick(&env, at(700)), "not quiet for long enough yet");
        assert!(watcher.tick(&env, at(800)), "one event for the burst");
        assert!(!watcher.tick(&env, at(1300)));

        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[test]
    fn changes_that_keep_coming_are_sent_at_most_every_quiet_period() {
        let (_dir, env, data) = setup("held-steady");
        let (watcher, received) = listening();

        let start = Instant::now();
        assert!(!watcher.tick(&env, start));

        let mut sent = Vec::new();

        for step in 1..=24u64 {
            let now = start + Duration::from_millis(step * 250);
            std::fs::write(held_file(&data, HELD), "x".repeat(step as usize)).unwrap();

            if watcher.tick(&env, now) {
                sent.push(now);
            }
        }

        assert!(!sent.is_empty(), "a steady stream still reaches the page");
        assert!(sent.windows(2).all(|pair| pair[1] - pair[0] >= HELD_QUIET), "{sent:?}");
        assert_eq!(received.lock().unwrap().len(), sent.len());
    }
}
