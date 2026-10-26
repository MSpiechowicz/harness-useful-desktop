//! Team routines on this machine (see "Team routines" in `docs/routines-format.md`): a catalogue
//! that arrives off, the status order, the trust rule (trust covers every team routine, one that
//! runs tools without asking included), approvals kept apart from local routines', the folder a
//! team project routine runs in, copying, sharing, and what a copy that leaves the team takes with
//! it.

use crate::routine_support;

use harness_business::BusinessError;
use harness_business::model::{
    HookApproval, HookAuthor, HookOrigin, RoutineStatus, RoutineView, ShareBlock, TeamRoutines,
};
use harness_business::routines::RoutineKind;
use harness_business::routines::state::read_state;
use harness_business::routines::team::{
    ProjectKey, TeamFetch, app_listing, apply_round, approve_team_routine, copy_team_routine, record_share,
    set_team_enabled, share_request, team_views,
};
use harness_business::signed_docs::{Approvals, team_approval_key};
use pretty_assertions::assert_eq;
use routine_support::{REPO, Root, body, command_draft, command_json, file, moment, rid, root, team_row};
use serde_json::json;

fn key() -> ProjectKey {
    ProjectKey {
        key: REPO.to_owned(),
        shareable: true,
    }
}

fn view(root: &Root, trust: bool, id: &str) -> RoutineView {
    team_views(&root.store, &root.folder, trust)
        .views
        .into_iter()
        .find(|view| view.id == id)
        .unwrap()
}

/// Shares a global command routine `n` (or one that runs tools without asking) as a team copy.
fn shared(root: &Root, n: u16, unattended: bool) -> String {
    let action = if unattended {
        json!({"type": "agent", "harness": "omp", "prompt": "Tidy up.", "approvalMode": "yolo"})
    } else {
        command_json()
    };

    let row = team_row(&rid(n), body(&rid(n), "Nightly", false, action), false);
    root.folder.write_row(&row).unwrap();
    row.sha256
}

#[test]
fn team_routines_arrive_off_and_run_once_turned_on() {
    let root = root();
    shared(&root, 1, false);

    let off = view(&root, true, &rid(1));
    assert_eq!(
        (off.status, off.origin, off.team_on),
        (RoutineStatus::Off, HookOrigin::Team, false)
    );
    assert_eq!(off.team_author.as_deref(), Some("mira"));
    assert!(!off.can_run_now);

    set_team_enabled(&root.store, &root.folder, &rid(1), true, None).unwrap();

    let on = view(&root, true, &rid(1));
    assert_eq!(
        (on.status, on.team_on, on.can_run_now),
        (RoutineStatus::Active, true, true)
    );
    assert_eq!(
        view(&root, false, &rid(1)).status,
        RoutineStatus::Pending,
        "without trust"
    );

    set_team_enabled(&root.store, &root.folder, &rid(1), false, None).unwrap();
    assert_eq!(view(&root, true, &rid(1)).status, RoutineStatus::Off);
}

#[test]
fn trust_covers_a_routine_that_runs_tools_without_asking() {
    let root = root();
    let sha256 = shared(&root, 2, true);
    set_team_enabled(&root.store, &root.folder, &rid(2), true, None).unwrap();

    let trusted = view(&root, true, &rid(2));
    assert_eq!(
        (trusted.status, trusted.runs_unattended, trusted.can_run_now),
        (RoutineStatus::Active, true, true)
    );
    assert_eq!(
        view(&root, false, &rid(2)).status,
        RoutineStatus::Pending,
        "without trust its exact bytes need approval"
    );

    approve_team_routine(&root.store, &root.folder, &rid(2), &sha256).unwrap();
    assert_eq!(view(&root, false, &rid(2)).status, RoutineStatus::Active);
}

