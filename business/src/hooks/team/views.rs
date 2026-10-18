//! The status of each team hook, and the app window's listing (local hooks, then team hooks).
//!
//! A team hook is checked in this order: Invalid (a body that breaks the team rules, or an
//! envelope that cannot be used), Duplicate (a local file has its id: the local one wins), Pending
//! ("Trust team hooks" is off and these exact bytes are not approved), Off (the body says
//! `enabled: false`, or the user turned it off on this machine), Paused (a requirement is not
//! met), else Active.

use std::collections::{BTreeMap, BTreeSet};

use super::body::parse_team_hook;
use super::folder::{TeamFile, TeamFolder};
use super::{ProjectKey, TeamRow};
use crate::error::{BusinessError, Result};
use crate::hooks::approvals::{Approvals, approved_bytes};
use crate::hooks::folder::read_file;
use crate::hooks::format::{MAX_NAME_CHARS, loose_name};
use crate::hooks::sha256_hex;
use crate::hooks::state::{HooksState, read_state};
use crate::hooks::store::{HookCapabilities, HookStore};
use crate::hooks::unicode::is_hidden_char;
use crate::hooks::view::{approval_of, earlier_version, local_defaults, team_order_key};
use crate::model::{
    Hook, HookApproval, HookListing, HookOrigin, HookScope, HookStatus, HookView, ShareBlock, TeamHooks,
};

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

/// What every team view is checked against.
struct Context<'a> {
    store: &'a HookStore,
    local_ids: BTreeSet<String>,
    approvals: Approvals,
    state: HooksState,
    capabilities: HookCapabilities,
    trust: bool,
}

/// Every team hook in `folder` with its status: approvals and local overrides from `store`'s
/// config folder, duplicates from its hooks folder. `trust`: "Trust team hooks" is on.
pub fn team_views(store: &HookStore, folder: &TeamFolder, capabilities: HookCapabilities, trust: bool) -> TeamViews {
    let context = Context {
        store,
        local_ids: store.entry_ids().unwrap_or_default(),
        approvals: store.read_approvals(),
        state: read_state(store.config_dir()),
        capabilities,
        trust,
    };

    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for file in folder.read() {
        let view = context.view_of(file);

        match view.hook.as_ref().map(team_order_key) {
            Some(order) => valid.push((order, view)),
            None => invalid.push(view),
        }
    }

    valid.sort_by(|left, right| left.0.cmp(&right.0));

    let mut views: Vec<HookView> = valid.into_iter().map(|(_, view)| view).collect();
    views.extend(invalid);

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
        .ok_or_else(|| BusinessError::NotFound("the team hook".to_owned()))
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
    let mut listing = store.list_for(capabilities, project);

    let Some(settings) = team else {
        for view in &mut listing.hooks {
            view.share_block = share_block(view, &BTreeMap::new(), &BTreeSet::new());
        }

        return listing;
    };

    let team_views = team_views(store, folder, capabilities, settings.trust);
    let team_ids: BTreeSet<String> = team_views.views.iter().map(|view| view.id.clone()).collect();

    for view in &mut listing.hooks {
        view.share_block = share_block(view, &team_views.projects, &team_ids);
    }

    let key = team_views.key_of(project.filter(|_| verified));
    listing.hooks.extend(team_views.shown_in(key).cloned());
    listing.team = Some(settings.clone());

    listing
}

/// Why the local hook of `view` cannot be shared: invalid, a Harness family hook, waiting for
/// review, a project without an `origin` remote (by `projects`), or already on the team
/// (`team_ids`).
fn share_block(
    view: &HookView,
    projects: &BTreeMap<String, ProjectKey>,
    team_ids: &BTreeSet<String>,
) -> Option<ShareBlock> {
    let Some(hook) = &view.hook else {
        return Some(ShareBlock::Invalid);
    };

    if view.status == HookStatus::Invalid {
        return Some(ShareBlock::Invalid);
    }

    if hook.seed.is_some() {
        return Some(ShareBlock::Seed);
    }

    if view.status == HookStatus::Pending {
        return Some(ShareBlock::Pending);
    }

    let no_origin = hook.scope == HookScope::Project
        && hook
            .project
            .as_ref()
            .and_then(|root| projects.get(root))
            .is_some_and(|key| !key.can_share());

    if no_origin {
        return Some(ShareBlock::NoOrigin);
    }

    team_ids.contains(&view.id).then_some(ShareBlock::OnTeam)
}

