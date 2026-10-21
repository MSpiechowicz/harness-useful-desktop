//! Script run requests of Harness Memory, shown to the user while the app runs (see
//! `harness_business::ecosystem::memory_requests` for the files).
//!
//! [`spawn`] starts one thread that, every [`POLL_INTERVAL`], looks for new and vanished requests
//! and sends them to the page that called `memory_script_requests_listen` (a new one also reaches
//! the user through [`crate::notifications`] while the main window is not in front), and every
//! [`HEARTBEAT_INTERVAL`] touches `desktop.alive` while Harness Memory is installed and its data
//! folder exists (the server creates it), and on every poll tells [`crate::memory_held`]'s
//! listener when the held saves changed (one event per burst). [`ScriptRequests::stop`] (app exit)
//! ends both and removes the heartbeat file, so the server stops waiting for an answer from this
//! app. Before its first poll the thread removes the chats' memory guards files untouched for 30 days
//! ([`sweep_chats`]).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime};

use harness_business::ecosystem::memory_requests::{HEARTBEAT_INTERVAL, POLL_INTERVAL, check_request_id};
use harness_business::ecosystem::{
    CHAT_MAX_AGE, RequestWatcher, memory_data_dir, remove_alive, resolve_memory, sweep_chats, touch_alive,
};
use harness_business::env::Env;
use harness_business::model::ScriptRequestEvent;
use harness_business::{BusinessError, Result};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::AppState;

/// How many polls make one heartbeat.
const POLLS_PER_HEARTBEAT: u32 = (HEARTBEAT_INTERVAL.as_millis() / POLL_INTERVAL.as_millis()) as u32;

/// The listener, the watcher and the heartbeat. See the module docs.
#[derive(Default)]
pub struct ScriptRequests(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    listener: Option<Channel<ScriptRequestEvent>>,
    watcher: Option<RequestWatcher>,
    /// The data folder `desktop.alive` was written to.
    alive: Option<PathBuf>,
    /// Whether Harness Memory was installed at the last heartbeat.
    installed: bool,
    /// Whether a failed heartbeat was logged (once until one succeeds).
    warned: bool,
    stopped: bool,
}

impl ScriptRequests {
    /// Replaces the listener and sends it every request still waiting.
    pub fn listen(&self, channel: Channel<ScriptRequestEvent>) {
        let mut inner = self.lock();

        let pending = inner.watcher.as_ref().map(RequestWatcher::pending).unwrap_or_default();

        inner.listener = Some(channel);
        tracing::info!(waiting = pending.len(), "a page listens for memory run requests");

        for request in pending {
            inner.send(ScriptRequestEvent::Request {
                request: Box::new(request),
            });
        }
    }

    /// One heartbeat (when `heartbeat`) and one look at the requests, in the data folder of `env`.
    /// Returns the script names of the requests seen for the first time, oldest first.
    pub fn tick(&self, env: &Env, heartbeat: bool, now: SystemTime) -> Vec<String> {
        let Some(data_dir) = memory_data_dir(env) else {
            return Vec::new();
        };

        let mut inner = self.lock();

        if inner.stopped {
            return Vec::new();
        }

        if heartbeat {
            inner.installed = resolve_memory(env).is_some();
            inner.beat(&data_dir);
        }

        if !inner.installed {
            return Vec::new();
        }

        let events = inner
            .watcher
            .get_or_insert_with(|| RequestWatcher::new(&data_dir))
            .poll(now);

        let mut fresh = Vec::new();

        for event in events {
            if let ScriptRequestEvent::Request { request } = &event {
                fresh.push(request.script.name.clone());
            }

            inner.send(event);
        }

        fresh
    }

    /// Answers request `request_id` (see [`RequestWatcher::decide`]): only one the watcher has
    /// shown and not yet answered, so nothing else can make the server run a script.
    /// `invalidInput` for a malformed id, `notFound` otherwise.
    pub fn decide(&self, request_id: &str, allow: bool) -> Result<()> {
        check_request_id(request_id)?;

        let mut inner = self.lock();

        match inner.watcher.as_mut() {
            Some(watcher) => watcher.decide(request_id, allow),
            None => Err(BusinessError::NotFound(format!("run request {request_id}"))),
        }
    }

    /// Ends the heartbeat and removes `desktop.alive` (app exit). Later ticks do nothing.
    pub fn stop(&self) {
        let mut inner = self.lock();
        inner.stopped = true;

        if let Some(dir) = inner.alive.take()
            && let Err(error) = remove_alive(&dir)
        {
            tracing::warn!(%error, "the memory heartbeat file could not be removed");
        }
    }