#[test]
fn a_team_approval_never_approves_a_local_file_with_the_same_bytes() {
    let root = root();
    let sha256 = shared(&root, 8, false);
    approve_team_routine(&root.store, &root.folder, &rid(8), &sha256).unwrap();
    assert_eq!(view(&root, false, &rid(8)).approval, HookApproval::Approved);

    // The team body written as a local file outside the app: its id and bytes, never approved.
    let body = root.folder.row(&rid(8)).unwrap().body;
    std::fs::create_dir_all(root.store.routines_dir()).unwrap();
    std::fs::write(file(&root.store, &rid(8)), &body).unwrap();

    let local = root.store.get(&rid(8)).unwrap();
    assert_eq!(
        (local.status, local.approval, local.sha256),
        (RoutineStatus::Pending, HookApproval::Never, sha256)
    );
}

#[test]
fn a_local_approval_never_carries_over_to_a_team_copy() {
    let root = root();
    let local = root
        .store
        .save(&command_draft("Mine"), HookAuthor::Desktop, moment(0))
        .unwrap();
    assert_eq!(local.status, RoutineStatus::Active);

    // The same bytes arrive as a team copy, then the local file goes outside the app.
    let bytes = std::fs::read_to_string(file(&root.store, &local.id)).unwrap();
    root.folder.write_row(&team_row(&local.id, bytes, false)).unwrap();
    set_team_enabled(&root.store, &root.folder, &local.id, true, None).unwrap();
    std::fs::remove_file(file(&root.store, &local.id)).unwrap();

    let team = view(&root, false, &local.id);
    assert_eq!(
        (team.status, team.approval),
        (RoutineStatus::Pending, HookApproval::Never)
    );

    // Pruning keeps the team copy's own approval and drops the local one of the gone file.
    approve_team_routine(&root.store, &root.folder, &local.id, &team.sha256).unwrap();
    root.store.prune_approvals().unwrap();

    let approvals = Approvals::<RoutineKind>::read(root.store.config_dir()).unwrap();
    assert_eq!(approvals.get(&local.id), None);
    assert_eq!(view(&root, false, &local.id).status, RoutineStatus::Active);
}

#[test]
fn the_status_order_is_invalid_duplicate_off_pending_paused_active() {
    let root = root();

    // Invalid: the hash does not match the body.
    let mut broken = team_row(&rid(3), body(&rid(3), "Broken", false, command_json()), false);
    broken.sha256 = "0".repeat(64);
    root.folder.write_row(&broken).unwrap();

    // Duplicate: a local routine has the id, even while off here.
    let local = root
        .store
        .save(&command_draft("Local"), HookAuthor::Desktop, moment(0))
        .unwrap();
    let duplicate = team_row(&local.id, body(&local.id, "Same id", false, command_json()), false);
    root.folder.write_row(&duplicate).unwrap();

    // Off before Pending: not turned on and not approved.
    shared(&root, 4, true);

    let views = team_views(&root.store, &root.folder, false).views;
    let status = |id: &str| views.iter().find(|view| view.id == id).unwrap().status;

    assert_eq!(status(&rid(3)), RoutineStatus::Invalid);
    assert_eq!(status(&local.id), RoutineStatus::Duplicate);
    assert_eq!(status(&rid(4)), RoutineStatus::Off);
}

#[test]
fn a_team_project_routine_runs_in_the_folder_chosen_for_it() {
    let root = root();
    let row = team_row(&rid(5), body(&rid(5), "Lint", true, command_json()), true);
    root.folder.write_row(&row).unwrap();

    let without = set_team_enabled(&root.store, &root.folder, &rid(5), true, None);
    assert!(matches!(without, Err(BusinessError::InvalidInput(_))), "{without:?}");

    let wrong = ProjectKey {
        key: "git:example.com/other/repo".to_owned(),
        shareable: true,
    };
    let other = set_team_enabled(&root.store, &root.folder, &rid(5), true, Some((&root.project, &wrong)));
    assert!(matches!(other, Err(BusinessError::InvalidInput(_))), "{other:?}");

    set_team_enabled(&root.store, &root.folder, &rid(5), true, Some((&root.project, &key()))).unwrap();

    let on = view(&root, true, &rid(5));
    assert_eq!(
        (on.status, on.team_folder.as_deref()),
        (RoutineStatus::Active, Some(root.project.as_str()))
    );
    assert_eq!(
        read_state(root.store.config_dir()).team_enabled(&rid(5)),
        Some(Some(root.project.as_str()))
    );

    std::fs::rename(&root.project, root.temp.path().join("moved")).unwrap();
    assert_eq!(
        view(&root, true, &rid(5)).status,
        RoutineStatus::Paused,
        "the folder is gone"
    );
    assert_eq!(
        view(&root, false, &rid(5)).status,
        RoutineStatus::Pending,
        "pending comes before paused"
    );
}

