//! Team hooks on this machine: each team hook's status (trust on and off, the local off-switch, a
//! local hook with the same id), the repository key of each project root, the app window's
//! listing, and the run order in a chat. Refresh rounds are in `hooks_team_sync.rs`.

use crate::hook_store_support;
use crate::team_support;

use harness_business::hooks::team::{app_listing, team_views};
use harness_business::hooks::{ChatContext, HookCapabilities, HookSession, HookSet, SendOrigin};
use harness_business::model::{
    AgentAction, HookAuthor, HookDraft, HookOrigin, HookStatus, HookView, ShareBlock, TeamHooks,
};
use hook_store_support::edit_of;
use pretty_assertions::assert_eq;
use team_support::{NONE, REPO, bytes_of, hook_json, id, local, row, row_of, team};

fn settings(trust: bool) -> TeamHooks {
    TeamHooks {
        trust,
        problem: None,
        refreshed_at: None,
    }
}

fn status_of(views: &[HookView], id: &str) -> HookStatus {
    views.iter().find(|view| view.id == id).expect("listed").status
}

#[test]
fn with_trust_a_team_hook_is_active_without_it_pending_until_approved() {
    let team = team();
    team.folder.write_row(&row(&id(1), "Global rule", None)).unwrap();

    let trusted = team_views(&team.store, &team.folder, NONE, true).views;
    assert_eq!(status_of(&trusted, &id(1)), HookStatus::Active);
    assert_eq!(trusted[0].origin, HookOrigin::Team);
    assert_eq!(trusted[0].team_author.as_deref(), Some("ana"));
    assert_eq!(trusted[0].team_updated_at.as_deref(), Some("2026-10-02T08:30:00Z"));

    let untrusted = team_views(&team.store, &team.folder, NONE, false).views;
    assert_eq!(status_of(&untrusted, &id(1)), HookStatus::Pending);
}

#[test]
fn the_local_off_switch_turns_only_this_machines_copy_off() {
    let team = team();
    let shared = row(&id(1), "Global rule", None);
    team.folder.write_row(&shared).unwrap();

    harness_business::hooks::team::set_team_enabled(&team.store, &team.folder, &id(1), false).unwrap();

    let views = team_views(&team.store, &team.folder, NONE, true).views;
    assert_eq!((views[0].status, views[0].team_disabled), (HookStatus::Off, true));
    assert_eq!(team.folder.row(&id(1)).unwrap(), shared, "the team copy is not changed");

    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(team.store.config_dir().join("hooks-state.json")).unwrap()).unwrap();
    assert_eq!(
        state,
        serde_json::json!({"version": 1, "seeded": [], "teamDisabled": [id(1)]})
    );

    harness_business::hooks::team::set_team_enabled(&team.store, &team.folder, &id(1), true).unwrap();
    assert_eq!(
        status_of(&team_views(&team.store, &team.folder, NONE, true).views, &id(1)),
        HookStatus::Active
    );
}

#[test]
fn a_local_hook_with_the_same_id_wins_and_the_team_copy_is_a_duplicate() {
    let team = team();
    let mine = local(&team.store, "Mine");
    team.folder.write_row(&row(&mine.id, "Shared", None)).unwrap();

    let views = team_views(&team.store, &team.folder, NONE, true).views;
    assert_eq!(status_of(&views, &mine.id), HookStatus::Duplicate);

    let listing = app_listing(&team.store, &team.folder, NONE, None, Some(&settings(true)), true);
    let local = listing
        .hooks
        .iter()
        .find(|view| view.origin == HookOrigin::Local)
        .unwrap();
    assert_eq!(local.share_block, Some(ShareBlock::OnTeam));

    // Only the local one runs.
    let set = HookSet::for_chat(std::slice::from_ref(local), None).with_team(&views, None, None);
    assert_eq!(fired(&set, "/elsewhere"), [mine.id]);
}

#[test]
fn invalid_bodies_and_envelopes_are_listed_as_invalid() {
    let team = team();
    let mut seeded = hook_json(&id(1), "Seeded", None);
    seeded["seed"] = "memory-search".into();
    team.folder.write_row(&row_of(&id(1), None, bytes_of(&seeded))).unwrap();

    let mut wrong_hash = row(&id(2), "Tampered", None);
    wrong_hash.body = wrong_hash.body.replace("Tampered text", "Other text");
    team.folder.write_row(&wrong_hash).unwrap();

    // A project body with a local path, not a repository key.
    let path_body = bytes_of(&hook_json(&id(3), "Path", Some("/home/u/demo")));
    team.folder
        .write_row(&row_of(&id(3), Some("/home/u/demo"), path_body))
        .unwrap();

    std::fs::write(team.folder.dir().join(format!("{}.json", id(4))), "not json").unwrap();

    let views = team_views(&team.store, &team.folder, NONE, true).views;
    assert_eq!(views.len(), 4);
    assert!(
        views
            .iter()
            .all(|view| view.status == HookStatus::Invalid && view.problem.is_some())
    );
}

