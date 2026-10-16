//! Process-wide state managed by Tauri and injected into commands.

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use harness_business::app_dirs::legacy_memory_copy;
use harness_business::ecosystem::{
    CompanionReleases, Dashboard, DashboardOptions, MemoryRecall, memory_version, resolve_memory,
};
use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{HarnessId, HarnessSummary};
use harness_business::pi_family::sessions::SessionCache;
use harness_business::prefs::PrefsStore;
use harness_business::process::{ProcessRunner, ResolvedBinary, TokioRunner, TokioSpawner};
use harness_business::sync::StoredPlan;
use tauri::async_runtime::{self, RwLock};
use tauri::{AppHandle, Manager, Runtime};

use crate::agents::Agents;
use crate::drops::DropSink;
use crate::memory_recall::RecallGate;
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
    /// Asks `harness-memory` for the notes about a chat's first message.
    pub memory_recall: MemoryRecall,
    /// The chats whose next message gets a recall, with their warmed-up servers.
    pub recall: Arc<RecallGate>,
    /// The `harness-memory` executable recalls run (no recall while none is found).
    pub memory_server: MemoryServer,
}

impl AppState {
    /// Builds the state from the app's config directory and the process environment.
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> Result<Self, Box<dyn Error>> {
        let config_dir = app.path().app_config_dir()?;

        Ok(Self {
            updates: Updates::from_config(app.config()),
            attachments_dir: app.path().app_cache_dir()?.join("attachments"),
            memory_server: MemoryServer::lookup(),
            ..Self::new(config_dir, Env::from_process()?)
        })
    }

    /// State over the prefs (and the other config files) in `prefs_dir` and `env`, with the default
    /// harnesses, updates off and no memory server (it is never looked up).
    pub fn new(prefs_dir: PathBuf, env: Env) -> Self {
        Self {
            prefs: SharedPrefs::new(PrefsStore::new(prefs_dir.clone())),
            env: Arc::new(env),
            registry: Arc::new(HarnessRegistry::with_defaults()),
            runner: TokioRunner,
            detections: RwLock::new(HashMap::new()),
            dialog: DialogGuard::default(),
            agents: Arc::new(Agents::new()),
            sessions: Arc::new(SessionCache::default()),
            dashboard: Arc::new(Dashboard::new(Arc::new(TokioSpawner), DashboardOptions::default())),
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
            memory_recall: MemoryRecall::new(Arc::new(TokioSpawner)),
            recall: Arc::new(RecallGate::default()),
            memory_server: MemoryServer::none(),
        }
    }
}

/// Where the `harness-memory` server recalls run is: the installed one ([`resolve_memory`]), else
/// the copy older app versions kept in `<local data>/bin` (macOS and Windows), so recall keeps
/// working until Harness Memory is installed and launch retires that copy. Either is used only once
/// it reported its version (`--version`, once per lookup).
///
/// Looked up on first use. While none is found every use looks again, so a server installed
/// meanwhile is used at once; a file that did not report a version is not asked again until it
/// changes. [`MemoryServer::refresh`] forgets everything (after an install, or when project memory
/// is turned on). Clones share the lookup.
#[derive(Clone)]
pub struct MemoryServer(Arc<Lookup>);

struct Lookup {
    /// False for a state that never looks the server up (tests).
    enabled: bool,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    found: Option<PathBuf>,
    /// The files that did not report a version, with their modification time then.
    rejected: Vec<Stamped>,
    /// Bumped by every refresh, so a lookup that started before one does not store its result.
    generation: u64,
}

/// A file and its modification time.
type Stamped = (PathBuf, Option<SystemTime>);

impl MemoryServer {
    /// Looks the server up on first use.
    pub fn lookup() -> Self {
        Self::with(true, None)
    }

    /// Never a server.
    pub fn none() -> Self {
        Self::with(false, None)
    }

    /// Always `path`, never looked up.
    #[cfg(test)]
    pub fn fixed(path: PathBuf) -> Self {
        Self::with(false, Some(path))
    }

    fn with(enabled: bool, found: Option<PathBuf>) -> Self {
        let cache = Cache {
            found,
            ..Cache::default()
        };

        Self(Arc::new(Lookup {
            enabled,
            cache: Mutex::new(cache),
        }))
    }

    /// The server, looked up in `env` (version probes run with `runner`) when it was not found yet.
    pub async fn get(&self, env: &Env, runner: &dyn ProcessRunner) -> Option<PathBuf> {
        let (generation, rejected) = {
            let cache = self.lock();

            if cache.found.is_some() || !self.0.enabled {
                return cache.found.clone();
            }

            (cache.generation, cache.rejected.clone())
        };

        let (found, newly_rejected) = recall_server(env, runner, &rejected).await;

        let mut cache = self.lock();
        if cache.generation == generation {
            cache.found.clone_from(&found);
            cache.rejected.extend(newly_rejected);
        }

        found
    }

    /// Forgets the server and the files that did not report a version, so the next use looks again
    /// (the server may have been installed or moved).
    pub fn refresh(&self) {
        let mut cache = self.lock();

        if !self.0.enabled {
            return;
        }

        cache.found = None;
        cache.rejected.clear();
        cache.generation += 1;
    }

