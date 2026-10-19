//! Team routines in the app window (see `harness_business::routines::team` and "Team routines" in
//! `docs/routines-format.md`): [`crate::team_sync`] for routines.
//!
//! - Whether routines are shared comes from `harness-memory routines config` (cached until the
//!   binary or its config file changes). While they are shared, a refresh lists the team's
//!   routines for the global scope, the folders team routines turned on here run in
//!   (`teamEnabled`, pinned so their repository keys stay known), and the other known project
//!   roots (those the Routines view used, starting from `projects.json`), and writes the copies to
//!   `<routines root>/team`.
//! - A refresh runs at launch, every [`TEAM_REFRESH`](crate::team_sync::TEAM_REFRESH), when the
//!   Routines view asks (`routines_team_refresh`), after a share or a removal, and after the team
//!   database settings are saved. The scheduler never waits for it: it uses the copies on disk.
//! - Sharing off, a server without team routines, or no Harness Memory deletes the team folder at
//!   once, so team routines stop. A server that cannot be asked keeps the copies but drops trust,
//!   so team routines wait for approval until it answers again.
//! - Team routines are a catalogue: only one turned on here, and trusted or approved, is
//!   scheduled ([`RoutineRuntime::schedule_views`]). A local routine with the same id wins. A team
//!   project routine runs in the one folder chosen when it was turned on, and Harness Memory is
//!   asked again at each run whether that folder is still a copy of the routine's repository. A
//!   team routine that runs on trust alone runs only while Harness Memory still holds the bytes
//!   of its copy.
//! - When the team routines change (a refresh, sharing off, a routine turned off here or removed)
//!   a team run whose routine may no longer run is stopped, and the run records of team routines
//!   whose copy left are deleted ([`RoutineRuntime::reconcile_team`]).
//!
//! Logs carry ids and counts, never a routine's prompt, command or body.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use harness_business::ecosystem::memory_team_docs::project_key;
use harness_business::env::Env;
use harness_business::model::{HookApproval, HookOrigin, Routine, RoutineView, RoutinesEvent, TeamRoutines};
use harness_business::process::TokioRunner;
use harness_business::routines::state::read_state;
use harness_business::routines::team::{TeamFolder, confirm_team_copy, team_view, team_views};
use harness_business::routines::view::{on_schedule, pause_lost_project};
use harness_business::routines::{RoutineKind, RoutineStore};
use harness_business::signed_docs::Approvals;
use harness_business::{BusinessError, Result};

use super::RoutineRuntime;
use super::job::StopReason;
use super::runner::STOP_BUDGET;
use crate::team_sync::{self, TeamHost, TeamSync};

team_sync::team_settings!(TeamRoutines);

impl TeamHost for RoutineRuntime {
    type Kind = RoutineKind;
    type Settings = TeamRoutines;

    fn team_sync(&self) -> &TeamSync<RoutineKind, TeamRoutines> {
        &self.team
    }

    fn env(&self) -> &Env {
        &self.env
    }

    fn doc_store(&self) -> Option<RoutineStore> {
        self.store.clone()
    }

    fn write_lock(&self) -> Arc<Mutex<()>> {
        self.writes.clone()
    }

    /// Routines keep no cache: the scheduler looks again, first at the team runs going on.
    fn invalidate(&self) {
        self.team_changed.store(true, Ordering::SeqCst);
        self.wake();
    }

    fn notify(&self) {
        self.emit(RoutinesEvent::Changed);
    }

    fn pinned_roots(store: &RoutineStore) -> Vec<String> {
        read_state(store.config_dir())
            .team_folders()
            .map(str::to_owned)
            .collect()
    }
}

impl RoutineRuntime {
    /// The team routines' refresh state.
    pub fn team(&self) -> &TeamSync<RoutineKind, TeamRoutines> {
        &self.team
    }

    /// The team routines' state for listings; `None` while routines are not shared.
    pub fn team_settings(&self) -> Option<TeamRoutines> {
        self.team.settings()
    }

    /// Routine `id` as a run sees it: the local file (Paused while its project folder is gone),
    /// else (while routines are shared) the team copy. `NotFound` when there is neither.
    pub async fn routine_view(&self, id: &str) -> Result<RoutineView> {
        let team = self.team_settings();
        let id = id.to_owned();

        self.read(move |store| match store.get(&id) {
            Err(BusinessError::NotFound(missing)) => match team {
                Some(team) => team_view(store, &TeamFolder::of(store), team.trust, &id),
                None => Err(BusinessError::NotFound(missing)),
            },
            Ok(mut view) => {
                pause_lost_project(&mut view);
                Ok(view)
            }
            failed => failed,
        })
        .await
    }

