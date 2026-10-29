//! Code graph builds of Harness Memory, announced to the page that called
//! `memory_code_builds_listen` (see `harness_business::ecosystem::memory_code` for the status
//! files). The memory request thread ([`crate::memory_requests::spawn`]) calls
//! [`CodeBuilds::tick`] every `POLL_INTERVAL`; each build that began, moved into another 10 % step
//! of its files or ended since the last look is one event.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use harness_business::ecosystem::{CodeBuildWatcher, memory_data_dir};
use harness_business::env::Env;
use harness_business::model::CodeBuildEvent;
use tauri::ipc::Channel;

/// The listener and the watcher. See the module docs.
#[derive(Default)]
pub struct CodeBuilds(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    listener: Option<Channel<CodeBuildEvent>>,
    watcher: Option<CodeBuildWatcher>,
}

impl CodeBuilds {
    /// Replaces the listener and sends it the builds still in progress, so a reloaded page shows
    /// them again.
    pub fn listen(&self, channel: Channel<CodeBuildEvent>) {
        let mut inner = self.lock();

        let active = inner.watcher.as_ref().map(CodeBuildWatcher::active).unwrap_or_default();

        inner.listener = Some(channel);
        tracing::info!(building = active.len(), "a page listens for code graph builds");

        for event in active {
            inner.send(event);
        }
    }

    /// One look at the build status files in the data folder of `env` at `now`.
    pub fn tick(&self, env: &Env, now: SystemTime) {
        let Some(data_dir) = memory_data_dir(env) else {
            return;
        };

        let mut inner = self.lock();

        let events = inner
            .watcher
            .get_or_insert_with(|| CodeBuildWatcher::new(&data_dir))
            .poll(now);

        for event in events {
            inner.send(event);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    /// Sends `event` to the listener; a closed channel means its page is gone.
    fn send(&mut self, event: CodeBuildEvent) {
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
    use std::path::Path;

    use harness_business::ecosystem::memory_code::BUILDS_DIR;
    use serde_json::json;

    use super::*;
    use crate::test_support::{collecting_channel, memory_data_setup as setup};

    const NAME: &str = "0123456789abcdef.json";

    fn write_status(data: &Path, files_done: u64) {
        let folder = data.join(BUILDS_DIR);
        std::fs::create_dir_all(&folder).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        let status = json!({
            "version": 1, "buildId": 3, "root": "/work/demo", "repoName": "demo", "kind": "full",
            "phase": "building", "filesTotal": 100, "filesDone": files_done,
            "startedAt": 0, "finishedAt": null, "error": null
        });

        std::fs::write(folder.join(NAME), status.to_string()).unwrap();
    }

    #[test]
    fn builds_reach_the_listener_once_and_a_new_listener_gets_the_ones_in_progress() {
        let (_dir, env, data) = setup("code-builds");
        std::fs::create_dir_all(&data).unwrap();
        write_status(&data, 20);

        let builds = CodeBuilds::default();
        let (first, first_received) = collecting_channel();
        builds.listen(first);

        builds.tick(&env, SystemTime::now());
        builds.tick(&env, SystemTime::now());

        let received = first_received.lock().unwrap().clone();
        assert_eq!(received.len(), 1, "an unchanged file is announced once");
        assert_eq!(received[0]["phase"], "building");
        assert_eq!(received[0]["repoName"], "demo");
        assert_eq!(received[0]["filesDone"], 20);
        assert_eq!(received[0]["buildId"], 3);

        // A reloaded page listens again and sees the build at once.
        let (second, second_received) = collecting_channel();
        builds.listen(second);
        assert_eq!(second_received.lock().unwrap().len(), 1);

        write_status(&data, 60);
        builds.tick(&env, SystemTime::now());
        assert_eq!(second_received.lock().unwrap().len(), 2);
        assert_eq!(
            first_received.lock().unwrap().len(),
            1,
            "the first listener was replaced"
        );
    }
}