#[test]
fn a_copy_needs_a_team_routine_that_may_run_here() {
    let root = root();
    let sha256 = shared(&root, 6, true);

    let pending = copy_team_routine(&root.store, &root.folder, false, &rid(6), &sha256, None);
    assert!(matches!(pending, Err(BusinessError::InvalidInput(_))), "{pending:?}");

    approve_team_routine(&root.store, &root.folder, &rid(6), &sha256).unwrap();
    let copied = copy_team_routine(&root.store, &root.folder, false, &rid(6), &sha256, None).unwrap();

    assert_eq!(
        (copied.origin, copied.status),
        (HookOrigin::Local, RoutineStatus::Active)
    );
    assert_eq!(view(&root, true, &rid(6)).status, RoutineStatus::Duplicate);
}

#[test]
fn sharing_sends_an_approved_local_routine_and_blocks_the_rest() {
    let root = root();
    let mine = root
        .store
        .save(&command_draft("Mine"), HookAuthor::Desktop, moment(0))
        .unwrap();
    let agents = root
        .store
        .save(&command_draft("Agent's"), HookAuthor::Agent, moment(1))
        .unwrap();

    let request = share_request(&root.store, &root.folder, &mine.id, &mine.sha256).unwrap();
    assert_eq!(
        request.body(None).unwrap(),
        std::fs::read(routine_support::file(&root.store, &mine.id)).unwrap()
    );

    let pending = share_request(&root.store, &root.folder, &agents.id, &agents.sha256);
    assert!(matches!(pending, Err(BusinessError::InvalidInput(_))), "{pending:?}");

    let stored = team_row(&mine.id, String::from_utf8(request.body(None).unwrap()).unwrap(), false);
    record_share(&root.store, &root.folder, &stored, &request.local_sha256, None).unwrap();

    let settings = TeamRoutines {
        trust: true,
        problem: None,
        refreshed_at: None,
    };
    let listing = app_listing(&root.store, &root.folder, None, Some(&settings), true);
    let block = |id: &str, origin: HookOrigin| {
        let found = listing
            .routines
            .iter()
            .find(|view| view.id == id && view.origin == origin);
        found.unwrap().share_block
    };

    assert_eq!(block(&mine.id, HookOrigin::Local), Some(ShareBlock::OnTeam));
    assert_eq!(block(&agents.id, HookOrigin::Local), Some(ShareBlock::Pending));
    assert_eq!(listing.counts.origin.get(&HookOrigin::Team), Some(&1));
}

#[test]
fn a_copy_that_leaves_the_team_takes_its_turned_on_entry_with_it() {
    let root = root();
    let sha256 = shared(&root, 7, false);
    set_team_enabled(&root.store, &root.folder, &rid(7), true, None).unwrap();
    approve_team_routine(&root.store, &root.folder, &rid(7), &sha256).unwrap();

    let round = TeamFetch {
        global_listed: true,
        complete: true,
        ..TeamFetch::default()
    };
    let counts = apply_round(&root.store, &root.folder, &round).unwrap();

    assert_eq!(counts.removed, [rid(7)]);
    assert_eq!(read_state(root.store.config_dir()).team_enabled(&rid(7)), None);
    let approvals = Approvals::<RoutineKind>::read(root.store.config_dir()).unwrap();
    assert_eq!(approvals.get(&team_approval_key::<RoutineKind>(&rid(7))), None);
    assert_eq!(approvals.ids().count(), 0);
}
