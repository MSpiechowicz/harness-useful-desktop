//! The Hooks view's commands over a temporary home with `home/demo` open, and the watcher.

use harness_business::model::{HookAction, HookApproval, HookEvent, HookStatus, IpcErrorCode};
use serde_json::json;
use tauri::async_runtime::block_on;

use super::*;
use crate::test_support::{ProjectHome, collecting_channel, path_arg};

fn draft(name: &str, scope: HookScope, project: Option<String>) -> HookDraft {
    HookDraft {
        id: None,
        expected_sha256: None,
        name: name.to_owned(),
        description: None,
        scope,
        project,
        event: HookEvent::PromptSubmit,
        matcher: None,
        action: HookAction::AddContext {
            text: "Be brief.".to_owned(),
        },
        enabled: true,
    }
}

/// Saves the global hook "Everywhere" and the `demo` hook "Demo only" from the window.
fn save_global_and_demo(home: &ProjectHome) {
    block_on(save(&home.state, draft("Everywhere", HookScope::Global, None))).expect("global hook");
    block_on(save(
        &home.state,
        draft("Demo only", HookScope::Project, path_arg(&home.project)),
    ))
    .expect("project hook");
}

fn names(listing: &HookListing) -> Vec<String> {
    listing.hooks.iter().filter_map(|view| view.name.clone()).collect()
}

#[test]
fn a_project_lists_the_global_hooks_and_its_own() {
    let home = ProjectHome::new("hooks-list");
    let state = &home.state;

    block_on(save(state, draft("Everywhere", HookScope::Global, None))).expect("global hook");
    let own = block_on(save(
        state,
        draft("Demo only", HookScope::Project, path_arg(&home.project)),
    ))
    .expect("project hook");
    assert_eq!(own.status, HookStatus::Active);

    // A hook of a project that is not open, as an agent would write it.
    let store = state.hooks.store().unwrap();
    let other = draft("Elsewhere", HookScope::Project, path_arg(&home.stranger));
    store
        .save(&other, HookAuthor::Agent, Default::default(), SystemTime::now())
        .expect("agent hook");

    let demo = block_on(list(state, path_arg(&home.project), false)).expect("listed");
    assert_eq!(names(&demo), ["Everywhere", "Demo only"]);

    let global = block_on(list(state, None, false)).expect("listed");
    assert_eq!(names(&global), ["Everywhere"]);

    let stranger = block_on(list(state, path_arg(&home.stranger), false)).expect_err("not a known project");
    assert_eq!(stranger.code(), IpcErrorCode::Forbidden);
}

#[test]
fn with_all_every_project_lists_its_hooks_with_its_root() {
    let home = ProjectHome::new("hooks-list-all");
    let state = &home.state;
    save_global_and_demo(&home);

    let store = state.hooks.store().unwrap();
    let other = draft("Elsewhere", HookScope::Project, path_arg(&home.stranger));
    let other = store
        .save(&other, HookAuthor::Agent, Default::default(), SystemTime::now())
        .expect("agent hook");

    // Project hooks saved in the same second run by id, so only the global one has a fixed place.
    let all = block_on(list(state, path_arg(&home.project), true)).expect("listed");
    let mut listed = names(&all);
    listed[1..].sort_unstable();
    assert_eq!(listed, ["Everywhere", "Demo only", "Elsewhere"]);

    let elsewhere = all.hooks.iter().find(|view| view.id == other.id).unwrap();
    assert_eq!(
        elsewhere.root,
        other.hook.as_ref().and_then(|hook| hook.project.clone())
    );
    assert_eq!(all.hooks[0].root, None);

    // Without a project too: `all` does not depend on the open one.
    assert_eq!(names(&block_on(list(state, None, true)).unwrap()).len(), 3);
}

#[test]
fn a_project_hook_is_saved_only_for_a_known_project() {
    let home = ProjectHome::new("hooks-save");

    let refused = block_on(save(
        &home.state,
        draft("Elsewhere", HookScope::Project, path_arg(&home.stranger)),
    ))
    .expect_err("not a known project");

    assert_eq!(refused.code(), IpcErrorCode::Forbidden);
    assert!(names(&block_on(list(&home.state, None, false)).unwrap()).is_empty());
}

#[test]
fn deleting_names_the_version_and_a_gone_hook_is_not_found() {
    let home = ProjectHome::new("hooks-delete");
    let state = &home.state;
    let saved = block_on(save(state, draft("Style", HookScope::Global, None))).expect("saved");

    let stale = block_on(delete(state, saved.id.clone(), "0".repeat(64))).expect_err("another version");
    assert_eq!(stale.code(), IpcErrorCode::Conflict);

    block_on(delete(state, saved.id.clone(), saved.sha256.clone())).expect("deleted");

    let gone = block_on(delete(state, saved.id, saved.sha256)).expect_err("gone");
    assert_eq!(gone.code(), IpcErrorCode::NotFound);
}

