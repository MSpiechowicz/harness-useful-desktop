//! Hooks in desktop chats (see `harness_business::hooks` and `docs/hooks-format.md`).
//!
//! - [`HookRuntime`] (in [`AppState`]) owns the hooks folder and the approvals: it keeps the
//!   hooks read once until the folder, the approvals or the capabilities change, serializes the
//!   app's writes, and tells the Hooks view about changes (`hooks_listen`).
//! - [`chat`]: every chat but a utility agent gets a `HookSession` from `agent_start` until it
//!   stops, its channel closes or the page reloads. Each send goes through it under the chat's
//!   own lock, and its events reach it from a [`chat::HookSink`] around the webview channel (or
//!   the relay of a routine run, `crate::routines::relay`).
//! - [`watch`]: a thread that looks at the folder and the approvals every second
//!   ([`crate::doc_watch`]).
//! - [`team`]: the team hooks shared through Harness Memory: whether hooks are shared, the
//!   refresh of their copies ([`crate::team_sync`]), and the copies the chats run after the local
//!   hooks.
//! - Capabilities: `harness-memory` is met while Harness Memory is installed and the
//!   `memoryEnabled` pref is on, `harness-dashboard` while Harness Dashboard is installed. Looked
//!   up at most every [`MEMBERS_TTL`], and again after an install or the memory switch.
//! - Harness family hooks: seeded once at launch for the members available
//!   ([`spawn_on_launch`]), and a member's missing ones are added again after the app installed it
//!   ([`after_member_install`]).
//!
//! - App exit waits up to [`SESSION_END_BUDGET`] (inside the exit budget) for the chats' hook
//!   tasks to handle their exits, so `sessionEnd` commands get to run ([`HookRuntime::finish_chats`]).
//!   Commands still running after that are stopped. A page reload does not end the process: the
//!   tasks finish in the background.
//!
//! Logs carry hook ids and counts, never a hook's text, command or output.

pub mod chat;
pub mod team;
#[cfg(test)]
mod team_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_opencode;
pub mod watch;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use harness_business::ecosystem::{Companion, installed_memory, resolve_dashboard};
use harness_business::env::Env;
use harness_business::hooks::team::{TeamFolder, TeamViews, team_views};
use harness_business::hooks::{HookCapabilities, HookKind, HookSet, HookStore, restore_member, seed_defaults};
use harness_business::model::{HookRequirement, HookView, HooksEvent, TeamHooks};
use harness_business::process::TokioRunner;
use harness_business::{BusinessError, Result};
use tauri::async_runtime::JoinHandle;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use self::chat::ChatHooks;
use crate::doc_watch::DocWatch;
use crate::state::{AppState, SharedPrefs};
use crate::team_sync::TeamSync;

/// How long a lookup of the installed family members is used.
pub const MEMBERS_TTL: Duration = Duration::from_secs(30);

/// Longest wait at exit for the chats' `sessionEnd` commands (within the exit budget).
pub const SESSION_END_BUDGET: Duration = Duration::from_secs(3);

/// See the module docs.
pub struct HookRuntime {
    store: Option<HookStore>,
    prefs: SharedPrefs,
    env: Arc<Env>,
    chats: Mutex<HashMap<String, Arc<ChatHooks>>>,
    cache: Mutex<Cache>,
    listener: Mutex<Option<Channel<HooksEvent>>>,
    /// Held by every write of the app (commands, seeding, pruning), so approvals are never lost
    /// to two writers that each load and save them.
    writes: Arc<Mutex<()>>,
    watch: DocWatch,
    /// The chats' hook tasks; each ends after its agent exited (and its `sessionEnd` ran).
    tasks: Mutex<Vec<JoinHandle<()>>>,
    /// The team hooks (see [`team`]).
    team: TeamSync<HookKind, TeamHooks>,
}

#[derive(Default)]
struct Cache {
    members: Option<(Instant, Members)>,
    views: Option<(HookCapabilities, Arc<RunViews>)>,
    /// Bumped by every invalidation, so a read that started before one does not store its result.
    generation: u64,
}

/// What the chats' hooks are picked from.
#[derive(Default)]
struct RunViews {
    /// Every local hook file.
    local: Vec<HookView>,
    /// Every team hook, while hooks are shared.
    team: TeamViews,
}

/// The family members on this machine.
#[derive(Debug, Clone, Copy, Default)]
struct Members {
    memory_installed: bool,
    memory_enabled: bool,
    dashboard_installed: bool,
}

impl Members {
    /// What the hooks' `requires` are checked against.
    fn capabilities(self) -> HookCapabilities {
        HookCapabilities {
            memory_available: self.memory_installed && self.memory_enabled,
            dashboard_available: self.dashboard_installed,
        }
    }

    /// Which members are installed (Harness Memory whatever its setting).
    fn installed(self) -> HookCapabilities {
        HookCapabilities {
            memory_available: self.memory_installed,
            dashboard_available: self.dashboard_installed,
        }
    }
}