    /// Whether [`ScriptRequests::stop`] ran.
    pub fn is_stopped(&self) -> bool {
        self.lock().stopped
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    /// Touches `desktop.alive` while Harness Memory is installed; removes it once it is not.
    fn beat(&mut self, data_dir: &Path) {
        if !self.installed {
            if let Some(dir) = self.alive.take() {
                remove_alive(&dir).ok();
            }
            return;
        }

        match touch_alive(data_dir) {
            Ok(true) => {
                if self.alive.is_none() {
                    tracing::info!("memory heartbeat started");
                }

                self.alive = Some(data_dir.to_path_buf());
                self.warned = false;
            }
            Ok(false) => {}
            Err(error) => {
                if !self.warned {
                    tracing::warn!(%error, "the memory heartbeat file could not be touched");
                    self.warned = true;
                }
            }
        }
    }

    /// Sends `event` to the listener; a closed channel means its page is gone.
    fn send(&mut self, event: ScriptRequestEvent) {
        if self
            .listener
            .as_ref()
            .is_some_and(|channel| channel.send(event).is_err())
        {
            self.listener = None;
        }
    }
}

/// Starts the request thread (see the module docs). It ends once [`ScriptRequests::stop`] ran.
pub fn spawn<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();

    let started = std::thread::Builder::new()
        .name("memory-requests".to_owned())
        .spawn(move || {
            if let Some(data_dir) = handle
                .try_state::<AppState>()
                .and_then(|state| memory_data_dir(&state.env))
            {
                sweep_chats(&data_dir, CHAT_MAX_AGE);
            }

            let mut polls: u32 = 0;

            loop {
                let Some(state) = handle.try_state::<AppState>() else {
                    return;
                };

                if state.script_requests.is_stopped() {
                    return;
                }

                let fresh = state.script_requests.tick(&state.env, polls == 0, SystemTime::now());
                state.memory_held.tick(&state.env, Instant::now());
                polls = (polls + 1) % POLLS_PER_HEARTBEAT;

                if !fresh.is_empty() {
                    crate::notifications::alert(&handle, &fresh);
                }

                std::thread::sleep(POLL_INTERVAL);
            }
        });

    if let Err(error) = started {
        tracing::error!(%error, "cannot start the memory request thread; agents cannot ask to run scripts");
    }
}

#[cfg(test)]
mod tests {
    use harness_business::ecosystem::memory_requests::{ALIVE_FILE, REQUESTS_DIR};
    use serde_json::json;

    use super::*;
    use crate::test_support::{
        REQUEST_ID as ID, collecting_channel, memory_data_setup as setup, place_companion, write_script_request,
    };

    fn write_request(data: &Path) {
        write_script_request(data, ID);
    }

    #[test]
    fn nothing_is_written_or_read_while_harness_memory_is_not_installed() {
        let (_dir, env, data) = setup("requests-none");
        std::fs::create_dir_all(&data).unwrap();
        write_request(&data);

        let requests = ScriptRequests::default();
        let (channel, received) = collecting_channel();
        requests.listen(channel);
        requests.tick(&env, true, SystemTime::now());

        assert!(!data.join(ALIVE_FILE).exists());
        assert!(received.lock().unwrap().is_empty());
    }

    #[test]
    fn the_heartbeat_runs_while_installed_and_stops_with_the_app() {
        let (_dir, env, data) = setup("requests-alive");
        place_companion(&env.home, "harness-memory");
        let requests = ScriptRequests::default();

        requests.tick(&env, true, SystemTime::now());
        assert!(!data.exists(), "the app never creates the data folder");

        std::fs::create_dir_all(&data).unwrap();
        requests.tick(&env, true, SystemTime::now());
        assert!(data.join(ALIVE_FILE).is_file());

        requests.stop();
        assert!(!data.join(ALIVE_FILE).exists());

        requests.tick(&env, true, SystemTime::now());
        assert!(!data.join(ALIVE_FILE).exists(), "no heartbeat after stop");
    }

    #[test]
    fn requests_reach_the_listener_and_a_new_listener_gets_the_waiting_ones() {
        let (_dir, env, data) = setup("requests-listen");
        place_companion(&env.home, "harness-memory");
        std::fs::create_dir_all(&data).unwrap();
        write_request(&data);

        let requests = ScriptRequests::default();
        let (first, first_received) = collecting_channel();
        requests.listen(first);
        assert_eq!(
            requests.tick(&env, true, SystemTime::now()),
            ["deploy"],
            "a new request is reported"
        );

        let first_received = first_received.lock().unwrap().clone();
        assert_eq!(first_received.len(), 1);
        assert_eq!(first_received[0]["type"], "request");
        assert_eq!(first_received[0]["request"]["requestId"], ID);
        assert_eq!(first_received[0]["request"]["script"]["approval"], "needed");

        // A reloaded page listens again and sees the request at once.
        let (second, second_received) = collecting_channel();
        requests.listen(second);
        assert_eq!(second_received.lock().unwrap().len(), 1);
        assert!(
            requests.tick(&env, false, SystemTime::now()).is_empty(),
            "a request sent again to a new listener is not reported again"
        );

        std::fs::remove_file(data.join(REQUESTS_DIR).join(format!("{ID}.json"))).unwrap();
        requests.tick(&env, false, SystemTime::now());

        let second_received = second_received.lock().unwrap();
        assert_eq!(second_received.len(), 2);
        assert_eq!(second_received[1], json!({ "type": "gone", "requestId": ID }));
    }
}
