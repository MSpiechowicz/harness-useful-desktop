//! The status of each team routine, and the app window's listing (local routines, then team
//! routines). See the order in the [module docs](super).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{ProjectKey, TeamFolder};
use crate::error::{BusinessError, Result};
use crate::model::{
    HookApproval, HookOrigin, HookScope, Routine, RoutineListing, RoutineStatus, RoutineView, TeamRoutines,
};
use crate::routines::RoutineKind;
use crate::routines::state::{RoutinesState, read_state};
use crate::routines::store::RoutineStore;
use crate::routines::view::{blank_view, counts_of};
use crate::signed_docs::listing::share_block;
use crate::signed_docs::team::entries::{TeamEntry, team_entries};

/// Every team routine with its status, and the repository key of each known project root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamRoutineViews {
    /// Valid ones in team order (global, then project, then `createdAt`, then `id`), invalid ones
    /// after them by id.
    pub views: Vec<RoutineView>,
    /// `projects.json`: the repository key of each project root a list was asked for.
    pub projects: BTreeMap<String, ProjectKey>,
}

impl TeamRoutineViews {
    /// The team routines a listing for a project with the repository `key` shows: the global
    /// ones, those of that repository, every one turned on here (it runs whatever is open), and
    /// every invalid one (its scope is unknown).
    pub fn shown_in<'a>(&'a self, key: Option<&'a str>) -> impl Iterator<Item = &'a RoutineView> {
        self.views.iter().filter(move |view| match &view.routine {
            None => true,
            Some(routine) => {
                let here = key.is_some() && routine.project.as_deref() == key;
                routine.scope == HookScope::Global || here || view.team_on
            }
        })
    }
}

/// What every team view is checked against.
struct Context<'a> {
    state: &'a RoutinesState,
    projects: &'a BTreeMap<String, ProjectKey>,
    trust: bool,
}

/// Every team routine in `folder` with its status: approvals and `teamEnabled` from `store`'s
/// config folder, duplicates from its routines folder. `trust`: "Run team routines without
/// asking" is on.
pub fn team_views(store: &RoutineStore, folder: &TeamFolder, trust: bool) -> TeamRoutineViews {
    let state = read_state(store.config_dir());
    let approvals = store.read_approvals();
    let projects = folder.projects();

    let context = Context {
        state: &state,
        projects: &projects,
        trust,
    };

    let views = team_entries(store, folder, &state, &approvals)
        .into_iter()
        .map(|entry| context.view_of(entry))
        .collect();

    TeamRoutineViews { views, projects }
}

/// Team routine `id` as [`team_views`] shows it (`NotFound` when there is no copy).
pub fn team_view(store: &RoutineStore, folder: &TeamFolder, trust: bool, id: &str) -> Result<RoutineView> {
    team_views(store, folder, trust)
        .views
        .into_iter()
        .find(|view| view.id == id)
        .ok_or_else(|| BusinessError::NotFound("the team routine".to_owned()))
}

/// The app window's listing for `project` (a project root; `None`: global only): the local
/// routines as [`RoutineStore::list_for`] shows them, each with why it cannot be shared, then,
/// while `team` is set (sharing is on), the team routines ([`TeamRoutineViews::shown_in`]). The
/// project's team routines are shown only while `verified` (a refresh of this session confirmed
/// the root's repository key).
pub fn app_listing(
    store: &RoutineStore,
    folder: &TeamFolder,
    project: Option<&str>,
    team: Option<&TeamRoutines>,
    verified: bool,
) -> RoutineListing {
    let mut listing = store.list_for(project);

    let (projects, team_ids, team_views) = match team {
        Some(settings) => {
            let views = team_views(store, folder, settings.trust);
            let ids: BTreeSet<String> = views.views.iter().map(|view| view.id.clone()).collect();

            (views.projects.clone(), ids, Some(views))
        }
        None => (BTreeMap::new(), BTreeSet::new(), None),
    };

    for view in &mut listing.routines {
        let pending = view.status == RoutineStatus::Pending;
        view.share_block = share_block::<RoutineKind>(view.routine.as_ref(), pending, &projects, &team_ids);
    }

    if let (Some(views), Some(settings)) = (team_views, team) {
        let root = project.filter(|_| verified);
        let key = root
            .and_then(|root| views.projects.get(root))
            .map(|key| key.key.as_str());

        listing.routines.extend(views.shown_in(key).cloned());
        listing.team = Some(settings.clone());
    }

    listing.counts = counts_of(&listing.routines);
    listing
}

impl Context<'_> {
    fn view_of(&self, entry: TeamEntry<RoutineKind>) -> RoutineView {
        let turned_on = entry.row.as_ref().and_then(|_| self.state.team_enabled(&entry.id));

        let mut view = RoutineView {
            id: entry.id,
            name: entry.name,
            sha256: entry.sha256,
            approval: entry.approval,
            approved: entry.approved,
            team_author: entry.author,
            team_updated_at: entry.row.as_ref().map(|row| row.updated_at.clone()),
            team_repo: entry.row.and_then(|row| row.repo),
            team_changed: entry.local_changed,
            team_on: turned_on.is_some(),
            team_folder: turned_on.flatten().map(str::to_owned),
            ..blank_view(HookOrigin::Team)
        };

        let routine = match entry.doc {
            Ok(routine) => routine,
            Err(problem) => {
                view.problem = Some(problem);
                return view;
            }
        };

        let may_run = may_run(entry.approval, self.trust);
        let folder_ok = self.folder_ok(&routine, turned_on);

        view.status = if entry.duplicate {
            RoutineStatus::Duplicate
        } else if turned_on.is_none() || !routine.enabled {
            RoutineStatus::Off
        } else if !may_run {
            RoutineStatus::Pending
        } else if !folder_ok {
            RoutineStatus::Paused
        } else {
            RoutineStatus::Active
        };

        view.can_run_now = !entry.duplicate && turned_on.is_some() && may_run && folder_ok;
        view.runs_unattended = routine.action.runs_unattended();
        view.name = Some(routine.name.clone());
        view.routine = Some(routine);

        view
    }

    /// A project routine runs in the folder chosen when it was turned on: still a folder, and
    /// still a root of the routine's repository (by `projects.json`, which each refresh keeps).
    fn folder_ok(&self, routine: &Routine, turned_on: Option<Option<&str>>) -> bool {
        if routine.scope == HookScope::Global {
            return true;
        }

        let Some(Some(root)) = turned_on else {
            return false;
        };

        let same_repo = self
            .projects
            .get(root)
            .is_some_and(|key| Some(key.key.as_str()) == routine.project.as_deref());

        same_repo && Path::new(root).is_dir()
    }
}

/// The trust rule: a team routine may run with "Run team routines without asking" on (`trust`,
/// which covers every team routine, one that runs tools without asking included) or its exact
/// bytes approved here.
pub(crate) fn may_run(approval: HookApproval, trust: bool) -> bool {
    trust || approval == HookApproval::Approved
}
