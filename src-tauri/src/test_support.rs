//! Test helpers for the shell's unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use harness_business::env::Env;
use harness_business::project::Projects;
use serde_json::Value;
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::state::AppState;

/// A uniquely named directory under the OS temp dir, removed on drop. Its path is canonical, so
/// containment checks compare like with like (macOS `/var` → `/private/var`).
pub struct TempDir(PathBuf);

impl TempDir {
    /// Creates `harness-desktop-<label>-<pid>-<n>`.
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);

        let name = format!(
            "harness-desktop-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).expect("create temp dir");

        Self(std::fs::canonicalize(&path).expect("canonical temp dir"))
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// App state over `env` with its prefs in `<dir>/config` and `project` created and opened (the
/// active project).
pub fn state_with_project(dir: &TempDir, env: Env, project: &Path) -> AppState {
    std::fs::create_dir_all(project).expect("create project");

    let state = AppState::new(dir.path().join("config"), env);
    state.prefs.with(|store| {
        Projects::new(store, &state.env).open(project).expect("open project");
    });

    state
}

/// A channel that keeps every message it receives, parsed as JSON.
pub fn collecting_channel<T>() -> (Channel<T>, Arc<Mutex<Vec<Value>>>) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let messages = received.clone();

    let channel = Channel::new(move |body| {
        if let InvokeResponseBody::Json(json) = body {
            let message = serde_json::from_str(&json).expect("channel messages are JSON");
            messages
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(message);
        }
        Ok(())
    });

    (channel, received)
}