    fn lock(&self) -> MutexGuard<'_, Cache> {
        // Every write leaves a complete value, so a panic cannot leave it half-written.
        self.0.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The installed server, else the old app copy, whichever first reports its version; files in
/// `rejected` (unchanged since) are skipped. Also returns the files that did not report one now.
async fn recall_server(env: &Env, runner: &dyn ProcessRunner, rejected: &[Stamped]) -> (Option<PathBuf>, Vec<Stamped>) {
    let legacy = legacy_memory_copy(env)
        .filter(|copy| copy.is_file())
        .map(|path| ResolvedBinary {
            path,
            is_batch_script: false,
        });

    let mut newly_rejected = Vec::new();

    for binary in resolve_memory(env).into_iter().chain(legacy) {
        let stamped = (binary.path.clone(), modified(&binary.path));

        if rejected.contains(&stamped) {
            continue;
        }

        if memory_version(env, runner, &binary).await.is_some() {
            return (Some(binary.path), newly_rejected);
        }

        tracing::warn!(
            path = %binary.path.display(),
            "harness-memory did not report a version; recalls do not run it"
        );
        newly_rejected.push(stamped);
    }

    (None, newly_rejected)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok()
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
    use harness_business::env::OsKind;

    use super::*;
    use crate::test_support::{
        TempDir, place_companion, place_executable, place_memory_server, place_memory_server_in,
    };

    #[test]
    fn only_one_dialog_is_open_at_a_time() {
        let guard = DialogGuard::default();

        let first = guard.try_open();
        assert!(first.is_some());
        assert!(guard.try_open().is_none());

        drop(first);
        assert!(guard.try_open().is_some());
    }

    /// The server `server` finds in `env`.
    fn found(server: &MemoryServer, env: &Env) -> Option<PathBuf> {
        tauri::async_runtime::block_on(server.get(env, &TokioRunner))
    }

    #[test]
    fn the_memory_server_is_found_once_it_is_installed() {
        let home = TempDir::new("memory-server");
        let env = Env::for_tests(home.path());
        let server = MemoryServer::lookup();

        assert_eq!(found(&server, &env), None);

        let Some(installed) = place_memory_server(home.path()) else {
            return;
        };
        assert_eq!(found(&server, &env), Some(installed.clone()));

        std::fs::remove_file(&installed).expect("remove the server");
        assert_eq!(
            found(&server, &env),
            Some(installed),
            "a found server is kept until a refresh"
        );

        server.refresh();
        assert_eq!(found(&server, &env), None);
    }

    #[test]
    // `~/.local/bin` is the installer's folder only on Linux and macOS (Windows has its own, tested apart).
    #[cfg(unix)]
    fn recall_uses_the_old_app_copy_until_the_server_is_installed() {
        let home = TempDir::new("memory-server-legacy");
        let env = Env::for_tests(home.path()).with_os(OsKind::MacOs);
        let copy = legacy_memory_copy(&env).expect("macOS has a legacy copy path");
        let Some(copy) = place_memory_server_in(copy.parent().expect("parent")) else {
            return;
        };

        let server = MemoryServer::lookup();
        assert_eq!(found(&server, &env), Some(copy));

        let installed = place_memory_server(home.path()).expect("fake_agent is built");
        server.refresh();
        assert_eq!(found(&server, &env), Some(installed), "the installed server wins");
    }

    #[test]
    // `~/.local/bin` is the installer's folder only on Linux and macOS (Windows has its own, tested apart).
    #[cfg(unix)]
    fn recall_runs_the_installers_server_over_one_on_path_that_reports_no_version() {
        let home = TempDir::new("memory-server-shadowed");
        let bun = home.path().join(".bun").join("bin");
        place_executable(&bun, "harness-memory");
        let env = Env::for_tests(home.path()).with_var("PATH", bun.to_string_lossy());
        let Some(installed) = place_memory_server(home.path()) else {
            return;
        };

        assert_eq!(found(&MemoryServer::lookup(), &env), Some(installed));
    }

    #[test]
    fn a_server_that_reports_no_version_is_not_run_nor_asked_again_until_it_changes() {
        let home = TempDir::new("memory-server-silent");
        let env = Env::for_tests(home.path());
        let silent = place_companion(home.path(), "harness-memory");
        let server = MemoryServer::lookup();

        assert_eq!(found(&server, &env), None);
        assert_eq!(server.lock().rejected.len(), 1, "the silent file is remembered");
        assert_eq!(found(&server, &env), None);
        assert_eq!(server.lock().rejected.len(), 1, "and not asked again");

        // Replaced in place (a later modification time), it is asked again.
        std::fs::remove_file(&silent).expect("remove the silent server");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let Some(installed) = place_memory_server(home.path()) else {
            return;
        };
        assert_eq!(found(&server, &env), Some(installed));
    }

    #[test]
    fn a_state_without_lookup_never_has_a_server() {
        let home = TempDir::new("memory-server-none");
        place_companion(home.path(), "harness-memory");

        let server = MemoryServer::none();
        server.refresh();

        assert_eq!(found(&server, &Env::for_tests(home.path())), None);
    }
}
