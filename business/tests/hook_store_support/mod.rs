//! A temporary hooks folder and hook drafts for the store tests (`hooks_store.rs`,
//! `hooks_store_folder.rs`).
#![allow(dead_code)]

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_business::hooks::{HookCapabilities, HookStore};
use harness_business::model::{HookAction, HookAuthor, HookDraft, HookEvent, HookScope, HookView};
use serde_json::{Value, json};
use tempfile::TempDir;

pub const MEMORY: HookCapabilities = HookCapabilities {
    memory_available: true,
    dashboard_available: false,
};
pub const NO_MEMORY: HookCapabilities = HookCapabilities {
    memory_available: false,
    dashboard_available: false,
};
pub const DESKTOP: HookAuthor = HookAuthor::Desktop;

pub struct Folder {
    _temp: TempDir,
    pub store: HookStore,
}

pub fn folder() -> Folder {
    let temp = tempfile::tempdir().unwrap();
    let store = HookStore::new(temp.path().join("harness-hooks"), temp.path().join("config"));

    Folder { _temp: temp, store }
}

pub fn at(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_791_000_000 + secs)
}

pub fn draft(name: &str) -> HookDraft {
    HookDraft {
        id: None,
        expected_sha256: None,
        name: name.to_owned(),
        description: None,
        scope: HookScope::Global,
        project: None,
        event: HookEvent::PromptSubmit,
        matcher: None,
        action: HookAction::AddContext {
            text: "Answer in short sentences.".to_owned(),
        },
        enabled: true,
    }
}

pub fn edit_of(view: &HookView) -> HookDraft {
    let hook = view.hook.clone().unwrap();

    HookDraft {
        id: Some(view.id.clone()),
        expected_sha256: Some(view.sha256.clone()),
        name: hook.name,
        description: hook.description,
        scope: hook.scope,
        project: hook.project,
        event: hook.event,
        matcher: hook.matcher,
        action: hook.action,
        enabled: hook.enabled,
    }
}

pub fn file_of(store: &HookStore, id: &str) -> PathBuf {
    store.hooks_dir().join(format!("{id}.json"))
}

pub fn view<'a>(views: &'a [HookView], id: &str) -> &'a HookView {
    views.iter().find(|view| view.id == id).unwrap()
}

/// An agent's rewrite of `id`: same JSON values, other formatting, `updatedBy` kept.
pub fn rewrite_same_values(store: &HookStore, id: &str) {
    let path = file_of(store, id);
    let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
}

/// A valid file an agent wrote, created at `created_at`.
pub fn agent_file(store: &HookStore, id: &str, created_at: &str, scope: Value, project: Value) {
    let value = json!({
        "version": 1, "id": id, "name": format!("Hook {id}"), "scope": scope, "project": project,
        "event": "sessionStart", "action": { "type": "addContext", "text": "Context." }, "enabled": true,
        "createdAt": created_at, "createdBy": "agent", "updatedAt": created_at, "updatedBy": "agent"
    });

    std::fs::create_dir_all(store.hooks_dir()).unwrap();
    std::fs::write(file_of(store, id), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

pub fn hex_id(number: usize) -> String {
    format!("{number:032x}")
}

/// Saves a desktop hook named `name` and returns it with the bytes written.
pub fn saved_with_bytes(folder: &Folder, name: &str) -> (HookView, Vec<u8>) {
    let saved = folder.store.save(&draft(name), DESKTOP, NO_MEMORY, at(0)).unwrap();
    let bytes = std::fs::read(file_of(&folder.store, &saved.id)).unwrap();

    (saved, bytes)
}
