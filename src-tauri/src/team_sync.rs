//! Team documents of one signed document kind in the app window (team hooks and team routines, see
//! `harness_business::signed_docs::team`). A [`TeamHost`] (the kind's runtime) owns a [`TeamSync`]
//! and this module refreshes its copies:
//!
//! - Whether the kind is shared comes from `harness-memory <plural> config` ([`MemoryTeamDocs`],
//!   cached until the binary or its config file changes). While it is shared, a refresh lists the
//!   team's documents for the global scope and each known project root (the roots the kind pins,
//!   [`TeamHost::pinned_roots`], then the most recent [`MAX_TEAM_ROOTS`] roots the app used,
//!   starting from `projects.json`), and writes the copies to `<root>/team`.
//! - A refresh runs at launch, every [`TEAM_REFRESH`], when the kind's view asks, after a share or
//!   a removal, after the team database settings are saved, and soon after a new project root is
//!   used ([`spawn`] runs the loop, [`TeamSync::refresh_soon`] wakes it). Nothing waits for it:
//!   the app uses the copies already on disk.
//! - Sharing off, a server without the kind's team documents, or no Harness Memory deletes the team
//!   folder at once, so team documents stop. A server that cannot be asked keeps the copies but
//!   drops trust, so they wait for approval until it answers again.
//!
//! Logs carry ids and counts, never a document's content.

mod backoff;
mod settings;

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use harness_business::agent::rfc3339;
use harness_business::ecosystem::{MemoryTeamDocs, TeamDocsConfig, TeamDocsSupport};
use harness_business::env::Env;
use harness_business::process::TokioRunner;
use harness_business::signed_docs::team::{TeamFolder, apply_round, clear_team, fetch_team, usable_root};
use harness_business::signed_docs::{DocKind, DocStore};
use harness_business::{BusinessError, Result};
use tokio::sync::Notify;

use backoff::Backoff;
pub use settings::TeamSettings;
pub(crate) use settings::team_settings;

/// How often the team copies are refreshed while the kind is shared.
pub const TEAM_REFRESH: Duration = Duration::from_secs(60);

/// The most project roots a refresh asks for.
pub const MAX_TEAM_ROOTS: usize = 32;

/// The runtime of a kind whose team copies [`TeamSync`] refreshes.
pub trait TeamHost: Send + Sync + 'static {
    /// The kind.
    type Kind: DocKind;
    /// What its view shows of the team documents.
    type Settings: TeamSettings;

    /// The team state.
    fn team_sync(&self) -> &TeamSync<Self::Kind, Self::Settings>;

    /// The environment.
    fn env(&self) -> &Env;

    /// The kind's folder; `None` when there is none (nothing is refreshed).
    fn doc_store(&self) -> Option<DocStore<Self::Kind>>;

    /// The lock every write of the app to the folder and the approvals holds.
    fn write_lock(&self) -> Arc<Mutex<()>>;

    /// Reads the documents again on their next use (the copies or the trust changed).
    fn invalidate(&self);

    /// Tells the view something changed.
    fn notify(&self);

    /// Project roots every refresh asks for, before the roots in use and without their limit
    /// (team routines: the folders the routines turned on here run in). Read off the async
    /// runtime; none by default.
    fn pinned_roots(_store: &DocStore<Self::Kind>) -> Vec<String> {
        Vec::new()
    }
}

/// See the module docs.
pub struct TeamSync<K: DocKind, S> {
    probe: MemoryTeamDocs<K>,
    state: Mutex<TeamState<S>>,
    /// Held by a refresh, so one runs at a time.
    refreshing: tokio::sync::Mutex<()>,
    /// Wakes the refresh loop early.
    wake: Notify,
    /// Bumped whenever sharing is found off, so a refresh that started before writes nothing.
    epoch: Arc<AtomicU64>,
}

struct TeamState<S> {
    /// The team documents' state while the kind is shared; `None` otherwise (and before the first
    /// probe), so no team document is used.
    settings: Option<S>,
    /// Project roots, the most recent first.
    roots: VecDeque<String>,
    /// Whether the roots of `projects.json` were added.
    roots_loaded: bool,
    /// Roots a round of this session queried, and not skipped since: only their mapping to a
    /// repository key (`projects.json`) is used, so a folder is never matched on what an earlier
    /// session or an unmounted drive left behind.
    verified: HashSet<String>,
    /// Unverified roots that already woke a refresh since the last round.
    woken: HashSet<String>,
    /// The last probe answered whether the kind is shared (yes or no), not "could not ask".
    known: bool,
}

