//! What `harness-desktop mcp` shows an agent of routines (see "Agent rules" in
//! `docs/routines-format.md`): only the global ones and those of the project the server works
//! in, so an agent never reads another project's prompts and commands.

use crate::routine_support;

use std::time::SystemTime;

use harness_business::hooks_mcp::Server;
use harness_business::model::{HookAuthor, HookScope};
use pretty_assertions::assert_eq;
use routine_support::{Root, command_draft, mcp_call as call, root};
use serde_json::json;

/// A project folder next to the server's own, canonical.
fn other_project(root: &Root) -> String {
    let other = root.temp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();

    dunce::canonicalize(other).unwrap().to_string_lossy().into_owned()
}

#[test]
fn another_projects_routines_are_neither_listed_nor_read() {
    let root = root();
    let other = other_project(&root);

    let mut theirs = command_draft("Their nightly build");
    theirs.scope = HookScope::Project;
    theirs.project = Some(other.clone());

    let save = |draft| {
        root.store
            .save(draft, HookAuthor::Desktop, SystemTime::now())
            .unwrap()
            .id
    };
    let (theirs, global) = (save(&theirs), save(&command_draft("Everywhere")));

    let mut server = Server::new(None, Some(root.project.clone().into())).with_routines(Some(root.store.clone()));

    let (refused, listed) = call(&mut server, "routine_list", json!({ "project": other }));
    assert!(refused && listed.contains("project it works in"), "{listed}");

    let (refused, hidden) = call(&mut server, "routine_get", json!({ "id": theirs }));
    assert_eq!((refused, hidden.as_str()), (true, "no routine has this id"));

    let (refused, shown) = call(&mut server, "routine_get", json!({ "id": global }));
    assert!(!refused && shown.contains(&global), "{shown}");

    let (_, here) = call(&mut server, "routine_list", json!({}));
    assert!(here.contains(&global) && !here.contains(&theirs), "{here}");
}

#[test]
fn another_projects_routine_is_neither_edited_nor_removed() {
    let root = root();
    let other = other_project(&root);

    // An agent wrote it, so an agent of that project could remove it.
    let mut theirs = command_draft("Their nightly build");
    theirs.scope = HookScope::Project;
    theirs.project = Some(other);
    let theirs = root.store.save(&theirs, HookAuthor::Agent, SystemTime::now()).unwrap();

    let mut server = Server::new(None, Some(root.project.clone().into())).with_routines(Some(root.store.clone()));

    let edit = json!({ "id": theirs.id, "sha256": theirs.sha256, "name": "Renamed" });
    let (refused, edited) = call(&mut server, "routine_update", edit);
    assert_eq!((refused, edited.as_str()), (true, "no routine has this id"));

    let removal = json!({ "id": theirs.id, "sha256": theirs.sha256 });
    let (refused, removed) = call(&mut server, "routine_remove", removal);
    assert_eq!((refused, removed.as_str()), (true, "no routine has this id"));

    assert_eq!(root.store.get(&theirs.id).unwrap().sha256, theirs.sha256, "untouched");
}
