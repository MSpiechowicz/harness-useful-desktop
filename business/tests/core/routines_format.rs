//! The routine file format against the shared fixtures (`tests/fixtures/routines`): every
//! `valid-*.json` is accepted and every `invalid-*.json` rejected, by content and when loaded from
//! a routines folder as `<id>.json`; the approvals fixture holds the real SHA-256 of the files
//! beside it; the writer keeps unknown fields and drops the reserved ones; the root is found as
//! the format doc says.

use crate::routine_support;

use std::collections::BTreeMap;

use harness_business::env::{Env, OsKind};
use harness_business::model::{ApprovalMode, HarnessId, RoutineAction, RoutineStatus, RoutineWhen};
use harness_business::routines::format::{check_when, parse_team_routine};
use harness_business::routines::{RoutineStore, parse_routine_file, routine_bytes, routines_root, sha256_hex};
use harness_business::signed_docs::is_doc_id;
use pretty_assertions::assert_eq;
use routine_support::{fixture_id, fixtures, fixtures_named};
use serde_json::{Value, json};

#[test]
fn every_valid_fixture_is_accepted() {
    for (name, bytes) in fixtures_named("valid-") {
        let id = fixture_id(&bytes).unwrap();

        if let Err(problem) = parse_routine_file(&id, &bytes) {
            panic!("{name} was rejected: {problem}");
        }
    }
}

#[test]
fn every_invalid_fixture_is_rejected() {
    for (name, bytes) in fixtures_named("invalid-") {
        let id = fixture_id(&bytes).unwrap_or_else(|| "0".repeat(32));

        assert!(parse_routine_file(&id, &bytes).is_err(), "{name} was accepted");
    }
}

#[test]
fn fixtures_load_from_a_routines_folder_as_their_id() {
    let temp = tempfile::tempdir().unwrap();
    let store = RoutineStore::new(temp.path().join("root"), temp.path().join("config"));
    std::fs::create_dir_all(store.routines_dir()).unwrap();

    let mut expected = BTreeMap::new();

    for (index, (name, bytes)) in fixtures_named("").into_iter().enumerate() {
        // Each file under its own id, so equal ids in fixtures do not collide; a malformed id stays.
        let stem = format!("{index:032x}");
        let text = String::from_utf8_lossy(&bytes);
        let renamed = match fixture_id(&bytes).filter(|id| is_doc_id(id)) {
            Some(id) => text.replace(&format!("\"{id}\""), &format!("\"{stem}\"")),
            None => text.into_owned(),
        };

        std::fs::write(store.routines_dir().join(format!("{stem}.json")), renamed).unwrap();
        expected.insert(stem, name);
    }

    let listing = store.list();
    assert_eq!(listing.routines.len(), expected.len());

    for view in listing.routines {
        let name = &expected[&view.id];
        let invalid = view.status == RoutineStatus::Invalid;

        assert_eq!(invalid, name.starts_with("invalid-"), "{name}: {:?}", view.problem);
        assert_eq!(
            view.status == RoutineStatus::Pending,
            !invalid,
            "{name}: never approved"
        );
    }
}

#[test]
fn the_approvals_fixture_holds_three_real_hashes_and_one_stale_one() {
    let text = std::fs::read_to_string(fixtures().join("approvals/valid-approvals.json")).unwrap();
    let approvals: Value = serde_json::from_str(&text).unwrap();

    let by_id: BTreeMap<String, String> = fixtures_named("valid-")
        .into_iter()
        .map(|(_, bytes)| (fixture_id(&bytes).unwrap(), sha256_hex(&bytes)))
        .collect();

    let entries = approvals["approvals"].as_object().unwrap();
    let real = entries
        .iter()
        .filter(|(id, hash)| by_id.get(*id).map(String::as_str) == hash.as_str())
        .count();

    assert_eq!((entries.len(), real), (4, 3));
}

#[test]
fn null_means_the_default_of_each_optional_action_field() {
    let agent = json!({"type": "agent", "harness": "pi", "prompt": "Hi", "model": null,
        "approvalMode": null, "trustOnce": null, "maxMinutes": null});
    let command = json!({"type": "command", "command": "true", "timeoutSecs": null});

    for (action, expected) in [
        (
            agent,
            RoutineAction::Agent {
                harness: HarnessId::Pi,
                model: None,
                prompt: "Hi".to_owned(),
                approval_mode: None,
                trust_once: false,
                max_minutes: None,
            },
        ),
        (
            command,
            RoutineAction::Command {
                command: "true".to_owned(),
                timeout_secs: None,
            },
        ),
    ] {
        let bytes = routine_with(json!({"action": action}));
        assert_eq!(parse_routine_file(&"1".repeat(32), &bytes).unwrap().action, expected);
    }
}

