//! The queue of code index builds started when a project opens (`memory_code_prepare`).

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tauri::async_runtime;
use tokio::sync::Semaphore;

/// The most paths that wait for, or run, a code index build at once. A page that keeps asking for
/// new paths cannot pile up tasks beyond it.
const MAX_QUEUED_PREPARES: usize = 8;

/// How long [`CodePrepares::resolve`] waits for a path to resolve, which a hung network mount can
/// stretch without end.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);

/// Runs the code index builds of all projects one after the other: each build is a process with its
/// own worker that can load hundreds of megabytes, so switching quickly through projects must not
/// start one per project. A project asked for while its own build waits or runs is not queued twice,
/// and nothing is queued beyond [`MAX_QUEUED_PREPARES`] paths.
pub struct CodePrepares {
    run: async_runtime::Mutex<()>,
    queued: Mutex<HashSet<PathBuf>>,
    resolving: Arc<Semaphore>,
}

impl Default for CodePrepares {
    fn default() -> Self {
        Self {
            run: async_runtime::Mutex::new(()),
            queued: Mutex::default(),
            resolving: Arc::new(Semaphore::new(1)),
        }
    }
}

impl CodePrepares {
    /// The real path of `project` (`/r`, `/r/.` and a symlink to it are one repository), read off
    /// the async threads. `None` when it does not resolve, or when the read, or the wait for an
    /// earlier read to finish, takes longer than a few seconds: at most one read runs at a time, and
    /// it keeps its blocking thread until the file system answers, so a hung mount pins one thread in
    /// total. A second project opened while the first is read waits its turn instead of being lost.
    pub async fn resolve(&self, project: String) -> Option<PathBuf> {
        self.resolve_within(project, RESOLVE_TIMEOUT, std::fs::canonicalize)
            .await
    }

    async fn resolve_within(
        &self,
        project: String,
        timeout: Duration,
        read: impl FnOnce(String) -> io::Result<PathBuf> + Send + 'static,
    ) -> Option<PathBuf> {
        let permit = tokio::time::timeout(timeout, self.resolving.clone().acquire_owned())
            .await
            .ok()?
            .ok()?;

        // The permit goes back when the read returns, not when the wait gives up.
        let read = tokio::task::spawn_blocking(move || {
            let resolved = read(project);
            drop(permit);

            resolved
        });

        tokio::time::timeout(timeout, read).await.ok()?.ok()?.ok()
    }

    /// Registers the build of `project`, a real path from [`CodePrepares::resolve`]; `None` when
    /// that path is already queued or running, or the queue is full. The path counts as queued
    /// until the returned token is dropped, also when the build ended in an error or the command
    /// was cancelled.
    pub fn enqueue(&self, project: PathBuf) -> Option<QueuedPrepare<'_>> {
        let mut queued = self.queued.lock().unwrap_or_else(PoisonError::into_inner);

        if queued.len() >= MAX_QUEUED_PREPARES {
            return None;
        }

        if !queued.insert(project.clone()) {
            return None;
        }

        Some(QueuedPrepare {
            prepares: self,
            project,
        })
    }
}

/// Proof that a path is queued for a code index build; dropping it takes the path off the queue.
pub struct QueuedPrepare<'a> {
    prepares: &'a CodePrepares,
    project: PathBuf,
}

impl<'a> QueuedPrepare<'a> {
    /// Waits until no other build runs. The build counts as running until the returned token is
    /// dropped.
    pub async fn turn(&self) -> PrepareTurn<'a> {
        self.prepares.run.lock().await
    }
}

impl Drop for QueuedPrepare<'_> {
    fn drop(&mut self) {
        let mut queued = self.prepares.queued.lock().unwrap_or_else(PoisonError::into_inner);

        queued.remove(&self.project);
    }
}

