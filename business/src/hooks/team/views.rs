//! The status of each team hook, and the app window's listing (local hooks, then team hooks).
//!
//! A team hook is checked in this order: Invalid (a body that breaks the team rules, or an
//! envelope that cannot be used), Duplicate (a local file has its id: the local one wins), Pending
//! ("Trust team hooks" is off and these exact bytes are not approved), Off (the body says
//! `enabled: false`, or the user turned it off on this machine), Paused (a requirement is not
//! met), else Active.

use std::collections::{BTreeMap, BTreeSet};

use super::{ProjectKey, TeamFolder};
use crate::error::{BusinessError, Result};
use crate::hooks::HookKind;
use crate::hooks::state::{HooksState, read_state};
use crate::hooks::store::{HookCapabilities, HookStore};
use crate::hooks::view::local_defaults;
use crate::model::{
    Hook, HookApproval, HookListing, HookOrigin, HookScope, HookStatus, HookView, ShareBlock, TeamHooks,
};
use crate::signed_docs::listing::{ListingScope, share_block};
use crate::signed_docs::team::entries::{TeamEntry, team_entries};

/// Every team hook with its status, and the repository key of each known project root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamViews {
    /// Valid ones in run order (global, then project, then `createdAt`, then `id`), invalid ones
    /// after them by id.
    pub views: Vec<HookView>,
    /// `projects.json`: the repository key of each project root a list was asked for.
    pub projects: BTreeMap<String, ProjectKey>,
}

impl TeamViews {
    /// The repository key of the project root `root`, when a list was asked for it.
    pub fn key_of(&self, root: Option<&str>) -> Option<&str> {
        self.projects.get(root?).map(|project| project.key.as_str())
    }

    /// The team hooks a listing for a chat in the repository `key` shows: the global ones, those
    /// of that repository, and every invalid one (its scope is unknown).
    pub fn shown_in<'a>(&'a self, key: Option<&'a str>) -> impl Iterator<Item = &'a HookView> {
        self.views.iter().filter(move |view| match &view.hook {
            None => true,
            Some(hook) => hook.scope == HookScope::Global || (key.is_some() && hook.project.as_deref() == key),
        })
    }
}

/// Every team hook in `folder` with its status: approvals and local overrides from `store`'s
/// config folder, duplicates from its hooks folder. `trust`: "Trust team hooks" is on.
pub fn team_views(store: &HookStore, folder: &TeamFolder, capabilities: HookCapabilities, trust: bool) -> TeamViews {
    let state = read_state(store.config_dir());
    let approvals = store.read_approvals();

    let views = team_entries(store, folder, &state, &approvals)
        .into_iter()
        .map(|entry| view_of(entry, &state, capabilities, trust))
        .collect();

    TeamViews {
        views,
        projects: folder.projects(),
    }
}

/// Team hook `id` as [`team_views`] shows it (`NotFound` when there is no copy).
pub fn team_view(
    store: &HookStore,
    folder: &TeamFolder,
    capabilities: HookCapabilities,
    trust: bool,
    id: &str,
) -> Result<HookView> {
    team_views(store, folder, capabilities, trust)
        .views
        .into_iter()
        .find(|view| view.id == id)
        .ok_or_else(|| BusinessError::NotFound("the remote hook".to_owned()))
}

/// The app window's listing for a chat in `project` (a project root; `None`: global only): the
/// local hooks as [`HookStore::list_for`] shows them, each with why it cannot be shared, then,
/// while `team` is set (sharing is on), the team hooks of that chat ([`TeamViews::shown_in`]).
/// The project's team hooks are shown only while `verified` (a refresh of this session confirmed
/// the root's repository key), its global team hooks always.
pub fn app_listing(
    store: &HookStore,
    folder: &TeamFolder,
    capabilities: HookCapabilities,
    project: Option<&str>,
    team: Option<&TeamHooks>,
    verified: bool,
) -> HookListing {
    let project_verified = |root: &str| verified && project == Some(root);
    let scope = ListingScope {
        project,
        all: false,
        verified: &project_verified,
    };

    listing_in(store, folder, capabilities, &scope, team)
}