    /// The routines the scheduler plans from: every local routine file (a project routine whose
    /// folder is gone Paused, so it is not planned) and, while routines are shared, the team routines on their schedule here (turned on, trusted or approved, their
    /// folder in place). A team copy with a local routine's id is a duplicate and never on it.
    /// `None` when the folder or the approvals could not be read: such a listing is incomplete.
    pub(super) async fn schedule_views(&self) -> Result<Option<Vec<RoutineView>>> {
        let team = self.team_settings();

        self.read(move |store| {
            let listing = store.list();

            if listing.folder_problem.is_some() || Approvals::<RoutineKind>::read(store.config_dir()).is_err() {
                return Ok(None);
            }

            let mut views = listing.routines;
            views.iter_mut().for_each(pause_lost_project);

            if let Some(team) = team {
                let teams = team_views(store, &TeamFolder::of(store), team.trust).views;
                views.extend(teams.into_iter().filter(on_schedule));
            }

            Ok(Some(views))
        })
        .await
    }

    /// After the team routines changed: stops the team runs whose routine may no longer run here
    /// (turned off, gone from the team, sharing off, no longer trusted or approved), and deletes
    /// the run records of team routines that have neither a team copy nor a local file any more,
    /// and the workspace folders of routines that are gone.
    pub async fn reconcile_team(self: &Arc<Self>) {
        let mut withdrawn = Vec::new();

        for id in self.team_runs() {
            let allowed = match self.routine_view(&id).await {
                Ok(view) if view.origin == HookOrigin::Team => view.can_run_now,
                // A local routine (one still starting, or the user's copy, which wins).
                Ok(_) => true,
                Err(BusinessError::NotFound(_)) => false,
                // The folder could not be read: decided at the next change.
                Err(_) => true,
            };

            if !allowed {
                withdrawn.push(id);
            }
        }

        if !withdrawn.is_empty() {
            tracing::info!(
                runs = withdrawn.len(),
                "team routine runs stopped: they may no longer run here"
            );
            self.stop_routines(&withdrawn, StopReason::Withdrawn, STOP_BUDGET).await;
        }

        self.delete_gone_team_runs().await;
        self.forget_workspaces_soon().await;
    }

    async fn delete_gone_team_runs(&self) {
        let Ok(store) = self.store() else {
            return;
        };

        let busy = self.busy();
        let deleted = self
            .record(move |runs| {
                // A routines root that is not there (an unmounted drive) says nothing about which
                // routines are gone.
                if !store.root().is_dir() {
                    return Ok(Vec::new());
                }

                let team = TeamFolder::of(&store);
                runs.delete_gone_team_runs(&busy, |id| {
                    has_entry(&store.routines_dir().join(format!("{id}.json")))
                        || has_entry(&team.dir().join(format!("{id}.json")))
                })
            })
            .await;

        match deleted {
            Ok(ids) if ids.is_empty() => {}
            Ok(ids) => {
                tracing::info!(routines = ids.len(), "run records of team routines that left deleted");
                self.forget_schedule(&ids).await;
            }
            Err(error) => tracing::warn!(code = ?error.code(), "run records of team routines that left not deleted"),
        }
    }

    /// For the team routine of `view` about to run, or be copied to the local routines, on trust
    /// alone (its bytes not approved here): the team database still holds the bytes of its copy
    /// ([`confirm_team_copy`]). Otherwise why it cannot go on.
    pub async fn confirm_team_copy(&self, view: &RoutineView) -> std::result::Result<(), &'static str> {
        if view.approval == HookApproval::Approved {
            return Ok(());
        }

        let Ok((server, _)) = self.team.sharing_server(&self.env).await else {
            return Err("Sharing routines with the team is off");
        };

        confirm_team_copy(&self.env, &TokioRunner, &server, view).await
    }

    /// For the team project `routine` about to run in `folder`: Harness Memory still gives that
    /// folder the routine's repository key. Otherwise why the run cannot go on.
    pub(super) async fn confirm_team_folder(
        &self,
        routine: &Routine,
        folder: &Path,
    ) -> std::result::Result<(), &'static str> {
        let Some(repo) = routine.project.as_deref() else {
            return Err("The routine names no repository");
        };

        let Ok((server, _)) = self.team.sharing_server(&self.env).await else {
            return Err("Sharing routines with the team is off");
        };

        match project_key::<RoutineKind>(&self.env, &TokioRunner, &server, folder).await {
            Ok(key) if key.key == repo => Ok(()),
            Ok(_) => Err("The folder is no longer a copy of the routine's repository"),
            Err(error) => {
                tracing::warn!(code = ?error.code(), "team routine folder not confirmed");
                Err("Harness Memory could not confirm the project folder")
            }
        }
    }
}

/// True when something is at `path` (an entry that cannot be read counts, as for approvals).
fn has_entry(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Asks whether routines are shared, then starts `scheduler` (its first look at launch must see
/// the team routines turned on here, so they catch up and app start ones run) and the refresh loop
/// (it runs until the app exits): a refresh now, then every
/// [`TEAM_REFRESH`](crate::team_sync::TEAM_REFRESH) or when woken.
pub fn spawn(runtime: Arc<RoutineRuntime>, scheduler: fn(Arc<RoutineRuntime>)) {
    drop(tauri::async_runtime::spawn(async move {
        team_sync::reprobe(&*runtime).await;

        scheduler(runtime.clone());
        team_sync::spawn(runtime);
    }));
}