impl HookRuntime {
    /// Hooks in `store` (`None`: no hooks folder, hooks are unavailable), with the
    /// `memoryEnabled` pref read from `prefs`.
    pub fn new(store: Option<HookStore>, prefs: SharedPrefs, env: Arc<Env>) -> Self {
        Self {
            store,
            prefs,
            env,
            chats: Mutex::default(),
            cache: Mutex::default(),
            listener: Mutex::default(),
            writes: Arc::default(),
            watch: DocWatch::default(),
            tasks: Mutex::default(),
            team: TeamSync::default(),
        }
    }

    /// The hooks folder; `unsupported` when there is none (Windows without `LOCALAPPDATA`).
    pub fn store(&self) -> Result<HookStore> {
        self.store
            .clone()
            .ok_or_else(|| BusinessError::Unsupported("hooks need a local data folder".to_owned()))
    }

    /// What the hooks' `requires` are checked against now.
    pub async fn capabilities(&self) -> HookCapabilities {
        self.members().await.capabilities()
    }

    /// Which family members are installed (Harness Memory whatever its setting).
    pub async fn installed(&self) -> HookCapabilities {
        self.members().await.installed()
    }

    /// Looks the family members up again on the next use (after an install or the memory
    /// switch); the active hooks are read again too.
    pub fn refresh_members(&self) {
        let mut cache = self.lock_cache();
        cache.members = None;
        cache.views = None;
        cache.generation += 1;
    }

    /// Reads the hooks again on their next firing (the folder, the approvals or the team hooks
    /// changed).
    pub fn invalidate(&self) {
        let mut cache = self.lock_cache();
        cache.views = None;
        cache.generation += 1;
    }

    async fn members(&self) -> Members {
        let (cached, generation) = {
            let cache = self.lock_cache();
            let fresh = cache.members.filter(|(at, _)| at.elapsed() < MEMBERS_TTL);

            (fresh.map(|(_, members)| members), cache.generation)
        };

        if let Some(members) = cached {
            return members;
        }

        let env = self.env.clone();
        let prefs = self.prefs.clone();

        let local = tauri::async_runtime::spawn_blocking(move || {
            let memory_enabled = prefs.with(|store| store.load().memory_enabled);

            (memory_enabled, resolve_dashboard(&env).is_some())
        })
        .await;
        let (memory_enabled, dashboard_installed) = local.unwrap_or_default();
        let memory_installed = installed_memory(&self.env, &TokioRunner).await.is_some();

        let members = Members {
            memory_installed,
            memory_enabled,
            dashboard_installed,
        };

        let mut cache = self.lock_cache();
        if cache.generation == generation {
            cache.members = Some((Instant::now(), members));
        }

        members
    }

    /// Every hook file and team hook with its status, read once until something changes.
    async fn views(&self) -> Arc<RunViews> {
        let capabilities = self.capabilities().await;

        let generation = {
            let cache = self.lock_cache();

            if let Some((cached, views)) = &cache.views
                && *cached == capabilities
            {
                return views.clone();
            }

            cache.generation
        };

        // Read after the generation: a trust change invalidates, so a read with the old trust is
        // not stored.
        let trust = self.team_settings().map(|settings| settings.trust);

        let Some(store) = self.store.clone() else {
            return Arc::default();
        };

        let listed = tauri::async_runtime::spawn_blocking(move || RunViews {
            local: store.list(capabilities).hooks,
            team: match trust {
                Some(trust) => team_views(&store, &TeamFolder::of(&store), capabilities, trust),
                None => TeamViews::default(),
            },
        })
        .await;
        let views = Arc::new(listed.unwrap_or_default());

        let mut cache = self.lock_cache();
        if cache.generation == generation {
            cache.views = Some((capabilities, views.clone()));
        }

        views
    }

    /// The active hooks of a chat in `project` (its project root; `None`: global hooks only): its
    /// local hooks, then the team hooks of its repository.
    pub async fn hook_set(&self, project: Option<&str>) -> HookSet {
        if self.store.is_none() {
            return HookSet::empty();
        }

        if let Some(root) = project {
            self.note_root(root);
        }

        let views = self.views().await;

        // A root's repository key counts once a round of this session confirmed it.
        let key = project
            .filter(|root| self.root_verified(root))
            .and_then(|root| views.team.key_of(Some(root)));

        HookSet::for_chat(&views.local, project).with_team(&views.team.views, project, key)
    }

