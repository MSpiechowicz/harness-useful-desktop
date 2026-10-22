//! What the user does with team hooks on this machine: the body a local hook is shared as (a
//! project hook under its repository key), what cannot be shared, the record that offers "Update
//! team copy", approving a team hook (with the diff against the version approved), copying one to
//! the local folder, and removing one from the team.

use crate::team_support;

use std::time::UNIX_EPOCH;

use harness_business::BusinessError;
use harness_business::hooks::team::{
    ProjectKey, ShareRequest, approve_team_hook, copy_team_hook, forget_team_hook, parse_team_hook, record_share,
    share_request, team_views,
};
use harness_business::hooks::{HookStore, apply_family, parse_hook_file};
use harness_business::model::{
    HookAction, HookApproval, HookAuthor, HookDraft, HookEvent, HookOrigin, HookScope, HookSeed, HookStatus, HookView,
};
use pretty_assertions::assert_eq;
use serde_json::Value;
use team_support::{NONE, REPO, bytes_of, hook_json, id, row, row_of, team};

fn save(store: &HookStore, scope: HookScope, project: Option<&str>, author: HookAuthor) -> HookView {
    let draft = HookDraft {
        id: None,
        expected_sha256: None,
        name: "Use tabs".to_owned(),
        description: Some("House style".to_owned()),
        scope,
        project: project.map(str::to_owned),
        event: HookEvent::PromptSubmit,
        matcher: None,
        action: HookAction::AddContext {
            text: "Indent with tabs.".to_owned(),
        },
        enabled: true,
    };

    store.save(&draft, author, NONE, UNIX_EPOCH).unwrap()
}