#[test]
fn harness_rules_and_value_types_are_checked() {
    let cases = [
        (
            json!({"type": "agent", "harness": "pi", "prompt": "x", "approvalMode": "write"}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "approvalMode": "write"}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "approvalMode": "yolo"}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "opencode", "prompt": "x", "approvalMode": "yolo"}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "claude", "prompt": "x", "approvalMode": "alwaysAsk"}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "claude", "prompt": "x", "approvalMode": "write"}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "claude", "prompt": "x", "approvalMode": "yolo"}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "claude", "prompt": "x", "approvalMode": "auto"}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "approvalMode": "auto"}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "opencode", "prompt": "x", "approvalMode": "auto"}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "pi", "prompt": "x", "approvalMode": "auto"}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "trustOnce": false}),
            true,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "trustOnce": true}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "maxMinutes": 30.0}),
            false,
        ),
        (
            json!({"type": "agent", "harness": "omp", "prompt": "x", "maxMinutes": -1}),
            false,
        ),
        (json!({"type": "command", "command": "x", "timeoutSecs": 3600}), true),
        (
            json!({"type": "script", "name": "a".repeat(64), "scope": "global"}),
            true,
        ),
        (
            json!({"type": "script", "name": "a".repeat(65), "scope": "global"}),
            false,
        ),
    ];

    for (action, valid) in cases {
        let bytes = routine_with(json!({"action": action}));
        assert_eq!(parse_routine_file(&"1".repeat(32), &bytes).is_ok(), valid, "{action}");
    }
}

#[test]
fn a_rewrite_keeps_unknown_fields_and_drops_the_reserved_ones() {
    let (_, bytes) = fixtures_named("valid-unknown-fields").into_iter().next().unwrap();
    let id = fixture_id(&bytes).unwrap();
    let routine = parse_routine_file(&id, &bytes).unwrap();
    let base: serde_json::Map<String, Value> = serde_json::from_slice(&bytes).unwrap();

    let rewritten = routine_bytes(&routine, base);
    let value: Value = serde_json::from_slice(&rewritten).unwrap();

    assert_eq!(parse_routine_file(&id, &rewritten).unwrap(), routine);
    assert_eq!(value["futureField"], json!({"keep": ["me"]}));
    assert!(rewritten.ends_with(b"}\n"));

    let mut reserved: serde_json::Map<String, Value> = serde_json::from_slice(&bytes).unwrap();
    reserved.insert("seed".to_owned(), Value::Null);
    reserved.insert("requires".to_owned(), Value::Null);

    let cleaned: Value = serde_json::from_slice(&routine_bytes(&routine, reserved)).unwrap();
    assert!(cleaned.get("seed").is_none() && cleaned.get("requires").is_none());
}

#[test]
fn a_new_file_is_written_in_the_doc_order() {
    let bytes = routine_with(json!({}));
    let routine = parse_routine_file(&"1".repeat(32), &bytes).unwrap();

    let written: Value = serde_json::from_slice(&routine_bytes(&routine, serde_json::Map::new())).unwrap();
    let keys: Vec<&str> = written.as_object().unwrap().keys().map(String::as_str).collect();

    assert_eq!(
        keys,
        [
            "version",
            "id",
            "name",
            "description",
            "scope",
            "project",
            "when",
            "action",
            "enabled",
            "createdAt",
            "createdBy",
            "updatedAt",
            "updatedBy",
        ]
    );
    assert_eq!(written["action"], json!({"type": "command", "command": "make report"}));
}

#[test]
fn a_team_body_names_a_repository_key_not_a_path() {
    let team = routine_with(json!({"scope": "project", "project": "git:example.com/team/app"}));
    let local = routine_with(json!({"scope": "project", "project": "/home/me/app"}));
    let id = "1".repeat(32);

    assert!(parse_team_routine(&id, &team).is_ok());
    assert!(parse_team_routine(&id, &local).is_err());
    assert!(parse_routine_file(&id, &team).is_err());
}

#[test]
fn a_local_project_is_a_rooted_plain_path_of_limited_length() {
    let id = "1".repeat(32);
    let root = "/work/app";
    let project = |path: &str| parse_routine_file(&id, &routine_with(json!({"scope": "project", "project": path})));

    assert!(project(root).is_ok(), "a POSIX root reads on every system");

    if cfg!(windows) {
        assert!(project("C:\\work\\app").is_ok(), "a root with its drive");
        assert!(project("\\\\server\\share\\app").is_ok(), "a root on a share");
        assert!(project("C:work\\app").is_err(), "a drive without its root");
    }

    let long = format!("{root}{}", "a".repeat(4096));
    let control = format!("{root}\u{7}");
    let invisible = format!("{root}\u{202E}ppa");

    for (bad, why) in [
        ("work/app", "not absolute"),
        ("", "empty"),
        (control.as_str(), "a control character"),
        (invisible.as_str(), "an invisible character"),
        (long.as_str(), "over 4096 bytes"),
    ] {
        assert!(project(bad).is_err(), "{why}: {bad:?} was accepted");
    }
}