/// Proof that a code index build is running; dropping it frees the next build.
pub type PrepareTurn<'a> = tokio::sync::MutexGuard<'a, ()>;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::TempDir;

    fn queued<'a>(prepares: &'a CodePrepares, name: &str) -> Option<QueuedPrepare<'a>> {
        prepares.enqueue(PathBuf::from(name))
    }

    #[test]
    fn a_path_is_queued_once_until_its_token_is_dropped() {
        let prepares = CodePrepares::default();

        let first = queued(&prepares, "/r");
        assert!(first.is_some());
        assert!(queued(&prepares, "/r").is_none(), "the same path twice is one request");
        let other = queued(&prepares, "/other");
        assert!(other.is_some(), "another path is a request of its own");

        drop(first);
        assert!(queued(&prepares, "/r").is_some(), "the path may be queued again");
    }

    #[test]
    fn the_queue_is_bounded() {
        let prepares = CodePrepares::default();

        let held: Vec<_> = (0..MAX_QUEUED_PREPARES)
            .map(|index| queued(&prepares, &format!("/r{index}")).expect("room in the queue"))
            .collect();
        assert!(queued(&prepares, "/one-too-many").is_none());

        drop(held);
        assert!(
            queued(&prepares, "/one-too-many").is_some(),
            "dropped tokens free their room"
        );
    }

    #[test]
    fn a_second_path_waits_for_the_running_build_and_then_runs() {
        let prepares = CodePrepares::default();
        let first = queued(&prepares, "/first").expect("queued");
        let second = queued(&prepares, "/second").expect("queued");

        block_on(async {
            let running = first.turn().await;
            let early = tokio::time::timeout(Duration::from_millis(50), second.turn()).await;
            assert!(early.is_err(), "the second build waits while the first runs");

            drop(running);
            let late = tokio::time::timeout(Duration::from_secs(5), second.turn()).await;
            assert!(late.is_ok(), "the second build runs once the first ended");
        });
    }

    #[test]
    fn a_cancelled_wait_leaves_the_queue() {
        let prepares = CodePrepares::default();
        let first = queued(&prepares, "/first").expect("queued");
        let second = queued(&prepares, "/second").expect("queued");

        block_on(async {
            let _running = first.turn().await;
            let waited = tokio::time::timeout(Duration::from_millis(20), second.turn()).await;
            assert!(waited.is_err());
        });

        drop(second);
        assert!(queued(&prepares, "/second").is_some());
    }

    #[test]
    fn a_path_resolves_to_its_real_path() {
        let dir = TempDir::new("code-prepare-resolve");
        let prepares = CodePrepares::default();

        let dotted = format!("{}/.", dir.path().display());
        let resolved = block_on(prepares.resolve(dotted));

        assert_eq!(resolved, Some(std::fs::canonicalize(dir.path()).expect("canonical")));
        assert_eq!(block_on(prepares.resolve("/no/such/path/at/all".to_owned())), None);
    }

    #[test]
    fn a_read_that_hangs_pins_one_thread_and_refuses_the_next_reads() {
        let prepares = CodePrepares::default();
        let (release, hang) = std::sync::mpsc::channel::<()>();

        let hung = block_on(
            prepares.resolve_within("/hung".to_owned(), Duration::from_millis(50), move |_| {
                let _ = hang.recv();

                Ok(PathBuf::from("/hung"))
            }),
        );
        assert_eq!(hung, None, "a read past the timeout is given up");

        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();
        let refused = block_on(
            prepares.resolve_within("/next".to_owned(), Duration::from_millis(50), move |_| {
                flag.store(true, Ordering::SeqCst);

                Ok(PathBuf::from("/next"))
            }),
        );
        assert_eq!(refused, None, "the hung read still holds the only slot past the wait");
        assert!(!started.load(Ordering::SeqCst), "no second thread was pinned");

        drop(release);
        let mut resolved = None;
        for _ in 0..200 {
            resolved = block_on(
                prepares.resolve_within("/after".to_owned(), Duration::from_secs(5), |path| {
                    Ok(PathBuf::from(path))
                }),
            );
            if resolved.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            resolved,
            Some(PathBuf::from("/after")),
            "the slot is free once the read returned"
        );
    }

    #[test]
    fn a_second_read_waits_for_a_quick_first_one_and_then_resolves() {
        let prepares = Arc::new(CodePrepares::default());
        let (release, slow) = std::sync::mpsc::channel::<()>();
        let first = {
            let prepares = prepares.clone();

            std::thread::spawn(move || {
                block_on(
                    prepares.resolve_within("/first".to_owned(), Duration::from_secs(5), move |path| {
                        let _ = slow.recv();

                        Ok(PathBuf::from(path))
                    }),
                )
            })
        };

        // the first read holds the slot until it is released, a moment after the second one asks
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(release);
        });
        let second = block_on(
            prepares.resolve_within("/second".to_owned(), Duration::from_secs(5), |path| {
                Ok(PathBuf::from(path))
            }),
        );

        releaser.join().expect("releaser");
        assert_eq!(first.join().expect("first"), Some(PathBuf::from("/first")));
        assert_eq!(second, Some(PathBuf::from("/second")), "the second project is not lost");
    }
}