/// The app window's listing of every hook: every local file as [`HookStore::list`] shows it, each
/// with why it cannot be shared for its own root, then, while `team` is set, every team hook. A
/// row is matched to a project root only by a root `verified` this session; `project` (the active
/// project root) is preferred.
pub fn app_listing_all(
    store: &HookStore,
    folder: &TeamFolder,
    capabilities: HookCapabilities,
    project: Option<&str>,
    team: Option<&TeamHooks>,
    verified: &dyn Fn(&str) -> bool,
) -> HookListing {
    let scope = ListingScope {
        project,
        all: true,
        verified,
    };

    listing_in(store, folder, capabilities, &scope, team)
}

/// The listing of `scope`, each row with its project root and repository key.
fn listing_in(
    store: &HookStore,
    folder: &TeamFolder,
    capabilities: HookCapabilities,
    scope: &ListingScope<'_>,
    team: Option<&TeamHooks>,
) -> HookListing {
    let mut listing = if scope.all {
        store.list(capabilities)
    } else {
        store.list_for(capabilities, scope.project)
    };

    let Some(settings) = team else {
        for view in &mut listing.hooks {
            view.share_block = local_share_block(view, &BTreeMap::new(), &BTreeSet::new());
        }

        return listing;
    };

    let team_views = team_views(store, folder, capabilities, settings.trust);
    let team_ids: BTreeSet<String> = team_views.views.iter().map(|view| view.id.clone()).collect();

    for view in &mut listing.hooks {
        view.share_block = local_share_block(view, &team_views.projects, &team_ids);
    }

    if scope.all {
        listing.hooks.extend(team_views.views.iter().cloned());
    } else {
        let key = team_views.key_of(scope.verified_project());
        listing.hooks.extend(team_views.shown_in(key).cloned());
    }

    for view in &mut listing.hooks {
        (view.root, view.repo_key) = scope.repo_of(
            view.origin,
            view.root.as_deref(),
            view.team_repo.as_deref(),
            &team_views.projects,
        );
    }

    listing.team = Some(settings.clone());

    listing
}

/// Why the local hook of `view` cannot be shared: invalid, a Harness family hook, waiting for
/// review, a project without an `origin` remote (by `projects`), or already on the team
/// (`team_ids`).
fn local_share_block(
    view: &HookView,
    projects: &BTreeMap<String, ProjectKey>,
    team_ids: &BTreeSet<String>,
) -> Option<ShareBlock> {
    let pending = view.status == HookStatus::Pending;

    share_block::<HookKind>(view.hook.as_ref(), pending, projects, team_ids)
}

/// The view of one team copy.
fn view_of(entry: TeamEntry<HookKind>, state: &HooksState, capabilities: HookCapabilities, trust: bool) -> HookView {
    let team_disabled = entry.row.is_some() && state.team_disabled(&entry.id);

    let mut view = HookView {
        id: entry.id,
        name: entry.name,
        sha256: entry.sha256,
        approval: entry.approval,
        approved: entry.approved,
        origin: HookOrigin::Team,
        team_author: entry.author,
        team_updated_at: entry.row.as_ref().map(|row| row.updated_at.clone()),
        team_repo: entry.row.and_then(|row| row.repo),
        team_changed: entry.local_changed,
        team_disabled,
        ..local_defaults()
    };

    match entry.doc {
        Ok(hook) => {
            view.name = Some(hook.name.clone());
            view.status = status_of(
                &hook,
                entry.duplicate,
                entry.approval,
                team_disabled,
                capabilities,
                trust,
            );
            view.hook = Some(hook);
        }
        Err(problem) => view.problem = Some(problem),
    }

    view
}

/// Invalid aside, a team hook is a duplicate, pending, off, waiting for a requirement or active.
fn status_of(
    hook: &Hook,
    duplicate: bool,
    approval: HookApproval,
    disabled: bool,
    capabilities: HookCapabilities,
    trust: bool,
) -> HookStatus {
    if duplicate {
        return HookStatus::Duplicate;
    }

    if !trust && approval != HookApproval::Approved {
        return HookStatus::Pending;
    }

    if !hook.enabled || disabled {
        return HookStatus::Off;
    }

    if !capabilities.allows(hook) {
        return HookStatus::Paused;
    }

    HookStatus::Active
}
