//! What `harness-desktop mcp` lets an agent do with hooks of another project (see "Agent rules"
//! in `docs/hooks-format.md`): `hook_get`, `hook_update` and `hook_remove` see only the global
//! hooks and those of the project the server works in, so an agent never reads, edits or removes
//! another project's hook by its id.

mod hook_store_support;

use std::path::PathBuf;
use std::time::SystemTime;

use harness_business::hooks::HookStore;
use harness_business::hooks_mcp::Server;
use harness_business::model::{HookAuthor, HookScope, HookView};
use hook_store_support::{MEMORY, draft};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

/// The hooks folder and two projects: the server's own (`repo`) and another one next to it.
struct Machine {
    _temp: TempDir,
    store: HookStore,
    repo: PathBuf,
    other: String,
}

fn machine() -> Machine {
    let temp = tempfile::tempdir().unwrap();
    let base = dunce::canonicalize(temp.path()).unwrap();

    let repo = base.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(base.join("other")).unwrap();

    Machine {
        store: HookStore::new(base.join("hooks-root"), base.join("app-config")),
        other: base.join("other").to_string_lossy().into_owned(),
        repo,
        _temp: temp,
    }
}

impl Machine {
    /// Saves a hook of the other project as `author`.
    fn theirs(&self, author: HookAuthor) -> HookView {
        let mut theirs = draft("Their context");
        theirs.scope = HookScope::Project;
        theirs.project = Some(self.other.clone());

        self.store.save(&theirs, author, MEMORY, SystemTime::now()).unwrap()
    }

    /// `tools/call` of `tool` on a server started in the own project: whether it was refused,
    /// and its text.
    fn call(&self, tool: &str, arguments: Value) -> (bool, String) {
        let mut server = Server::new(Some(self.store.clone()), Some(self.repo.clone())).with_capabilities(MEMORY);
        let params = json!({ "name": tool, "arguments": arguments });
        let answer = server
            .handle(json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": params }))
            .expect("answered");

        let result = &answer["result"];
        let text = result["content"][0]["text"].as_str().unwrap_or_default();

        (result["isError"] == json!(true), text.to_owned())
    }
}

#[test]
fn another_projects_hook_is_not_read() {
    let machine = machine();
    let theirs = machine.theirs(HookAuthor::Desktop).id;
    let global = machine
        .store
        .save(&draft("Everywhere"), HookAuthor::Desktop, MEMORY, SystemTime::now())
        .unwrap()
        .id;

    let (refused, hidden) = machine.call("hook_get", json!({ "id": theirs }));
    assert_eq!((refused, hidden.as_str()), (true, "no hook has this id"));

    let (refused, shown) = machine.call("hook_get", json!({ "id": global }));
    assert!(!refused && shown.contains(&global), "{shown}");

    let (_, here) = machine.call("hook_list", json!({}));
    assert!(here.contains(&global) && !here.contains(&theirs), "{here}");
}

#[test]
fn another_projects_hook_is_neither_edited_nor_removed() {
    let machine = machine();

    // An agent wrote it, so an agent of that project could remove it.
    let theirs = machine.theirs(HookAuthor::Agent);

    let edit = json!({ "id": theirs.id, "sha256": theirs.sha256, "name": "Renamed" });
    let (refused, edited) = machine.call("hook_update", edit);
    assert_eq!((refused, edited.as_str()), (true, "no hook has this id"));

    let removal = json!({ "id": theirs.id, "sha256": theirs.sha256 });
    let (refused, removed) = machine.call("hook_remove", removal);
    assert_eq!((refused, removed.as_str()), (true, "no hook has this id"));

    let kept = machine.store.get(&theirs.id, MEMORY).unwrap();
    assert_eq!(kept.sha256, theirs.sha256, "untouched");
}

#[test]
fn a_hook_of_the_own_project_is_still_edited_and_removed() {
    let machine = machine();

    let (refused, added) = machine.call(
        "hook_add",
        json!({ "name": "Ours", "event": "promptSubmit", "action": "addContext", "text": "Be brief.", "scope": "project" }),
    );
    assert!(!refused, "{added}");

    let added: Value = serde_json::from_str(&added).unwrap();
    let (id, sha256) = (&added["added"]["id"], &added["added"]["sha256"]);

    let (refused, edited) = machine.call("hook_update", json!({ "id": id, "sha256": sha256, "name": "Renamed" }));
    assert!(!refused, "{edited}");

    let edited: Value = serde_json::from_str(&edited).unwrap();
    let (refused, removed) = machine.call(
        "hook_remove",
        json!({ "id": id, "sha256": edited["updated"]["sha256"] }),
    );
    assert!(!refused, "{removed}");
}
