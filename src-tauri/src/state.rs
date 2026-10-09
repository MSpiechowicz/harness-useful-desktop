//! Process-wide state managed by Tauri and injected into commands.

use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::env::Env;
use harness_business::prefs::PrefsStore;
use tauri::{AppHandle, Manager, Runtime};

/// State shared by all IPC commands.
pub struct AppState {
    /// Preferences in the OS app-config directory.
    pub prefs: SharedPrefs,
    /// The environment captured once at startup (used by harness commands in later milestones).
    pub env: Env,
}

impl AppState {
    /// Builds the state from the app's config directory and the process environment.
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> Result<Self, Box<dyn Error>> {
        let config_dir = app.path().app_config_dir()?;

        Ok(Self {
            prefs: SharedPrefs::new(PrefsStore::new(config_dir)),
            env: Env::from_process()?,
        })
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
