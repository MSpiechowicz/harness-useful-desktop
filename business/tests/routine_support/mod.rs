//! Routines for the tests (`routines_*.rs`): a temporary routines root with its team folder,
//! drafts, team rows as `harness-memory routines` lists them, and the shared fixtures.
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_business::model::{
    ApprovalMode, HarnessId, HookScope, RoutineAction, RoutineDraft, RoutineView, RoutineWhen,
};
use harness_business::routines::team::{TeamFolder, TeamRow};
use harness_business::routines::{RoutineStore, sha256_hex};
use serde_json::{Value, json};
use tempfile::TempDir;

/// The repository key of the tests' project.
pub const REPO: &str = "git:example.com/team/app";

/// A routine id ending in `n`.
pub fn rid(n: u16) -> String {
    format!("{:028x}{n:04x}", 0xabc)
}

/// A routines root, its team folder and a project folder.
pub struct Root {
    pub temp: TempDir,
    pub store: RoutineStore,
    pub folder: TeamFolder,
    pub project: String,
}

pub fn root() -> Root {
    let temp = tempfile::tempdir().unwrap();
    let store = RoutineStore::new(temp.path().join("harness-routines"), temp.path().join("app-config"));
    let folder = TeamFolder::of(&store);

    let project = temp.path().join("app");
    std::fs::create_dir_all(project.join(".git")).unwrap();
    let project = dunce::canonicalize(project).unwrap().to_string_lossy().into_owned();

    Root {
        temp,
        store,
        folder,
        project,
    }
}

/// A moment of the tests, `secs` after a fixed start.
pub fn moment(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_792_000_000 + secs)
}

/// A global daily command routine.
pub fn command_draft(name: &str) -> RoutineDraft {
    RoutineDraft {
        id: None,
        expected_sha256: None,
        name: name.to_owned(),
        description: None,
        scope: HookScope::Global,
        project: None,
        when: RoutineWhen::Daily { at: "07:30".to_owned() },
        action: RoutineAction::Command {
            command: "make report".to_owned(),
            timeout_secs: None,
        },
        enabled: true,
    }
}

/// An OMP agent action, with `yolo` when `unattended`.
pub fn agent_action(unattended: bool) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Omp,
        model: None,
        prompt: "List what changed since yesterday.".to_owned(),
        approval_mode: unattended.then_some(ApprovalMode::Yolo),
        trust_once: false,
        max_minutes: None,
    }
}

/// The draft that edits `view` as it is.
pub fn edit(view: &RoutineView) -> RoutineDraft {
    let routine = view.routine.clone().expect("a valid routine");

    RoutineDraft {
        id: Some(view.id.clone()),
        expected_sha256: Some(view.sha256.clone()),
        name: routine.name,
        description: routine.description,
        scope: routine.scope,
        project: routine.project,
        when: routine.when,
        action: routine.action,
        enabled: routine.enabled,
    }
}

/// The file of routine `id`.
pub fn file(store: &RoutineStore, id: &str) -> PathBuf {
    store.routines_dir().join(format!("{id}.json"))
}

/// A team body: routine `id` named `name`, global or of [`REPO`], with `action`.
pub fn body(id: &str, name: &str, project: bool, action: Value) -> String {
    let (scope, project) = if project {
        ("project", json!(REPO))
    } else {
        ("global", Value::Null)
    };

    let routine = json!({
        "version": 1,
        "id": id,
        "name": name,
        "scope": scope,
        "project": project,
        "when": {"type": "everyHours", "every": 4},
        "action": action,
        "enabled": true,
        "createdAt": "2026-10-01T06:00:00Z",
        "createdBy": "desktop",
        "updatedAt": "2026-10-01T06:00:00Z",
        "updatedBy": "desktop",
    });

    serde_json::to_string_pretty(&routine).unwrap()
}

/// The row `harness-memory routines list` answers for `body`.
pub fn team_row(id: &str, body: String, project: bool) -> TeamRow {
    TeamRow {
        id: id.to_owned(),
        repo: project.then(|| REPO.to_owned()),
        sha256: sha256_hex(body.as_bytes()),
        author: "mira".to_owned(),
        created_at: "2026-10-01T06:00:00Z".to_owned(),
        updated_at: "2026-10-03T12:00:00Z".to_owned(),
        body,
    }
}

/// A command action as JSON.
pub fn command_json() -> Value {
    json!({"type": "command", "command": "npm run nightly"})
}

/// The routine fixtures folder.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/routines")
}

/// `(name, bytes)` of the fixtures whose name starts with `prefix`, by name.
pub fn fixtures_named(prefix: &str) -> Vec<(String, Vec<u8>)> {
    let mut found: Vec<(String, Vec<u8>)> = std::fs::read_dir(fixtures())
        .unwrap()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_owned();

            let wanted = path.is_file() && name.starts_with(prefix) && name.ends_with(".json");
            wanted.then(|| (name, std::fs::read(&path).unwrap()))
        })
        .collect();

    found.sort();
    assert!(!found.is_empty(), "no {prefix} fixtures");
    found
}

/// The `id` field of a fixture, when it has a string one.
pub fn fixture_id(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    Some(value.get("id")?.as_str()?.to_owned())
}

/// `tools/call` of `tool` on `server`: whether it was refused, and its text.
pub fn mcp_call(server: &mut harness_business::hooks_mcp::Server, tool: &str, arguments: Value) -> (bool, String) {
    let params = json!({ "name": tool, "arguments": arguments });
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": params });
    let result = server.handle(request).expect("answered")["result"].clone();

    let text = result["content"][0]["text"].as_str().unwrap_or_default().to_owned();
    (result["isError"] == json!(true), text)
}