#[test]
fn schedules_are_checked_as_in_a_file() {
    assert!(check_when(&RoutineWhen::Daily { at: "23:59".to_owned() }).is_ok());
    assert!(check_when(&RoutineWhen::Daily { at: "7:30".to_owned() }).is_err());
    assert!(check_when(&RoutineWhen::EveryMinutes { every: 721 }).is_err());
    assert!(
        check_when(&RoutineWhen::Cron {
            expression: "0 9 * * fri-sun".to_owned()
        })
        .is_ok()
    );
    assert!(
        check_when(&RoutineWhen::Cron {
            expression: "0 0 31 2 *".to_owned()
        })
        .is_err()
    );
}

#[test]
fn approval_modes_use_the_app_names() {
    for (mode, expected) in [("alwaysAsk", Some(ApprovalMode::AlwaysAsk)), ("always-ask", None)] {
        let bytes = routine_with(json!({"action": {"type": "agent", "harness": "omp", "prompt": "x",
            "approvalMode": mode}}));
        let parsed = parse_routine_file(&"1".repeat(32), &bytes).ok();

        let found = parsed.and_then(|routine| match routine.action {
            RoutineAction::Agent { approval_mode, .. } => approval_mode,
            _ => None,
        });
        assert_eq!(found, expected);
    }
}

#[test]
fn the_root_is_the_override_or_the_platform_data_folder() {
    let linux = Env::for_tests("/home/u").with_os(OsKind::Linux);
    assert_eq!(
        routines_root(&linux),
        Some(std::path::PathBuf::from("/home/u/.local/share/harness-routines"))
    );

    let custom = linux.clone().with_var("HARNESS_ROUTINES_DIR", "/srv/routines");
    assert_eq!(routines_root(&custom), Some(std::path::PathBuf::from("/srv/routines")));

    let relative = linux.with_var("HARNESS_ROUTINES_DIR", "routines");
    assert_eq!(
        routines_root(&relative),
        Some(std::path::PathBuf::from("/home/u/.local/share/harness-routines"))
    );

    let windows = Env::for_tests("C:/Users/u").with_os(OsKind::Windows);
    assert_eq!(routines_root(&windows), None);
}

/// A valid global command routine with id `1…1`, `changes` merged over it.
fn routine_with(changes: Value) -> Vec<u8> {
    let mut routine = json!({
        "version": 1,
        "id": "1".repeat(32),
        "name": "Report",
        "description": null,
        "scope": "global",
        "project": null,
        "when": {"type": "daily", "at": "07:30"},
        "action": {"type": "command", "command": "make report"},
        "enabled": true,
        "createdAt": "2026-10-17T10:00:00Z",
        "createdBy": "desktop",
        "updatedAt": "2026-10-17T10:00:00Z",
        "updatedBy": "desktop",
    });

    for (key, value) in changes.as_object().unwrap() {
        routine[key] = value.clone();
    }

    serde_json::to_vec_pretty(&routine).unwrap()
}

#[test]
fn an_auto_routine_is_claude_codes_only_runs_unattended_and_keeps_its_mode_on_a_rewrite() {
    let bytes =
        routine_with(json!({"action": {"type": "agent", "harness": "claude", "prompt": "x", "approvalMode": "auto"}}));
    let routine = parse_routine_file(&"1".repeat(32), &bytes).unwrap();

    assert!(matches!(
        routine.action,
        RoutineAction::Agent {
            approval_mode: Some(ApprovalMode::Auto),
            ..
        }
    ));
    assert!(routine.action.runs_unattended(), "its classifier runs tools unasked");

    let rewritten: Value = serde_json::from_slice(&routine_bytes(&routine, serde_json::Map::new())).unwrap();
    assert_eq!(rewritten["action"]["approvalMode"], "auto");

    let omp =
        routine_with(json!({"action": {"type": "agent", "harness": "omp", "prompt": "x", "approvalMode": "auto"}}));
    let refused = parse_routine_file(&"1".repeat(32), &omp).unwrap_err().to_string();
    assert!(
        refused.contains("action.approvalMode \"auto\" is for claude only"),
        "{refused}"
    );
}
