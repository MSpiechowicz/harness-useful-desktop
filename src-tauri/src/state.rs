//! Process-wide state managed by Tauri and injected into commands.

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::app_dirs::local_data_dir;
use harness_business::ecosystem::{CompanionReleases, Dashboard, DashboardOptions, MemoryScripts, PoolOptions};
use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::hooks::HookStore;
use harness_business::model::{HarnessId, HarnessSummary};
use harness_business::pi_family::sessions::SessionCache;
use harness_business::prefs::PrefsStore;
use harness_business::process::{TokioRunner, TokioSpawner};
use harness_business::routines::RoutineStore;
use harness_business::sync::StoredPlan;
use tauri::async_runtime::{self, RwLock};
use tauri::{AppHandle, Manager, Runtime};

use crate::agents::Agents;
use crate::drops::DropSink;
use crate::hooks::HookRuntime;
use crate::memory_requests::ScriptRequests;
use crate::routines::RoutineRuntime;
use crate::updates::Updates;

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
    /// The running agent sessions.
    pub agents: Arc<Agents>,
    /// Saved-session summaries by file, reused by `sessions_list` while a file is unchanged.
    pub sessions: Arc<SessionCache>,
    /// The pooled `harness-dashboard mcp` connection (stops after 30 s without a query).
    pub dashboard: Arc<Dashboard>,
    /// The pooled `harness-memory mcp` connection for the Memory view's scripts (stops after 30 s
    /// without a call), and whether the installed server keeps scripts.
    pub memory_scripts: Arc<MemoryScripts>,
    /// The script run requests of Harness Memory and the heartbeat that lets the server ask.
    pub script_requests: Arc<ScriptRequests>,
    /// Where saved attachments live (the app's cache; the agent opens them by path).
    pub attachments_dir: PathBuf,
    /// Where native drag and drop events go.
    pub drops: DropSink,
    /// The in-app updater (off when the build has no updater public key).
    pub updates: Updates,
    /// The cached latest GitHub releases of the companion tools (`ecosystem_releases`).
    pub releases: CompanionReleases,
    /// Held while the setup step's dashboard installer runs, so only one runs at a time.
    pub dashboard_install: async_runtime::Mutex<()>,
    /// Held while the memory server's installer runs, so only one runs at a time.
    pub memory_install: async_runtime::Mutex<()>,
    /// The latest "Share setup" import review, kept until it is applied or replaced by a newer one.
    pub sync_plan: Mutex<Option<StoredPlan>>,
    /// Held while a "Share setup" import is applied, so only one runs at a time.
    pub sync_apply: async_runtime::Mutex<()>,
    /// The OS app-config directory (prefs, the memory registration record).
    pub config_dir: PathBuf,
    /// Held while the memory server is installed, registered or unregistered, so one runs at a time.
    pub memory_setup: async_runtime::Mutex<()>,
    /// Held while the app's own MCP server is registered, so one registration runs at a time.
    pub desktop_mcp_setup: async_runtime::Mutex<()>,
    /// The hooks folder, its approvals and the hooks of each chat.
    pub hooks: Arc<HookRuntime>,
    /// The routines folder, its schedule and the runs.
    pub routines: Arc<RoutineRuntime>,
}

impl AppState {
    /// Builds the state from the app's config directory and the process environment.
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> Result<Self, Box<dyn Error>> {
        let config_dir = app.path().app_config_dir()?;

        Ok(Self {
            updates: Updates::from_config(app.config()),
            attachments_dir: app.path().app_cache_dir()?.join("attachments"),
            ..Self::new(config_dir, Env::from_process()?)
        })
    }

    /// State over the prefs (and the other config files) in `prefs_dir` and `env`, with the default
    /// harnesses and updates off.
    pub fn new(prefs_dir: PathBuf, env: Env) -> Self {
        let prefs = SharedPrefs::new(PrefsStore::new(prefs_dir.clone()));
        let env = Arc::new(env);
        let hooks = HookRuntime::new(HookStore::from_env(&env, prefs_dir.clone()), prefs.clone(), env.clone());
        let memory_scripts = Arc::new(MemoryScripts::new(Arc::new(TokioSpawner), PoolOptions::default()));
        let routines = RoutineRuntime::new(
            RoutineStore::from_env(&env, prefs_dir.clone()),
            local_data_dir(&env),
            env.clone(),
            memory_scripts.clone(),
        );

        Self {
            prefs,
            env,
            registry: Arc::new(HarnessRegistry::with_defaults()),
            runner: TokioRunner,
            detections: RwLock::new(HashMap::new()),
            dialog: DialogGuard::default(),
            agents: Arc::new(Agents::new()),
            sessions: Arc::new(SessionCache::default()),
            dashboard: Arc::new(Dashboard::new(Arc::new(TokioSpawner), DashboardOptions::default())),
            memory_scripts,
            script_requests: Arc::new(ScriptRequests::default()),
            attachments_dir: prefs_dir.join("attachments"),
            drops: DropSink::default(),
            updates: Updates::new(None),
            releases: CompanionReleases::default(),
            dashboard_install: async_runtime::Mutex::new(()),
            memory_install: async_runtime::Mutex::new(()),
            sync_plan: Mutex::new(None),
            sync_apply: async_runtime::Mutex::new(()),
            config_dir: prefs_dir,
            memory_setup: async_runtime::Mutex::new(()),
            desktop_mcp_setup: async_runtime::Mutex::new(()),
            hooks: Arc::new(hooks),
            routines: Arc::new(routines),
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