#[test]
fn a_project_root_gets_the_team_hooks_of_its_repository() {
    let team = team();
    team.copies(&[
        row(&id(1), "Global rule", None),
        row(&id(2), "Repo rule", Some(REPO)),
        row(&id(3), "Other repo", Some("git:github.com/o/other")),
    ]);
    team.map_project();

    let listed = |verified| {
        app_listing(
            &team.store,
            &team.folder,
            NONE,
            Some(&team.project),
            Some(&settings(true)),
            verified,
        )
    };
    let in_project = listed(true);
    let ids: Vec<&str> = in_project.hooks.iter().map(|view| view.id.as_str()).collect();
    assert_eq!(ids, [id(1), id(2)]);

    // A root whose key no refresh of this session confirmed shows only the global team hooks.
    let unverified = listed(false);
    let ids: Vec<&str> = unverified.hooks.iter().map(|view| view.id.as_str()).collect();
    assert_eq!(ids, [id(1)]);
    assert_eq!(in_project.team, Some(settings(true)));
    assert_eq!(in_project.hooks[1].team_repo.as_deref(), Some(REPO));

    let global = app_listing(&team.store, &team.folder, NONE, None, Some(&settings(true)), true);
    assert_eq!(global.hooks.len(), 1);

    // Sharing off: no team hooks, whatever is on disk.
    let off = app_listing(&team.store, &team.folder, NONE, Some(&team.project), None, true);
    assert!(off.hooks.is_empty() && off.team.is_none());
}

#[test]
fn a_chat_runs_local_hooks_first_then_team_global_then_team_project() {
    let team = team();
    let mine = local(&team.store, "Mine");
    team.copies(&[row(&id(2), "Repo rule", Some(REPO)), row(&id(1), "Global rule", None)]);
    team.map_project();

    let local_views = team.store.list(NONE).hooks;
    let views = team_views(&team.store, &team.folder, NONE, true);
    let key = views.key_of(Some(&team.project));
    assert_eq!(key, Some(REPO));

    let in_project =
        HookSet::for_chat(&local_views, Some(&team.project)).with_team(&views.views, Some(&team.project), key);
    assert_eq!(fired(&in_project, &team.project), [mine.id.clone(), id(1), id(2)]);

    let elsewhere =
        HookSet::for_chat(&local_views, Some("/elsewhere")).with_team(&views.views, Some("/elsewhere"), None);
    assert_eq!(fired(&elsewhere, "/elsewhere"), [mine.id.clone(), id(1)]);

    // Without trust the team hooks wait for approval.
    let pending = team_views(&team.store, &team.folder, NONE, false);
    let set = HookSet::for_chat(&local_views, Some(&team.project)).with_team(&pending.views, Some(&team.project), key);
    assert_eq!(fired(&set, &team.project), [mine.id]);
}

/// The ids of the hooks the first prompt of a chat in `folder` fires, in run order.
fn fired(set: &HookSet, folder: &str) -> Vec<String> {
    let mut session = HookSession::new(ChatContext {
        session_id: "session-1".to_owned(),
        folder: folder.into(),
        project: Some(folder.to_owned()),
        fresh: false,
        utility: false,
    });
    let prompt = AgentAction::Prompt {
        message: "Hello".to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    };

    session
        .on_send(set, &prompt, SendOrigin::User)
        .firings
        .iter()
        .flat_map(|firing| firing.parts.iter().map(|part| part.hook_id().to_owned()))
        .collect()
}

#[test]
fn the_mcp_listing_never_shows_team_hooks_and_delete_all_keeps_them() {
    let team = team();
    local(&team.store, "Mine");
    team.folder.write_row(&row(&id(1), "Global rule", None)).unwrap();

    let listing = team.store.list(HookCapabilities::default());
    assert_eq!(listing.hooks.len(), 1);
    assert!(listing.team.is_none());

    assert_eq!(team.store.delete_all(None).unwrap(), 1);
    assert!(team.folder.row(&id(1)).is_ok());
}

#[test]
fn local_hooks_say_why_they_cannot_be_shared() {
    let team = team();
    let mut pending = local(&team.store, "Pending");
    pending = team
        .store
        .save(
            &HookDraft {
                id: Some(pending.id.clone()),
                expected_sha256: Some(pending.sha256.clone()),
                name: "Agent edit".to_owned(),
                ..edit_of(&pending)
            },
            HookAuthor::Agent,
            NONE,
            std::time::UNIX_EPOCH,
        )
        .unwrap();
    let free = local(&team.store, "Free");

    let listing = app_listing(&team.store, &team.folder, NONE, None, Some(&settings(false)), true);
    let block = |id: &str| listing.hooks.iter().find(|view| view.id == id).unwrap().share_block;

    assert_eq!(block(&pending.id), Some(ShareBlock::Pending));
    assert_eq!(block(&free.id), None);
}

#[test]
fn a_team_folder_other_users_can_change_is_not_read() {
    let team = team();
    team.folder.write_row(&row(&id(1), "Global rule", None)).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(team.folder.dir(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(team_views(&team.store, &team.folder, NONE, true).views.is_empty());
    }
}
