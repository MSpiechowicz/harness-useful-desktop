//! The status of each local routine file (see [`RoutineStatus`]) and what the app window adds to
//! a listing: the routines running now, the next run of each active one, the counts of each
//! filter value, and the filter.
//!
//! A local routine is Invalid (it breaks the format, or it is over its scope's limit), Pending
//! (this exact file was never approved), Off (approved, its schedule off) or Active. The app
//! window adds: Paused for an approved project routine whose folder is missing or no longer that
//! project ([`pause_lost_project`]), and Running in place of Active or Off while a run of it goes
//! on.

use std::collections::BTreeSet;
use std::path::Path;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::when::{Schedule, rfc3339_utc};
use crate::model::{
    HookApproval, HookOrigin, Routine, RoutineCounts, RoutineFilter, RoutineListing, RoutineStatus, RoutineView,
};
use crate::signed_docs::DocFile;
use crate::signed_docs::approvals::Approvals;
use crate::signed_docs::fields::loose_name;
use crate::signed_docs::listing::{SortedFiles, earlier_version};
use crate::signed_docs::project_identity;
use crate::signed_docs::scope::{FileOwner, scoped_doc};

use super::RoutineKind;

scoped_doc!(Routine);

/// The views of a folder's files with whose each is: valid routines first (global, then project,
/// then `createdAt`, then `id`), then invalid files by id. `approved` returns the kept copy of an
/// approved earlier version.
pub(crate) fn local_views(
    sorted: SortedFiles<Routine>,
    approvals: &Approvals<RoutineKind>,
    approved: &dyn Fn(&str) -> Option<Routine>,
) -> Vec<(RoutineView, FileOwner)> {
    let mut views: Vec<(RoutineView, FileOwner)> = sorted
        .used
        .into_iter()
        .map(|(routine, file)| {
            let owner = FileOwner::of(&routine);
            (view_of(routine, file.sha256, approvals, approved), owner)
        })
        .collect();

    views.extend(
        sorted
            .invalid
            .into_iter()
            .map(|(file, problem, owner)| (invalid_view(file, problem, approvals, approved), owner)),
    );

    views
}

/// The view of the valid local `routine` whose file hashes to `sha256`.
pub(crate) fn view_of(
    routine: Routine,
    sha256: String,
    approvals: &Approvals<RoutineKind>,
    approved: &dyn Fn(&str) -> Option<Routine>,
) -> RoutineView {
    let approval = approvals.approval(&routine.id, &sha256);

    let status = match (approval, routine.enabled) {
        (HookApproval::Approved, true) => RoutineStatus::Active,
        (HookApproval::Approved, false) => RoutineStatus::Off,
        _ => RoutineStatus::Pending,
    };

    RoutineView {
        id: routine.id.clone(),
        name: Some(routine.name.clone()),
        status,
        sha256,
        approved: earlier_version(approval, &routine.id, approved),
        approval,
        runs_unattended: routine.action.runs_unattended(),
        can_run_now: approval == HookApproval::Approved,
        routine: Some(routine),
        ..blank_view(HookOrigin::Local)
    }
}

fn invalid_view(
    file: DocFile,
    problem: String,
    approvals: &Approvals<RoutineKind>,
    approved: &dyn Fn(&str) -> Option<Routine>,
) -> RoutineView {
    let approval = approvals.approval(&file.id, &file.sha256);

    RoutineView {
        name: loose_name(&file.bytes),
        approved: earlier_version(approval, &file.id, approved),
        approval,
        problem: Some(problem),
        id: file.id,
        sha256: file.sha256,
        ..blank_view(HookOrigin::Local)
    }
}

/// An invalid view of `origin` with nothing filled in.
pub(crate) fn blank_view(origin: HookOrigin) -> RoutineView {
    RoutineView {
        id: String::new(),
        routine: None,
        name: None,
        status: RoutineStatus::Invalid,
        sha256: String::new(),
        problem: None,
        approval: HookApproval::Never,
        approved: None,
        origin,
        runs_unattended: false,
        can_run_now: false,
        queued: false,
        next_run: None,
        team_author: None,
        team_updated_at: None,
        team_repo: None,
        team_changed: false,
        team_on: false,
        team_folder: None,
        share_block: None,
        root: None,
        repo_key: None,
    }
}