impl<K: DocKind, S> Default for TeamSync<K, S> {
    fn default() -> Self {
        Self {
            probe: MemoryTeamDocs::new(),
            state: Mutex::new(TeamState {
                settings: None,
                roots: VecDeque::new(),
                roots_loaded: false,
                verified: HashSet::new(),
                woken: HashSet::new(),
                known: false,
            }),
            refreshing: tokio::sync::Mutex::default(),
            wake: Notify::new(),
            epoch: Arc::default(),
        }
    }
}

impl<K: DocKind, S: TeamSettings> TeamSync<K, S> {
    /// The team documents' state for listings; `None` while the kind is not shared.
    pub fn settings(&self) -> Option<S> {
        self.lock().settings.clone()
    }

    /// True once the latest probe of Harness Memory said whether the kind is shared; false before
    /// the first probe and while it cannot be asked (the team documents in use may then be
    /// fewer than the team has).
    pub fn known(&self) -> bool {
        self.lock().known
    }

    /// Whether the installed Harness Memory keeps the kind's team documents, and what its config
    /// says.
    pub async fn support(&self, env: &Env) -> TeamDocsSupport {
        self.probe.support(env, &TokioRunner).await
    }

    /// The installed server while the kind is shared (see [`MemoryTeamDocs::sharing_server`]).
    pub async fn sharing_server(&self, env: &Env) -> Result<(PathBuf, TeamDocsConfig)> {
        self.probe.sharing_server(env, &TokioRunner).await
    }

    /// Notes that `root` (a project root) is in use: refreshes ask for its team documents, and one
    /// not verified this session wakes a refresh (once per round). The filesystem root and the
    /// home folder are not projects and are left out.
    pub fn note_root(&self, env: &Env, root: &str) {
        if !usable_root(env, root) {
            return;
        }

        let mut team = self.lock();

        if team.roots.front().is_none_or(|front| front != root) {
            if let Some(index) = team.roots.iter().position(|known| known == root) {
                team.roots.remove(index);
            }

            team.roots.push_front(root.to_owned());
            team.roots.truncate(MAX_TEAM_ROOTS);
        }

        let wake = team.settings.is_some() && !team.verified.contains(root) && team.woken.insert(root.to_owned());
        drop(team);

        if wake {
            self.refresh_soon();
        }
    }

    /// True when a round of this session queried `root` and has not skipped it since, so its
    /// repository key may be used.
    pub fn root_verified(&self, root: &str) -> bool {
        self.lock().verified.contains(root)
    }

    /// Every root [`TeamSync::root_verified`] holds true for now.
    pub fn verified_roots(&self) -> HashSet<String> {
        self.lock().verified.clone()
    }

    /// Keeps refresh rounds from running while the guard lives (a share or a removal, so a round
    /// that listed before it cannot undo its copy).
    pub async fn hold_rounds(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.refreshing.lock().await
    }

    /// Asks the refresh loop to run now.
    pub fn refresh_soon(&self) {
        self.wake.notify_one();
    }

    /// Sets the latest refresh's problem (and its time after a complete one); true when the
    /// settings changed. Nothing while the kind is not shared.
    fn set_problem(&self, problem: Option<String>, refreshed: bool) -> bool {
        let mut team = self.lock();

        let Some(settings) = team.settings.as_mut() else {
            return false;
        };

        let mut before = settings.clone();
        *settings.problem() = problem;

        if refreshed {
            *settings.refreshed_at() = Some(rfc3339(SystemTime::now()));
        }

        // The time alone is not a change the view needs to hear about.
        *before.problem() != *settings.problem() || before.refreshed_at().is_none() != settings.refreshed_at().is_none()
    }