fn key(key: &str, shareable: bool) -> ProjectKey {
    ProjectKey {
        key: key.to_owned(),
        shareable,
    }
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn a_project_hook_is_shared_under_its_repository_key_and_nothing_else_changes() {
    let team = team();
    let view = save(
        &team.store,
        HookScope::Project,
        Some(&team.project),
        HookAuthor::Desktop,
    );

    let request = share_request(&team.store, &team.folder, &view.id, &view.sha256).unwrap();
    assert_eq!(request.project.as_deref(), Some(team.project.as_str()));
    assert_eq!(request.team_sha256, None);

    let body = request.body(Some(&key(REPO, true))).unwrap();
    let mut expected = json_of(&std::fs::read(team.store.hooks_dir().join(format!("{}.json", view.id))).unwrap());
    expected["project"] = REPO.into();
    assert_eq!(json_of(&body), expected);

    let hook = parse_team_hook(&view.id, &body).unwrap();
    assert_eq!(hook.project.as_deref(), Some(REPO));

    let refused = request.body(Some(&key("path:/home/u/demo", false)));
    assert!(matches!(refused, Err(BusinessError::Forbidden(_))), "{refused:?}");

    let unknown = request.body(None);
    assert!(matches!(unknown, Err(BusinessError::InvalidInput(_))), "{unknown:?}");
}

/// A global hook saved in the app window, ready to share.
fn shared_global(team: &team_support::Team) -> (HookView, ShareRequest) {
    let view = save(&team.store, HookScope::Global, None, HookAuthor::Desktop);
    let request = share_request(&team.store, &team.folder, &view.id, &view.sha256).unwrap();

    (view, request)
}

#[test]
fn a_global_hook_is_shared_byte_for_byte() {
    let team = team();
    let (view, request) = shared_global(&team);
    let body = request.body(None).unwrap();

    assert_eq!(
        body,
        std::fs::read(team.store.hooks_dir().join(format!("{}.json", view.id))).unwrap()
    );
}

#[test]
fn family_pending_and_changed_hooks_are_not_shared() {
    let team = team();

    apply_family(&team.store, &[HookSeed::MemorySearch], NONE, UNIX_EPOCH).unwrap();
    let seeded = team.store.list(NONE).hooks.remove(0);
    let family = share_request(&team.store, &team.folder, &seeded.id, &seeded.sha256);
    assert!(matches!(family, Err(BusinessError::Forbidden(_))), "{family:?}");

    let agent = save(&team.store, HookScope::Global, None, HookAuthor::Agent);
    assert_eq!(agent.status, HookStatus::Pending);
    let pending = share_request(&team.store, &team.folder, &agent.id, &agent.sha256);
    assert!(matches!(pending, Err(BusinessError::InvalidInput(_))), "{pending:?}");

    let mine = save(&team.store, HookScope::Global, None, HookAuthor::Desktop);
    let stale = share_request(&team.store, &team.folder, &mine.id, &"0".repeat(64));
    assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

    // A seed in a body never passes as a team hook.
    let mut seeded_body = hook_json(&id(9), "Seeded", None);
    seeded_body["seed"] = "memory-save".into();
    assert!(parse_team_hook(&id(9), bytes_of(&seeded_body).as_bytes()).is_err());
}

#[test]
fn after_a_share_a_local_edit_offers_update_team_copy() {
    let team = team();
    let (view, request) = shared_global(&team);

    let stored = row_of(&view.id, None, String::from_utf8(request.body(None).unwrap()).unwrap());
    record_share(&team.store, &team.folder, &stored, &request.local_sha256, None).unwrap();

    let shared = team_views(&team.store, &team.folder, NONE, true).views.remove(0);
    assert_eq!((shared.status, shared.team_changed), (HookStatus::Duplicate, false));

    // The local original changes: the team copy says so, and the next share updates it.
    let edited = team
        .store
        .save(
            &HookDraft {
                id: Some(view.id.clone()),
                expected_sha256: Some(view.sha256.clone()),
                name: "Use tabs everywhere".to_owned(),
                description: None,
                scope: HookScope::Global,
                project: None,
                event: HookEvent::PromptSubmit,
                matcher: None,
                action: HookAction::AddContext {
                    text: "Indent with tabs.".to_owned(),
                },
                enabled: true,
            },
            HookAuthor::Desktop,
            NONE,
            UNIX_EPOCH,
        )
        .unwrap();

    let changed = team_views(&team.store, &team.folder, NONE, true).views.remove(0);
    assert!(changed.team_changed);

    let update = share_request(&team.store, &team.folder, &edited.id, &edited.sha256).unwrap();
    assert_eq!(update.team_sha256.as_deref(), Some(stored.sha256.as_str()));

    // Removed from the team: the copy and its record go.
    forget_team_hook(&team.store, &team.folder, &view.id).unwrap();
    assert!(team_views(&team.store, &team.folder, NONE, true).views.is_empty());
}

#[test]
fn an_approved_team_hook_runs_and_a_new_version_shows_the_approved_one() {
    let team = team();
    let first = row(&id(1), "Team rule", None);
    team.folder.write_row(&first).unwrap();

    let stale = approve_team_hook(&team.store, &team.folder, &id(1), &"0".repeat(64));
    assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

    approve_team_hook(&team.store, &team.folder, &id(1), &first.sha256).unwrap();
    let approved = team_views(&team.store, &team.folder, NONE, false).views.remove(0);
    assert_eq!(
        (approved.status, approved.approval),
        (HookStatus::Active, HookApproval::Approved)
    );

    // A teammate changes it: pending again, with the approved version for the diff.
    let mut changed = hook_json(&id(1), "Team rule", None);
    changed["action"]["text"] = "Something else".into();
    team.folder
        .write_row(&row_of(&id(1), None, bytes_of(&changed)))
        .unwrap();

    let pending = team_views(&team.store, &team.folder, NONE, false).views.remove(0);
    assert_eq!(
        (pending.status, pending.approval),
        (HookStatus::Pending, HookApproval::Changed)
    );
    assert_eq!(
        pending.approved.unwrap().action,
        parse_team_hook(&id(1), first.body.as_bytes()).unwrap().action
    );

    // The watcher's pruning keeps the approval of a team hook.
    std::fs::create_dir_all(team.store.hooks_dir()).unwrap();
    assert_eq!(team.store.prune_approvals().unwrap(), 0);
}

#[test]
fn a_trusted_team_hook_copied_to_the_local_folder_wins_and_runs_in_the_project() {
    let team = team();
    let shared = row(&id(2), "Repo rule", Some(REPO));
    team.folder.write_row(&shared).unwrap();
    let copy = |project| copy_team_hook(&team.store, &team.folder, NONE, true, &id(2), &shared.sha256, project);

    let without = copy(None);
    assert!(matches!(without, Err(BusinessError::InvalidInput(_))), "{without:?}");

    let wrong = copy(Some((team.project.as_str(), "git:github.com/o/other")));
    assert!(matches!(wrong, Err(BusinessError::InvalidInput(_))), "{wrong:?}");

    let project = Some((team.project.as_str(), REPO));
    let copied = copy(project).unwrap();
    assert_eq!((copied.status, copied.origin), (HookStatus::Active, HookOrigin::Local));
    assert_eq!(
        copied.hook.as_ref().unwrap().project.as_deref(),
        Some(team.project.as_str())
    );

    let bytes = std::fs::read(team.store.hooks_dir().join(format!("{}.json", id(2)))).unwrap();
    assert!(parse_hook_file(&id(2), &bytes).is_ok());

    let team_copy = team_views(&team.store, &team.folder, NONE, true).views.remove(0);
    assert_eq!(team_copy.status, HookStatus::Duplicate);

    let again = copy(project);
    assert!(matches!(again, Err(BusinessError::InvalidInput(_))), "{again:?}");
}

#[test]
fn a_pending_team_hook_is_not_copied_until_it_is_approved() {
    let team = team();
    let shared = row(&id(3), "Team rule", None);
    team.folder.write_row(&shared).unwrap();
    let copy = || copy_team_hook(&team.store, &team.folder, NONE, false, &id(3), &shared.sha256, None);

    // Without trust or approval: refused, and nothing is written or approved.
    let pending = copy();
    assert!(matches!(pending, Err(BusinessError::InvalidInput(_))), "{pending:?}");
    assert!(!team.store.hooks_dir().join(format!("{}.json", id(3))).exists());
    assert!(!team.store.config_dir().join("hook-approvals.json").exists());

    // Invalid team copies are refused as well.
    let mut seeded = hook_json(&id(4), "Seeded", None);
    seeded["seed"] = "memory-search".into();
    let broken = row_of(&id(4), None, bytes_of(&seeded));
    team.folder.write_row(&broken).unwrap();
    let invalid = copy_team_hook(&team.store, &team.folder, NONE, true, &id(4), &broken.sha256, None);
    assert!(matches!(invalid, Err(BusinessError::InvalidInput(_))), "{invalid:?}");

    approve_team_hook(&team.store, &team.folder, &id(3), &shared.sha256).unwrap();
    let copied = copy().unwrap();
    assert_eq!((copied.status, copied.origin), (HookStatus::Active, HookOrigin::Local));
}