/// The counts of each filter value over `routines`.
pub fn counts_of(routines: &[RoutineView]) -> RoutineCounts {
    let mut counts = RoutineCounts {
        total: u32::try_from(routines.len()).unwrap_or(u32::MAX),
        ..RoutineCounts::default()
    };

    for view in routines {
        *counts.status.entry(view.status).or_default() += 1;
        *counts.origin.entry(view.origin).or_default() += 1;

        if let Some(routine) = &view.routine {
            *counts.when.entry(routine.when.kind()).or_default() += 1;
        }
    }

    counts
}

/// True when the routine of `view` runs on its schedule: active, or running while its schedule is
/// on (a local routine, or a team routine turned on here).
pub fn on_schedule(view: &RoutineView) -> bool {
    match view.status {
        RoutineStatus::Active => true,
        RoutineStatus::Running => {
            let enabled = view.routine.as_ref().is_some_and(|routine| routine.enabled);
            enabled && (view.origin == HookOrigin::Local || view.team_on)
        }
        _ => false,
    }
}

/// The message of a project routine whose folder is missing.
pub const PROJECT_MISSING: &str = "The project folder is missing";

/// The message of a project routine whose folder is no longer that project.
pub const PROJECT_MOVED: &str = "The folder is no longer that project";

/// Why a project routine cannot run in the project root `project` now: the folder is missing,
/// or its project identity is another folder. Reads the folder.
pub fn project_problem(project: &Path) -> Option<&'static str> {
    if !project.is_dir() {
        return Some(PROJECT_MISSING);
    }

    match project_identity(project) {
        Ok(identity) if identity == project => None,
        _ => Some(PROJECT_MOVED),
    }
}

/// Pauses the approved local project routine of `view` (Active or Off) while its folder is
/// missing or no longer that project ([`project_problem`]): Paused, with the reason as its
/// problem, and no "Run now". Other views stay as they are. Reads the folder: call it off the
/// async runtime.
pub fn pause_lost_project(view: &mut RoutineView) {
    let approved = matches!(view.status, RoutineStatus::Active | RoutineStatus::Off);

    if view.origin != HookOrigin::Local || !approved {
        return;
    }

    let Some(project) = view.routine.as_ref().and_then(|routine| routine.project.as_deref()) else {
        return;
    };

    if let Some(problem) = project_problem(Path::new(project)) {
        view.status = RoutineStatus::Paused;
        view.problem = Some(problem.to_owned());
        view.can_run_now = false;
    }
}

/// The routines the app window has a run of: going on, or waiting for a free place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveRoutines {
    /// A run goes on.
    pub running: BTreeSet<String>,
    /// A run waits for a free place.
    pub queued: BTreeSet<String>,
}

/// True when `view` passes `filter`.
pub fn passes(view: &RoutineView, filter: &RoutineFilter) -> bool {
    let when = view.routine.as_ref().map(|routine| routine.when.kind());

    filter.status.is_none_or(|status| view.status == status)
        && filter.origin.is_none_or(|origin| view.origin == origin)
        && filter.when.is_none_or(|kind| when == Some(kind))
}

impl RoutineListing {
    /// What the app window adds to a listing: approved local project routines whose folder is
    /// gone are Paused ([`pause_lost_project`]), the routines of `live` with a run going on become
    /// Running and those waiting for a place are `queued`, each active routine gets its next run
    /// after `now` in `tz`, and the counts are taken again. Reads the project folders: call it off
    /// the async runtime.
    pub fn finish(&mut self, live: &LiveRoutines, now: Timestamp, tz: &TimeZone) {
        for view in &mut self.routines {
            pause_lost_project(view);

            let can_run = matches!(view.status, RoutineStatus::Active | RoutineStatus::Off);

            if can_run && live.running.contains(&view.id) {
                view.status = RoutineStatus::Running;
            }

            view.queued = live.queued.contains(&view.id);

            let scheduled = on_schedule(view);

            view.next_run = view
                .routine
                .as_ref()
                .filter(|_| scheduled)
                .and_then(|routine| Schedule::of(&routine.when).ok())
                .and_then(|schedule| schedule.next_after(now, tz))
                .map(rfc3339_utc);
        }

        self.counts = counts_of(&self.routines);
    }

    /// The listing with only the routines `filter` passes; the counts stay those of every
    /// routine, so the filter menus can show them.
    pub fn filtered(mut self, filter: &RoutineFilter) -> Self {
        self.routines.retain(|view| passes(view, filter));
        self
    }
}