impl Context<'_> {
    fn view_of(&self, file: TeamFile) -> HookView {
        let row = match file.row {
            Ok(row) => row,
            Err(problem) => {
                return HookView {
                    id: file.id,
                    problem: Some(problem),
                    origin: HookOrigin::Team,
                    ..local_defaults()
                };
            }
        };

        let bytes = row.body.as_bytes();
        let sha256 = sha256_hex(bytes);
        let approval = approval_of(&self.approvals, &row.id, &sha256);

        let mut view = HookView {
            id: row.id.clone(),
            name: loose_name(bytes),
            sha256: sha256.clone(),
            approval,
            origin: HookOrigin::Team,
            team_author: Some(label(&row.author)),
            team_updated_at: Some(row.updated_at.clone()),
            team_repo: row.repo.clone(),
            team_changed: self.local_changed(&row.id),
            team_disabled: self.state.team_disabled(&row.id),
            ..local_defaults()
        };

        match checked(&row, &sha256) {
            Ok(hook) => {
                view.name = Some(hook.name.clone());
                view.status = self.status_of(&hook, approval, view.team_disabled);
                view.hook = Some(hook);

                // A duplicate's approval is the local hook's: there is no team version to compare.
                if view.status != HookStatus::Duplicate {
                    view.approved = earlier_version(approval, &row.id, &|id| self.approved_copy(id));
                }
            }
            Err(problem) => view.problem = Some(problem),
        }

        view
    }

    fn status_of(&self, hook: &Hook, approval: HookApproval, disabled: bool) -> HookStatus {
        if self.local_ids.contains(&hook.id) {
            return HookStatus::Duplicate;
        }

        if !self.trust && approval != HookApproval::Approved {
            return HookStatus::Pending;
        }

        if !hook.enabled || disabled {
            return HookStatus::Off;
        }

        if !hook
            .requires
            .iter()
            .all(|requirement| self.capabilities.meets(*requirement))
        {
            return HookStatus::Paused;
        }

        HookStatus::Active
    }

    /// True when the local hook `id` changed since the user shared it.
    fn local_changed(&self, id: &str) -> bool {
        let Some(shared) = self.state.shared_from(id) else {
            return false;
        };

        read_file(&self.store.file_path(id), id).is_some_and(|file| file.sha256 != shared)
    }

    /// The approved earlier version of team hook `id` (its kept copy), for the diff.
    fn approved_copy(&self, id: &str) -> Option<Hook> {
        let bytes = approved_bytes(self.store.config_dir(), id, self.approvals.get(id)?)?;

        parse_team_hook(id, &bytes).ok()
    }
}

/// The hook in `row`, when its body hashes to its `sha256`, follows the team rules and matches its
/// repository.
fn checked(row: &TeamRow, sha256: &str) -> std::result::Result<Hook, String> {
    if row.sha256 != sha256 {
        return Err("the team copy does not match its hash".to_owned());
    }

    let hook = parse_team_hook(&row.id, row.body.as_bytes())?;

    let matches = match hook.scope {
        HookScope::Global => row.repo.is_none(),
        HookScope::Project => row.repo.is_some() && row.repo == hook.project,
    };

    if !matches {
        return Err("the team copy's repository does not match its project".to_owned());
    }

    Ok(hook)
}

/// `author` without control or invisible characters, at most [`MAX_NAME_CHARS`] long.
fn label(author: &str) -> String {
    author
        .chars()
        .filter(|character| !character.is_control() && !is_hidden_char(*character))
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}