    /// After a round: the roots that answered are verified, skipped ones no longer are, refused
    /// ones are forgotten, and unverified roots may wake the next round again.
    fn roots_seen(&self, answered: &[String], skipped: &[String], refused: &[String]) {
        let mut team = self.lock();

        team.verified.extend(answered.iter().cloned());

        for root in skipped.iter().chain(refused) {
            team.verified.remove(root);
        }

        team.roots.retain(|root| !refused.contains(root));
        team.woken.clear();
    }

    fn lock(&self) -> MutexGuard<'_, TeamState<S>> {
        // Every write leaves a complete value, so a panic cannot leave it half-written.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Asks the server again whether the kind is shared and applies the answer (no lists): sharing
/// off deletes the copies before this returns. True while it is shared.
pub async fn reprobe<H: TeamHost>(host: &H) -> bool {
    let support = host.team_sync().support(host.env()).await;

    apply_support(host, support).await.is_some()
}

/// One refresh (see the module docs). False when it could not read or save the team documents.
pub async fn refresh<H: TeamHost>(host: &H) -> bool {
    let team = host.team_sync();
    let plural = H::Kind::PLURAL;
    let _refreshing = team.refreshing.lock().await;

    // Taken before the probe: sharing turned off from here on drops this round's answers.
    let started = team.epoch.load(Ordering::SeqCst);
    let support = team.support(host.env()).await;

    if apply_support(host, support).await.is_none() {
        return true;
    }

    let server = match team.sharing_server(host.env()).await {
        Ok((server, _)) => server,
        Err(error) => {
            tracing::warn!(code = ?error.code(), "team {plural} not refreshed");

            if team.set_problem(Some(error.to_string()), false) {
                host.notify();
            }

            return false;
        }
    };

    let Some(store) = host.doc_store() else {
        return true;
    };

    let roots = with_pinned::<H>(team_roots(team, &store).await, &store).await;
    let fetch = fetch_team::<H::Kind>(host.env(), &TokioRunner, &server, &roots).await;

    let writes = host.write_lock();
    let epoch = team.epoch.clone();
    let folder = TeamFolder::of(&store);
    let complete = fetch.complete;
    let problem = fetch.problem.as_ref().map(|error| (error.code(), error.to_string()));
    let rows = fetch.rows.len();
    let answered: Vec<String> = fetch.projects.keys().cloned().collect();
    let skipped = fetch.skipped.clone();
    let refused = fetch.refused.clone();

    let applied = tauri::async_runtime::spawn_blocking(move || {
        let _writing = writes.lock().unwrap_or_else(PoisonError::into_inner);

        // Sharing was turned off while the lists ran: their answers are dropped.
        if epoch.load(Ordering::SeqCst) != started {
            return Ok(None);
        }

        apply_round(&store, &folder, &fetch).map(Some)
    })
    .await;

    let counts = match applied {
        Ok(Ok(Some(counts))) => counts,
        Ok(Ok(None)) | Err(_) => return true,
        Ok(Err(error)) => {
            tracing::warn!(code = ?error.code(), "team {plural} not saved");
            team.set_problem(Some(error.to_string()), false);
            return false;
        }
    };

    match &problem {
        Some((code, _)) => tracing::warn!(?code, rows, "team {plural} refreshed in part"),
        None => tracing::info!(
            rows,
            written = counts.written,
            removed = counts.removed.len(),
            roots = roots.len(),
            "team {plural} refreshed"
        ),
    }

    team.roots_seen(&answered, &skipped, &refused);

    let healthy = problem.is_none();
    let settings_changed = team.set_problem(problem.map(|(_, message)| message), complete);

    if counts.changed() || settings_changed {
        host.invalidate();
        host.notify();
    }

    healthy
}

/// Applies what the server says about the kind's team documents; returns the config while they
/// are shared.
async fn apply_support<H: TeamHost>(host: &H, support: TeamDocsSupport) -> Option<TeamDocsConfig> {
    let team = host.team_sync();
    team.lock().known = support != TeamDocsSupport::Unknown;

    match support {
        TeamDocsSupport::Supported(config) if config.remote => {
            let changed = {
                let mut state = team.lock();
                let before = state.settings.clone();

                let settings = state.settings.get_or_insert_with(|| H::Settings::shared(config.trust));
                *settings.trust() = config.trust;

                before.as_ref() != state.settings.as_ref()
            };

            if changed {
                host.invalidate();
                host.notify();
            }

            Some(config)
        }
        // The copies stay, but nothing is used on trust the server cannot confirm.
        TeamDocsSupport::Unknown => {
            let distrusted = {
                let mut state = team.lock();

                state
                    .settings
                    .as_mut()
                    .is_some_and(|settings| std::mem::replace(settings.trust(), false))
            };
            let problem = team.set_problem(Some("Harness Memory could not be asked".to_owned()), false);

            if distrusted {
                host.invalidate();
            }

            if distrusted || problem {
                host.notify();
            }

            None
        }
        TeamDocsSupport::Supported(_) | TeamDocsSupport::Unsupported | TeamDocsSupport::NotInstalled => {
            stop_sharing(host).await;
            None
        }
    }
}

/// The kind is not shared (any more): the copies are deleted, team documents stop.
async fn stop_sharing<H: TeamHost>(host: &H) {
    let team = host.team_sync();
    let plural = H::Kind::PLURAL;

    team.epoch.fetch_add(1, Ordering::SeqCst);
    let was_on = team.lock().settings.take().is_some();

    let cleared = match host.doc_store() {
        Some(store) => {
            let writes = host.write_lock();

            tauri::async_runtime::spawn_blocking(move || {
                let _writing = writes.lock().unwrap_or_else(PoisonError::into_inner);
                clear_team(&store, &TeamFolder::of(&store))
            })
            .await
            .unwrap_or_else(|error| Err(BusinessError::Internal(error.to_string())))
        }
        None => Ok(false),
    };

    let removed = match cleared {
        Ok(removed) => removed,
        Err(error) => {
            tracing::warn!(code = ?error.code(), "team {plural} folder not removed");
            false
        }
    };

    if removed {
        tracing::info!("team {plural} are not shared, their copies are removed");
    }

    if was_on || removed {
        host.invalidate();
        host.notify();
    }
}

/// The roots a refresh asks for: the ones in use, then those of `projects.json` (once).
async fn team_roots<K: DocKind, S: TeamSettings>(team: &TeamSync<K, S>, store: &DocStore<K>) -> Vec<String> {
    let loaded = team.lock().roots_loaded;

    if !loaded {
        let folder = TeamFolder::of(store);
        let saved = tauri::async_runtime::spawn_blocking(move || folder.projects().into_keys().collect::<Vec<_>>())
            .await
            .unwrap_or_default();

        let mut state = team.lock();
        state.roots_loaded = true;

        for root in saved {
            if state.roots.len() < MAX_TEAM_ROOTS && !state.roots.contains(&root) {
                state.roots.push_back(root);
            }
        }
    }

    // A root that is no longer a folder stays known (its drive may come back): the round skips it
    // and keeps its repository's copies.
    team.lock().roots.iter().cloned().collect()
}

/// `roots` after the roots the kind pins ([`TeamHost::pinned_roots`]), each once.
async fn with_pinned<H: TeamHost>(roots: Vec<String>, store: &DocStore<H::Kind>) -> Vec<String> {
    let store = store.clone();
    let pinned = tauri::async_runtime::spawn_blocking(move || H::pinned_roots(&store))
        .await
        .unwrap_or_default();

    let mut all: Vec<String> = Vec::with_capacity(pinned.len() + roots.len());

    for root in pinned.into_iter().chain(roots) {
        if !all.contains(&root) {
            all.push(root);
        }
    }

    all
}

/// Starts the refresh loop of `host` (it runs until the app exits): a refresh now, then every
/// [`TEAM_REFRESH`] or when woken. After a refresh that failed the wait doubles each time, up to
/// `backoff::TEAM_REFRESH_MAX` (the team database is down: asking every minute only fills the log and the
/// process table); a wake runs one at once and a refresh that worked ends it. Does nothing without the kind's folder.
pub fn spawn<H: TeamHost>(host: Arc<H>) {
    if host.doc_store().is_none() {
        return;
    }

    drop(tauri::async_runtime::spawn(async move {
        let mut backoff = Backoff::default();

        loop {
            let wait = backoff.after(refresh(&*host).await);

            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = host.team_sync().wake.notified() => {}
            }
        }
    }));
}
