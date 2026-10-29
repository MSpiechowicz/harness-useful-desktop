//! Process-wide state managed by Tauri and injected into commands.

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::app_dirs::{child_processes_dir, local_data_dir};
use harness_business::ecosystem::{
    ChatGuardWrites, CompanionReleases, Dashboard, DashboardOptions, EmbedderSupervisor, MemoryFeatures, MemoryKinds,
    MemoryScripts, NotesCopyCache, PoolOptions, SaveRoute, StoredNotesImport,
};
use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::hooks::HookStore;
use harness_business::model::{HarnessId, HarnessSummary};
use harness_business::pi_family::sessions::SessionCache;
use harness_business::prefs::PrefsStore;
use harness_business::process::{TokioRunner, TokioSpawner};
use harness_business::project::ProjectKeys;
use harness_business::routines::RoutineStore;
use harness_business::sync::StoredPlan;
use tauri::async_runtime::{self, RwLock};
use tauri::{AppHandle, Manager, Runtime};

use crate::agents::Agents;
use crate::drops::DropSink;
use crate::hooks::HookRuntime;
use crate::memory_code_builds::CodeBuilds;
use crate::memory_embedder::EmbedderWatch;
use crate::memory_held::HeldWatcher;
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
    /// Which team databases the installed Harness Memory can use (`harness-memory kinds`, probed
    /// once per binary).
    pub memory_kinds: MemoryKinds,
    /// What else the installed Harness Memory can do (`harness-memory features`, probed once per
    /// binary).
    pub memory_features: MemoryFeatures,
    /// The embedder behind vector search (`harness-memory embedder run`), kept running while project
    /// memory and vector search are on.
    pub memory_embedder: Arc<EmbedderSupervisor>,
    /// The page told when the embedder's state changes.
    pub embedder_watch: EmbedderWatch,
    /// The script run requests of Harness Memory and the heartbeat that lets the server ask.
    pub script_requests: Arc<ScriptRequests>,
    /// The page told when the saves Harness Memory holds for review change.
    pub memory_held: Arc<HeldWatcher>,
    /// The page told when a code graph build begins, progresses or ends.
    pub code_builds: Arc<CodeBuilds>,
    /// The writes of each chat's memory guards file after its start (toggles, newest revision
    /// only, and moves), one at a time per chat.
    pub memory_chat_writes: Arc<ChatGuardWrites>,
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
    /// Held while the Harness Skills' OpenCode installer runs, so only one runs at a time.
    pub opencode_skills_install: async_runtime::Mutex<()>,
    /// Held while the Harness Skills' Codex installer runs, so only one runs at a time.
    pub codex_skills_install: async_runtime::Mutex<()>,
    /// The latest "Share setup" import review, kept until it is applied or replaced by a newer one.
    pub sync_plan: Mutex<Option<StoredPlan>>,
    /// Held while a "Share setup" import is applied, so only one runs at a time.
    pub sync_apply: async_runtime::Mutex<()>,
    /// The latest review of a notes file to import, kept until it is imported or replaced by a
    /// newer one.
    pub notes_import: Mutex<Option<StoredNotesImport>>,
    /// Held while a notes import is applied, so only one runs at a time.
    pub notes_import_apply: async_runtime::Mutex<()>,
    /// The latest review of copying this computer's notes, and the latest of copying its scripts,
    /// to the team database, each kept until it is copied or replaced by a newer one of its kind.
    pub notes_copy: NotesCopyCache,
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
    /// The Harness Memory repository key of each project, for the session (`project_scopes`).
    pub project_keys: ProjectKeys,
    /// The locks that let one git write (fetch, push, rebase, switch...) run at a time per project.
    pub git_writes: GitWriteLocks,
}

impl AppState {
    /// Builds the state from the app's config directory and the process environment.
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> Result<Self, Box<dyn Error>> {
        let config_dir = app.path().app_config_dir()?;
        let env = Env::from_process()?;
        let attachments_dir = app.path().app_cache_dir()?.join("attachments");
        let agents = Agents::tracked(child_processes_dir(&env), attachments_dir.clone());

        Ok(Self {
            updates: Updates::from_config(app.config()),
            attachments_dir,
            memory_embedder: Arc::new(EmbedderSupervisor::new(agents.spawner())),
            agents: Arc::new(agents),
            ..Self::new(config_dir, env)
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
            memory_kinds: MemoryKinds::new(),
            memory_features: MemoryFeatures::new(),
            memory_embedder: Arc::new(EmbedderSupervisor::new(Arc::new(TokioSpawner))),
            embedder_watch: EmbedderWatch::default(),
            script_requests: Arc::new(ScriptRequests::default()),
            memory_held: Arc::new(HeldWatcher::default()),
            code_builds: Arc::new(CodeBuilds::default()),
            memory_chat_writes: Arc::new(ChatGuardWrites::default()),
            attachments_dir: prefs_dir.join("attachments"),
            drops: DropSink::default(),
            updates: Updates::new(None),
            releases: CompanionReleases::default(),
            dashboard_install: async_runtime::Mutex::new(()),
            memory_install: async_runtime::Mutex::new(()),
            opencode_skills_install: async_runtime::Mutex::new(()),
            codex_skills_install: async_runtime::Mutex::new(()),
            sync_plan: Mutex::new(None),
            sync_apply: async_runtime::Mutex::new(()),
            notes_import: Mutex::new(None),
            notes_import_apply: async_runtime::Mutex::new(()),
            notes_copy: NotesCopyCache::new(),
            config_dir: prefs_dir,
            memory_setup: async_runtime::Mutex::new(()),
            desktop_mcp_setup: async_runtime::Mutex::new(()),
            hooks: Arc::new(hooks),
            routines: Arc::new(routines),
            project_keys: ProjectKeys::default(),
            git_writes: GitWriteLocks::default(),
        }
    }

    /// How the user's own notes and scripts are saved: through the commands agents cannot reach
    /// while the installed Harness Memory lists `save-guards`, else through MCP.
    pub async fn save_route(&self) -> SaveRoute {
        SaveRoute::of(self.memory_features.support(&self.env, &self.runner).await)
    }
}

/// One async lock per repository, held while a git command that changes the repository (or fetches
/// into it) runs, so two clicks or a click and a background fetch never overlap. The key is the
/// repository's common git directory, so projects in a subdirectory of one repository, or in
/// linked work trees of it, share the lock.
#[derive(Default)]
pub struct GitWriteLocks(Mutex<HashMap<PathBuf, Arc<async_runtime::Mutex<()>>>>);

impl GitWriteLocks {
    /// The lock of the repository whose common git directory (or, outside a repository, whose
    /// project directory) is `key`, created on first use.
    pub fn of(&self, key: &Path) -> Arc<async_runtime::Mutex<()>> {
        let mut locks = self.0.lock().unwrap_or_else(PoisonError::into_inner);

        locks.entry(key.to_path_buf()).or_default().clone()
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
