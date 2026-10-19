//! The routines folder (see `docs/routines-format.md`, Approvals and Agent rules): the app window's
//! writes are approved at once, an agent's stay pending, every edit names the version it changes,
//! an agent removes only what an agent created, a scope holds 64 routines with approved ones
//! first, rejecting restores or deletes, "Delete all routines" keeps other projects, and the
//! listing's counts, filter and running and next-run additions.

mod routine_support;

use std::collections::BTreeSet;

use harness_business::BusinessError;
use harness_business::model::{
    HookApproval, HookAuthor, HookOrigin, HookScope, RoutineFilter, RoutineStatus, RoutineWhen, RoutineWhenKind,
};
use harness_business::routines::scheduled_from;
use harness_business::routines::view::LiveRoutines;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;
use routine_support::{agent_action, command_draft, edit, file, moment, root};

const DESKTOP: HookAuthor = HookAuthor::Desktop;
const AGENT: HookAuthor = HookAuthor::Agent;

#[test]
fn the_app_windows_routine_runs_at_once_and_an_agents_waits_for_review() {
    let root = root();

    let mine = root.store.save(&command_draft("Report"), DESKTOP, moment(0)).unwrap();
    assert_eq!(
        (mine.status, mine.approval),
        (RoutineStatus::Active, HookApproval::Approved)
    );
    assert!(mine.can_run_now);

    let theirs = root.store.save(&command_draft("Agent's"), AGENT, moment(1)).unwrap();
    assert_eq!(
        (theirs.status, theirs.approval),
        (RoutineStatus::Pending, HookApproval::Never)
    );
    assert!(
        !theirs.can_run_now,
        "a pending routine never runs, not even with Run now"
    );

    let routine = theirs.routine.unwrap();
    assert_eq!((routine.created_by, routine.updated_by), (AGENT, AGENT));

    root.store.approve(&theirs.id, &theirs.sha256).unwrap();
    assert_eq!(root.store.get(&theirs.id).unwrap().status, RoutineStatus::Active);
}

#[test]
fn an_agents_edit_of_a_users_routine_is_pending_and_shows_what_changed() {
    let root = root();
    let mine = root.store.save(&command_draft("Report"), DESKTOP, moment(0)).unwrap();

    let mut changed = edit(&mine);
    changed.action = agent_action(true);
    let edited = root.store.save(&changed, AGENT, moment(5)).unwrap();

    assert_eq!(
        (edited.status, edited.approval),
        (RoutineStatus::Pending, HookApproval::Changed)
    );
    assert!(edited.runs_unattended);
    assert_eq!(edited.approved.unwrap().action, mine.routine.clone().unwrap().action);

    let routine = edited.routine.unwrap();
    assert_eq!((routine.created_by, routine.updated_by), (DESKTOP, AGENT));
    assert_eq!(routine.created_at, mine.routine.unwrap().created_at);

    // The app window approves or rejects a pending file before it edits it.
    let refused = root
        .store
        .save(&edit(&root.store.get(&mine.id).unwrap()), DESKTOP, moment(6));
    assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");
}

#[test]
fn an_edit_names_the_version_it_changes() {
    let root = root();
    let view = root.store.save(&command_draft("Report"), DESKTOP, moment(0)).unwrap();

    let mut stale = edit(&view);
    stale.expected_sha256 = Some("0".repeat(64));
    assert!(matches!(
        root.store.save(&stale, DESKTOP, moment(1)),
        Err(BusinessError::Conflict(_))
    ));

    let mut unnamed = edit(&view);
    unnamed.expected_sha256 = None;
    assert!(matches!(
        root.store.save(&unnamed, AGENT, moment(1)),
        Err(BusinessError::InvalidInput(_))
    ));

    let mut renamed = edit(&view);
    renamed.name = "Evening report".to_owned();
    let saved = root.store.save(&renamed, DESKTOP, moment(2)).unwrap();
    assert_eq!(saved.name.as_deref(), Some("Evening report"));
}

#[test]
fn an_agent_removes_only_the_version_it_read_of_a_routine_an_agent_created() {
    let root = root();
    let mine = root.store.save(&command_draft("Mine"), DESKTOP, moment(0)).unwrap();
    let theirs = root.store.save(&command_draft("Theirs"), AGENT, moment(1)).unwrap();

    let forbidden = root.store.delete(&mine.id, AGENT, Some(&mine.sha256));
    assert!(matches!(forbidden, Err(BusinessError::Forbidden(_))), "{forbidden:?}");

    let stale = root.store.delete(&theirs.id, AGENT, Some(&"0".repeat(64)));
    assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

    root.store.delete(&theirs.id, AGENT, Some(&theirs.sha256)).unwrap();
    root.store.delete(&mine.id, DESKTOP, None).unwrap();

    assert!(root.store.list().routines.is_empty());
    assert!(!file(&root.store, &mine.id).exists());
}

