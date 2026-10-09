//! Process-wide state managed by Tauri and injected into commands.

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{HarnessId, HarnessSummary};
use harness_business::prefs::PrefsStore;
use harness_business::process::TokioRunner;
use tauri::async_runtime::RwLock;
use tauri::{AppHandle, Manager, Runtime};

/// State shared by all IPC commands.
pub struct AppState {
    /// Preferences in the OS app-config directory.
    pub prefs: SharedPrefs,
    /// The environment captured once at startup.
    pub env: Arc<Env>,
    /// Every supported harness.
    pub registry: Arc<HarnessRegistry>,
    /// Runs the detection probes (`<binary> --version`).
    pub runner: TokioRunner,
    /// The latest detection of each harness; filled by `harness_list`, refreshed by
    /// `harness_detect` and the binary override commands.
    pub detections: RwLock<HashMap<HarnessId, HarnessSummary>>,
    /// Set while a native dialog is open, so the webview cannot stack dialogs.
    pub dialog: DialogGuard,
}

impl AppState {
    /// Builds the state from the app's config directory and the process environment.
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> Result<Self, Box<dyn Error>> {
        let config_dir = app.path().app_config_dir()?;

        Ok(Self::new(config_dir, Env::from_process()?))
    }

    /// State over the prefs in `prefs_dir` and `env`, with the default harnesses.
    pub fn new(prefs_dir: PathBuf, env: Env) -> Self {
        Self {
            prefs: SharedPrefs::new(PrefsStore::new(prefs_dir)),
            env: Arc::new(env),
            registry: Arc::new(HarnessRegistry::with_defaults()),
            runner: TokioRunner,
            detections: RwLock::new(HashMap::new()),
            dialog: DialogGuard::default(),
        }
    }
}

/// A [`PrefsStore`] that serializes access across commands and can be moved into blocking tasks.
#[derive(Clone)]
pub struct SharedPrefs(Arc<Mutex<PrefsStore>>);

impl SharedPrefs {
    fn new(store: PrefsStore) -> Self {
        Self(Arc::new(Mutex::new(store)))
    }

    /// Runs `operation` with exclusive access to the store. Blocks; call from a blocking task.
    pub fn with<T>(&self, operation: impl FnOnce(&PrefsStore) -> T) -> T {
        // The store holds only its directory, so a panic elsewhere cannot leave it inconsistent.
        let store = self.0.lock().unwrap_or_else(PoisonError::into_inner);

        operation(&store)
    }
}

/// Allows one native dialog at a time.
#[derive(Debug, Default)]
pub struct DialogGuard(AtomicBool);

impl DialogGuard {
    /// Marks a dialog as open; `None` when one already is. The dialog counts as open until the
    /// returned token is dropped.
    pub fn try_open(&self) -> Option<OpenDialog<'_>> {
        let was_open = self.0.swap(true, Ordering::AcqRel);

        if was_open { None } else { Some(OpenDialog(&self.0)) }
    }
}

/// Proof that a dialog is open; dropping it marks the dialog as closed.
pub struct OpenDialog<'a>(&'a AtomicBool);

impl Drop for OpenDialog<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_dialog_is_open_at_a_time() {
        let guard = DialogGuard::default();

        let first = guard.try_open();
        assert!(first.is_some());
        assert!(guard.try_open().is_none());

        drop(first);
        assert!(guard.try_open().is_some());
    }
}