    /// Runs `task` on the store off the async runtime (file IO) and without the write lock.
    pub async fn read<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&HookStore) -> Result<T> + Send + 'static,
    {
        let store = self.store()?;

        tauri::async_runtime::spawn_blocking(move || task(&store))
            .await
            .map_err(|error| BusinessError::Internal(format!("hooks task failed: {error}")))?
    }

    /// Runs the write `task` on the store off the async runtime, one write at a time; the hooks
    /// are read again afterwards.
    pub async fn write<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&HookStore) -> Result<T> + Send + 'static,
    {
        let store = self.store()?;
        let writes = self.writes.clone();

        let written = tauri::async_runtime::spawn_blocking(move || {
            let _writing = writes.lock().unwrap_or_else(PoisonError::into_inner);
            task(&store)
        })
        .await
        .map_err(|error| BusinessError::Internal(format!("hooks task failed: {error}")));

        self.invalidate();
        written?
    }

    /// Sends the changes to `channel`, replacing the previous listener.
    pub fn listen(&self, channel: Channel<HooksEvent>) {
        *self.listener.lock().unwrap_or_else(PoisonError::into_inner) = Some(channel);
    }

    /// Tells the listener the hooks changed; a listener whose page is gone is dropped.
    fn notify(&self) {
        let mut listener = self.listener.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(channel) = listener.as_ref()
            && channel.send(HooksEvent::Changed).is_err()
        {
            *listener = None;
        }
    }

    /// The hook state of chat `key`, while it is tracked.
    pub fn chat(&self, key: &str) -> Option<Arc<ChatHooks>> {
        self.lock_chats().get(key).cloned()
    }

    /// Stops tracking chat `key` (it stopped or its channel closed). A `sessionEnd` still fires
    /// from its `exited` event.
    pub fn forget(&self, key: &str) {
        self.lock_chats().remove(key);
    }

    /// Stops tracking every chat but those in `keep` (the page reloaded, so every other chat
    /// stopped).
    pub fn clear_except(&self, keep: &HashSet<String>) {
        self.lock_chats().retain(|key, _| keep.contains(key));
    }

    /// Tracks `chat` unless it is no longer in `live` (it stopped while it started). Forgets the
    /// chats that stopped without a stop call. True when it is tracked.
    fn insert(&self, chat: Arc<ChatHooks>, live: &HashSet<String>) -> bool {
        let mut chats = self.lock_chats();
        chats.retain(|key, _| live.contains(key));

        if !live.contains(chat.key()) {
            return false;
        }

        chats.insert(chat.key().to_owned(), chat);
        true
    }

    /// Keeps the hook task of a chat, for [`HookRuntime::finish_chats`]; finished ones are dropped.
    pub fn keep_task(&self, task: JoinHandle<()>) {
        let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);

        tasks.retain(|task| !task.inner().is_finished());
        tasks.push(task);
    }

    /// Waits at most `budget` for every chat's hook task to end (their agents are stopping, so
    /// each handles its exit and runs its `sessionEnd` commands), then stops the ones left, which
    /// stops the commands they run. Returns how many were stopped.
    pub async fn finish_chats(&self, budget: Duration) -> usize {
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap_or_else(PoisonError::into_inner));
        let deadline = tokio::time::Instant::now() + budget;
        let mut stopped = 0;

        for mut task in tasks {
            if tokio::time::timeout_at(deadline, &mut task).await.is_ok() {
                continue;
            }

            // Dropping the task's future kills the command it waits for.
            task.abort();
            task.await.ok();
            stopped += 1;
        }

        if stopped > 0 {
            tracing::warn!(stopped, "hook commands were stopped at exit");
        }

        stopped
    }

    /// Forgets `chat` when it is still the tracked one of its key (its agent exited).
    fn forget_exited(&self, chat: &Arc<ChatHooks>) {
        let mut chats = self.lock_chats();

        if chats.get(chat.key()).is_some_and(|tracked| Arc::ptr_eq(tracked, chat)) {
            chats.remove(chat.key());
        }
    }

    fn lock_chats(&self) -> MutexGuard<'_, HashMap<String, Arc<ChatHooks>>> {
        // The map holds no invariant a panic could break.
        self.chats.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_cache(&self) -> MutexGuard<'_, Cache> {
        // Every write leaves a complete value, so a panic cannot leave it half-written.
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Seeds the Harness family hooks of the available members in the background, and starts the
/// watcher and the team hooks refresh; the window does not wait for any of them.
pub fn spawn_on_launch(app: &AppHandle) {
    let state = app.state::<AppState>();
    watch::spawn(state.hooks.clone());
    team::spawn(state.hooks.clone());

    let app = app.clone();
    drop(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        seed_available(&state.hooks).await;
    }));
}

/// Launch seeding: the family hooks of every available member that were never seeded.
pub async fn seed_available(runtime: &HookRuntime) {
    let available = runtime.capabilities().await;

    if available == HookCapabilities::default() {
        return;
    }

    let seeded = runtime
        .write(move |store| seed_defaults(store, available, SystemTime::now()))
        .await;

    match seeded {
        Ok(seeds) => tracing::info!(created = seeds.len(), "harness family hooks seeded"),
        Err(error) => tracing::warn!(code = ?error.code(), "harness family hooks not seeded"),
    }
}

/// After the app installed `companion`: its missing family hooks are added again (even ones the
/// user deleted), never overwriting one that exists.
pub async fn after_member_install(runtime: &HookRuntime, companion: Companion) {
    runtime.refresh_members();

    let member = match companion {
        Companion::Memory => HookRequirement::HarnessMemory,
        Companion::Dashboard => HookRequirement::HarnessDashboard,
    };

    let restored = runtime
        .write(move |store| restore_member(store, member, SystemTime::now()))
        .await;

    match restored {
        Ok(seeds) => tracing::info!(?member, added = seeds.len(), "harness family hooks restored"),
        Err(error) => tracing::warn!(?member, code = ?error.code(), "harness family hooks not restored"),
    }
}
