//! Team hooks in the app window (see `harness_business::hooks::team`).
//!
//! - Whether hooks are shared comes from `harness-memory hooks config` ([`MemoryHooks`], cached
//!   until the binary or its config file changes). While they are shared, a refresh lists the
//!   team's hooks for the global scope and each known project root (the most recent
//!   [`MAX_TEAM_ROOTS`] roots the Hooks view or a chat used, starting from `projects.json`), and
//!   writes the copies to `<hooks root>/team`.
//! - A refresh runs at launch, every [`TEAM_REFRESH`], when the Hooks view asks
//!   (`hooks_team_refresh`), after a share or a removal, after the team database settings are
//!   saved, and soon after a chat in a new project root starts. Chats never wait for it: they run
//!   the copies already on disk.
//! - Sharing off, a server without team hooks, or no Harness Memory deletes the team folder at
//!   once, so team hooks stop. A server that cannot be asked keeps the copies but drops trust,
//!   so team hooks wait for approval until it answers again.
//!
//! Logs carry ids and counts, never a hook's text, command or body.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use harness_business::agent::rfc3339;
use harness_business::ecosystem::{HooksConfig, HooksSupport, MemoryHooks};
use harness_business::hooks::team::{TeamFolder, apply_round, clear_team, fetch_team, usable_root};
use harness_business::model::TeamHooks;
use harness_business::process::TokioRunner;
use harness_business::{BusinessError, Result};
use tokio::sync::Notify;

use super::HookRuntime;

/// How often the team copies are refreshed while hooks are shared.
pub const TEAM_REFRESH: Duration = Duration::from_secs(60);

/// The most project roots a refresh asks for.
pub const MAX_TEAM_ROOTS: usize = 32;

/// See the module docs.
#[derive(Default)]
pub struct TeamRuntime {
    probe: MemoryHooks,
    state: std::sync::Mutex<TeamState>,
    /// Held by a refresh, so one runs at a time.
    refreshing: tokio::sync::Mutex<()>,
    /// Wakes the refresh loop early.
    wake: Notify,
    /// Bumped whenever sharing is found off, so a refresh that started before writes nothing.
    epoch: Arc<AtomicU64>,
}

#[derive(Default)]
struct TeamState {
    /// The team hooks' state while hooks are shared; `None` otherwise (and before the first
    /// probe), so no team hook runs.
    settings: Option<TeamHooks>,
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
}

impl HookRuntime {
    /// The team hooks' state for listings and the engine; `None` while hooks are not shared.
    pub fn team_settings(&self) -> Option<TeamHooks> {
        self.lock_team().settings.clone()
    }

    /// Whether the installed Harness Memory keeps team hooks, and what its config says.
    pub async fn team_support(&self) -> HooksSupport {
        self.team.probe.support(&self.env, &TokioRunner).await
    }

    /// The installed server while hooks are shared (see [`MemoryHooks::sharing_server`]).
    pub async fn sharing_server(&self) -> Result<(PathBuf, HooksConfig)> {
        self.team.probe.sharing_server(&self.env, &TokioRunner).await
    }