#[test]
fn deleting_all_removes_what_the_view_lists_for_the_project() {
    let home = ProjectHome::new("hooks-delete-all");
    let state = &home.state;

    save_global_and_demo(&home);

    let store = state.hooks.store().unwrap();
    let agent = |name: &str, scope: HookScope, project: Option<String>| {
        store
            .save(
                &draft(name, scope, project),
                HookAuthor::Agent,
                Default::default(),
                SystemTime::now(),
            )
            .expect("agent hook")
    };
    agent("Pending", HookScope::Global, None);
    agent("Elsewhere", HookScope::Project, path_arg(&home.stranger));

    let refused = block_on(delete_all(state, path_arg(&home.stranger))).expect_err("not a known project");
    assert_eq!(refused.code(), IpcErrorCode::Forbidden);

    let deleted = block_on(delete_all(state, path_arg(&home.project))).expect("deleted");
    assert_eq!(deleted, HooksDeleted { deleted: 3 });
    assert!(names(&block_on(list(state, path_arg(&home.project), false)).unwrap()).is_empty());

    // Another project's hooks stay.
    let left: Vec<String> = store
        .list(Default::default())
        .hooks
        .into_iter()
        .filter_map(|view| view.name)
        .collect();
    assert_eq!(left, ["Elsewhere"]);
    assert_eq!(
        serde_json::to_value(deleted).unwrap(),
        json!({ "deleted": 3 }),
        "the shape the UI reads"
    );
}

#[test]
fn another_projects_hooks_over_its_limit_are_neither_listed_nor_deleted() {
    let home = ProjectHome::new("hooks-over-limit");
    let state = &home.state;

    save_global_and_demo(&home);

    // Another project holds one hook more than its limit, as agents would write them.
    let store = state.hooks.store().unwrap();
    let root = project_identity(&home.stranger).unwrap().to_string_lossy().into_owned();
    let elsewhere: Vec<String> = (1..=65).map(|number| format!("{number:032x}")).collect();

    for (index, id) in elsewhere.iter().enumerate() {
        let created = format!("2026-10-17T10:{:02}:{:02}Z", index / 60, index % 60);
        let file = json!({
            "version": 1, "id": id, "name": format!("Elsewhere {index}"), "scope": "project", "project": root,
            "event": "sessionStart", "action": { "type": "addContext", "text": "Context." }, "enabled": true,
            "createdAt": created, "createdBy": "agent", "updatedAt": created, "updatedBy": "agent"
        });
        std::fs::write(store.hooks_dir().join(format!("{id}.json")), file.to_string()).unwrap();
    }

    let over: Vec<String> = store
        .list(Default::default())
        .hooks
        .into_iter()
        .filter(|view| view.status == HookStatus::Invalid)
        .map(|view| view.id)
        .collect();
    assert_eq!(over, [elsewhere[64].clone()], "the 65th is over the limit");

    let demo = block_on(list(state, path_arg(&home.project), false)).expect("listed");
    assert_eq!(names(&demo), ["Everywhere", "Demo only"]);

    let deleted = block_on(delete_all(state, path_arg(&home.project))).expect("deleted");
    assert_eq!(deleted, HooksDeleted { deleted: 2 });

    let left: Vec<String> = store
        .list(Default::default())
        .hooks
        .into_iter()
        .map(|view| view.id)
        .collect();
    assert_eq!(left.len(), 65);
    assert!(elsewhere.iter().all(|id| left.contains(id)));
}

#[test]
fn approving_a_changed_file_is_a_conflict() {
    let home = ProjectHome::new("hooks-approve");
    let state = &home.state;
    let store = state.hooks.store().unwrap();
    let pending = store
        .save(
            &draft("Agent rule", HookScope::Global, None),
            HookAuthor::Agent,
            Default::default(),
            SystemTime::now(),
        )
        .expect("agent hook");

    let stale = block_on(approve(state, pending.id.clone(), "0".repeat(64))).expect_err("another version");
    assert_eq!(stale.code(), IpcErrorCode::Conflict);

    let approved = block_on(approve(state, pending.id, pending.sha256)).expect("approved");
    assert_eq!(
        (approved.status, approved.approval),
        (HookStatus::Active, HookApproval::Approved)
    );
}

#[test]
fn the_watcher_reports_changes_and_drops_approvals_of_removed_files() {
    let home = ProjectHome::new("hooks-watch");
    let state = &home.state;
    let hooks = &state.hooks;
    let (channel, received) = collecting_channel();
    state.hooks.listen(channel);

    assert!(!hooks.poll(), "the first look only takes the fingerprint");
    assert!(!hooks.poll());

    let saved = block_on(save(state, draft("Style", HookScope::Global, None))).expect("saved");
    assert!(hooks.poll());
    assert_eq!(*received.lock().unwrap(), [json!({ "kind": "changed" })]);
    assert!(!hooks.poll());

    // An agent removes the file and writes it back byte for byte: the approval is gone.
    let store = hooks.store().unwrap();
    let path = store.hooks_dir().join(format!("{}.json", saved.id));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();

    assert!(hooks.poll());
    assert_eq!(received.lock().unwrap().len(), 2);

    std::fs::write(&path, bytes).unwrap();
    assert!(hooks.poll());

    let listing = block_on(list(state, None, false)).expect("listed");
    assert_eq!(
        (listing.hooks[0].status, listing.hooks[0].approval),
        (HookStatus::Pending, HookApproval::Never)
    );
}