#[test]
fn rejecting_restores_the_approved_version_or_deletes_a_new_one() {
    let root = root();
    let mine = root.store.save(&command_draft("Report"), DESKTOP, moment(0)).unwrap();

    let mut changed = edit(&mine);
    changed.when = RoutineWhen::EveryMinutes { every: 1 };
    let pending = root.store.save(&changed, AGENT, moment(1)).unwrap();

    let restored = root.store.reject(&pending.id, &pending.sha256).unwrap().unwrap();
    assert_eq!(
        (restored.sha256, restored.status),
        (mine.sha256.clone(), RoutineStatus::Active)
    );

    let new = root.store.save(&command_draft("New"), AGENT, moment(2)).unwrap();
    assert_eq!(root.store.reject(&new.id, &new.sha256).unwrap(), None);
    assert!(!file(&root.store, &new.id).exists());

    let approved = root.store.reject(&mine.id, &mine.sha256);
    assert!(matches!(approved, Err(BusinessError::InvalidInput(_))), "{approved:?}");
}

#[test]
fn a_scope_holds_sixty_four_routines_and_approved_ones_come_first() {
    let root = root();

    let pending: Vec<String> = (0..3)
        .map(|n| {
            root.store
                .save(&command_draft(&format!("Agent {n}")), AGENT, moment(n))
                .unwrap()
                .id
        })
        .collect();

    for n in 0..64 {
        root.store
            .save(&command_draft(&format!("Mine {n}")), DESKTOP, moment(100 + n))
            .unwrap();
    }

    let full = root.store.save(&command_draft("One more"), DESKTOP, moment(500));
    assert!(matches!(full, Err(BusinessError::InvalidInput(_))), "{full:?}");

    let listing = root.store.list();
    let over: Vec<&str> = listing
        .routines
        .iter()
        .filter(|view| view.status == RoutineStatus::Invalid)
        .map(|view| view.id.as_str())
        .collect();

    // The older pending files are pushed out by the approved ones.
    assert_eq!(
        over.into_iter().collect::<BTreeSet<_>>(),
        pending.iter().map(String::as_str).collect()
    );

    let mut project = command_draft("Project");
    project.scope = HookScope::Project;
    project.project = Some(root.project.clone());
    assert!(
        root.store.save(&project, DESKTOP, moment(600)).is_ok(),
        "another scope has room"
    );
}

#[test]
fn delete_all_keeps_other_projects() {
    let root = root();
    let other = root.temp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();

    let global = root.store.save(&command_draft("Global"), DESKTOP, moment(0)).unwrap();
    let mut here = command_draft("Here");
    here.scope = HookScope::Project;
    here.project = Some(root.project.clone());
    let here = root.store.save(&here, AGENT, moment(1)).unwrap();

    let mut elsewhere = command_draft("Elsewhere");
    elsewhere.scope = HookScope::Project;
    elsewhere.project = Some(other.to_string_lossy().into_owned());
    let elsewhere = root.store.save(&elsewhere, DESKTOP, moment(2)).unwrap();

    let deleted: BTreeSet<String> = root
        .store
        .delete_all(Some(&root.project))
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(deleted, [global.id, here.id].into_iter().collect());

    let left: Vec<String> = root.store.list().routines.into_iter().map(|view| view.id).collect();
    assert_eq!(left, [elsewhere.id]);
}

#[test]
fn a_listing_counts_filters_and_shows_running_and_next_runs() {
    let root = root();
    let active = root.store.save(&command_draft("Active"), DESKTOP, moment(0)).unwrap();

    let mut off = command_draft("Off");
    off.enabled = false;
    off.when = RoutineWhen::AppStart;
    let off = root.store.save(&off, DESKTOP, moment(1)).unwrap();
    root.store.save(&command_draft("Pending"), AGENT, moment(2)).unwrap();

    let mut listing = root.store.list();
    let now: jiff::Timestamp = "2026-06-01T06:00:00Z".parse().unwrap();
    let live = LiveRoutines {
        running: [off.id.clone()].into_iter().collect(),
        queued: [active.id.clone()].into_iter().collect(),
    };
    listing.finish(&live, now, &TimeZone::UTC);

    let status = |id: &str| listing.routines.iter().find(|view| view.id == id).unwrap().status;
    assert_eq!(status(&off.id), RoutineStatus::Running, "Run now on an off routine");

    let next = listing.routines.iter().find(|view| view.id == active.id).unwrap();
    assert_eq!(next.next_run.as_deref(), Some("2026-06-01T07:30:00Z"));
    assert!(next.queued, "its run waits for a free place");
    assert_eq!(next.status, RoutineStatus::Active);

    assert_eq!(listing.counts.total, 3);
    assert_eq!(listing.counts.status.get(&RoutineStatus::Pending), Some(&1));
    assert_eq!(listing.counts.when.get(&RoutineWhenKind::Daily), Some(&2));
    assert_eq!(listing.counts.origin.get(&HookOrigin::Local), Some(&3));

    let scheduled = scheduled_from(&listing.routines, &BTreeSet::new());
    assert_eq!(
        scheduled.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
        [active.id.as_str()]
    );

    let filter = RoutineFilter {
        status: Some(RoutineStatus::Pending),
        ..RoutineFilter::default()
    };
    let filtered = listing.filtered(&filter);
    assert_eq!((filtered.routines.len(), filtered.counts.total), (1, 3));
}