    /// Notes that `root` (a project root) is in use: refreshes ask for its team hooks, and one not
    /// verified this session wakes a refresh (once per round). The filesystem root and the home
    /// folder are not projects and are left out.
    pub fn note_root(&self, root: &str) {
        if !usable_root(&self.env, root) {
            return;
        }

        let mut team = self.lock_team();

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
            self.refresh_team_soon();
        }
    }

    /// True when a round of this session queried `root` and has not skipped it since, so its
    /// repository key may be used.
    pub fn root_verified(&self, root: &str) -> bool {
        self.lock_team().verified.contains(root)
    }

    /// Keeps refresh rounds from running while the guard lives (a share or a removal, so a round
    /// that listed before it cannot undo its copy).
    pub async fn hold_team_rounds(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.team.refreshing.lock().await
    }

    /// Asks the refresh loop to run now.
    pub fn refresh_team_soon(&self) {
        self.team.wake.notify_one();
    }

    /// Asks the server again whether hooks are shared and applies the answer (no lists): sharing
    /// off deletes the copies before this returns. True while hooks are shared.
    pub async fn team_reprobe(&self) -> bool {
        let support = self.team_support().await;

        self.apply_support(support).await.is_some()
    }

    /// One refresh (see the module docs).
    pub async fn refresh_team(&self) {
        let _refreshing = self.team.refreshing.lock().await;

        // Taken before the probe: sharing turned off from here on drops this round's answers.
        let started = self.team.epoch.load(Ordering::SeqCst);
        let support = self.team_support().await;

        if self.apply_support(support).await.is_none() {
            return;
        }

        let server = match self.sharing_server().await {
            Ok((server, _)) => server,
            Err(error) => {
                tracing::warn!(code = ?error.code(), "team hooks not refreshed");

                if self.set_problem(Some(error.to_string()), false) {
                    self.notify();
                }

                return;
            }
        };

        let Some(store) = self.store.clone() else {
            return;
        };

        let roots = self.team_roots(&store).await;
        let fetch = fetch_team(&self.env, &TokioRunner, &server, &roots).await;

        let writes = self.writes.clone();
        let epoch = self.team.epoch.clone();
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
            Ok(Ok(None)) | Err(_) => return,
            Ok(Err(error)) => {
                tracing::warn!(code = ?error.code(), "team hooks not saved");
                self.set_problem(Some(error.to_string()), false);
                return;
            }
        };

        match &problem {
            Some((code, _)) => tracing::warn!(?code, rows, "team hooks refreshed in part"),
            None => tracing::info!(
                rows,
                written = counts.written,
                removed = counts.removed.len(),
                roots = roots.len(),
                "team hooks refreshed"
            ),
        }

        self.roots_seen(&answered, &skipped, &refused);

        let settings_changed = self.set_problem(problem.map(|(_, message)| message), complete);

        if counts.changed() || settings_changed {
            self.invalidate();
            self.notify();
        }
    }

    /// Applies what the server says about team hooks; returns the config while they are shared.
    async fn apply_support(&self, support: HooksSupport) -> Option<HooksConfig> {
        match support {
            HooksSupport::Supported(config) if config.remote => {
                let changed = {
                    let mut team = self.lock_team();
                    let before = team.settings.clone();

                    let settings = team.settings.get_or_insert(TeamHooks {
                        trust: config.trust,
                        problem: None,
                        refreshed_at: None,
                    });
                    settings.trust = config.trust;

                    before.as_ref() != team.settings.as_ref()
                };

                if changed {
                    self.invalidate();
                    self.notify();
                }

                Some(config)
            }
            // The copies stay, but nothing runs on trust the server cannot confirm.
            HooksSupport::Unknown => {
                let distrusted = {
                    let mut team = self.lock_team();

                    team.settings
                        .as_mut()
                        .is_some_and(|settings| std::mem::replace(&mut settings.trust, false))
                };
                let problem = self.set_problem(Some("Harness Memory could not be asked".to_owned()), false);

                if distrusted {
                    self.invalidate();
                }

                if distrusted || problem {
                    self.notify();
                }

                None
            }
            HooksSupport::Supported(_) | HooksSupport::Unsupported | HooksSupport::NotInstalled => {
                self.stop_sharing().await;
                None
            }
        }
    }

    /// Hooks are not shared (any more): the copies are deleted, team hooks stop.
    async fn stop_sharing(&self) {
        self.team.epoch.fetch_add(1, Ordering::SeqCst);
        let was_on = self.lock_team().settings.take().is_some();

        let cleared = match self.store.clone() {
            Some(store) => {
                let writes = self.writes.clone();

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
                tracing::warn!(code = ?error.code(), "team hooks folder not removed");
                false
            }
        };

        if removed {
            tracing::info!("team hooks are not shared, their copies are removed");
        }

        if was_on || removed {
            self.invalidate();
            self.notify();
        }
    }

    /// Sets the latest refresh's problem (and its time after a complete one); true when the
    /// settings changed. Nothing while hooks are not shared.
    fn set_problem(&self, problem: Option<String>, refreshed: bool) -> bool {
        let mut team = self.lock_team();

        let Some(settings) = team.settings.as_mut() else {
            return false;
        };

        let before = settings.clone();
        settings.problem = problem;

        if refreshed {
            settings.refreshed_at = Some(rfc3339(SystemTime::now()));
        }

        // The time alone is not a change the Hooks view needs to hear about.
        before.problem != settings.problem || before.refreshed_at.is_none() != settings.refreshed_at.is_none()
    }

    /// The roots a refresh asks for: the ones in use, then those of `projects.json` (once).
    async fn team_roots(&self, store: &harness_business::hooks::HookStore) -> Vec<String> {
        let loaded = self.lock_team().roots_loaded;

        if !loaded {
            let folder = TeamFolder::of(store);
            let saved = tauri::async_runtime::spawn_blocking(move || folder.projects().into_keys().collect::<Vec<_>>())
                .await
                .unwrap_or_default();

            let mut team = self.lock_team();
            team.roots_loaded = true;

            for root in saved {
                if team.roots.len() < MAX_TEAM_ROOTS && !team.roots.contains(&root) {
                    team.roots.push_back(root);
                }
            }
        }

        // A root that is no longer a folder stays known (its drive may come back): the round
        // skips it and keeps its repository's copies.
        self.lock_team().roots.iter().cloned().collect()
    }

    /// After a round: the roots that answered are verified, skipped ones no longer are, refused
    /// ones are forgotten, and unverified roots may wake the next round again.
    fn roots_seen(&self, answered: &[String], skipped: &[String], refused: &[String]) {
        let mut team = self.lock_team();

        team.verified.extend(answered.iter().cloned());

        for root in skipped.iter().chain(refused) {
            team.verified.remove(root);
        }

        team.roots.retain(|root| !refused.contains(root));
        team.woken.clear();
    }

    fn lock_team(&self) -> MutexGuard<'_, TeamState> {
        // Every write leaves a complete value, so a panic cannot leave it half-written.
        self.team.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Starts the refresh loop (it runs until the app exits): a refresh now, then every
/// [`TEAM_REFRESH`] or when woken. Does nothing without a hooks folder.
pub fn spawn(runtime: Arc<HookRuntime>) {
    if runtime.store.is_none() {
        return;
    }

    drop(tauri::async_runtime::spawn(async move {
        loop {
            runtime.refresh_team().await;

            tokio::select! {
                () = tokio::time::sleep(TEAM_REFRESH) => {}
                () = runtime.team.wake.notified() => {}
            }
        }
    }));
}
