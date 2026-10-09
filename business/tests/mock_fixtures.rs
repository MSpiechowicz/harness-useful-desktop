//! Exports the JSON fixtures served by the UI mock transport (`ui/src/lib/ipc/mock/fixtures`).
//!
//! Run `cargo test -p harness_business export_mock_fixtures`; CI re-runs it and fails when the
//! committed files differ. Views and catalogs are real adapter output over the synthetic fixture
//! homes (`tests/fixtures`) with the demo project open; paths are rewritten to `/home/demo/...` so
//! the files are machine-independent. Detection depends on real binaries, so `harness_list.json`
//! and `project_recent.json` stay synthetic. The agent chat fixtures under `agent/` are engine
//! output: `fake_agent` scenarios run through the real session engine (`tests/samples`). The
//! session, git and review fixtures under `sessions/`, `git/` and `review/` are synthetic DTO
//! samples (`tests/samples/m3`).

mod agent_support;
mod common;
mod samples;

use std::fs;
use std::path::PathBuf;

use common::{CANARY, DEMO_HOME, DEMO_PROJECT, demo_project, env_at, exported, omp_home, pi_home};
use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{Detection, HarnessSummary, ProjectInfo, VersionSupport};
use serde::Serialize;
use serde_json::{Value, json};

/// Every fixture file name with its content, in a stable order.
fn fixtures() -> Vec<(&'static str, Value)> {
    let project = demo_project();
    let omp = omp_home();
    let pi = pi_home();
    let omp_env = env_at(&omp);
    let pi_env = env_at(&pi);

    let mut fixtures = vec![
        ("harness_list.json", to_json(harness_list())),
        (
            "config_view.omp.json",
            exported(Omp.config_view(&omp_env, Some(&project)).unwrap(), &omp, &project),
        ),
        (
            "config_view.pi.json",
            exported(Pi.config_view(&pi_env, Some(&project)).unwrap(), &pi, &project),
        ),
        (
            "extensions_view.omp.json",
            exported(Omp.extensions_view(&omp_env, Some(&project)).unwrap(), &omp, &project),
        ),
        (
            "extensions_view.pi.json",
            exported(Pi.extensions_view(&pi_env, Some(&project)).unwrap(), &pi, &project),
        ),
        ("commands_catalog.omp.json", to_json(Omp.catalog().merged(&[]))),
        ("commands_catalog.pi.json", to_json(Pi.catalog().merged(&[]))),
        ("project_recent.json", to_json(project_recent())),
    ];

    fixtures.extend(samples::agent_fixtures());
    fixtures.extend(samples::m3::m3_fixtures());
    fixtures
}

fn to_json(value: impl Serialize) -> Value {
    serde_json::to_value(value).expect("DTOs always serialize")
}

/// OMP installed and supported; PI not installed (its config left behind), with an install hint.
fn harness_list() -> Vec<HarnessSummary> {
    let omp = HarnessSummary {
        info: Omp.info(),
        capabilities: Omp.capabilities(),
        detection: Detection {
            installed: true,
            binary_path: Some(format!("{DEMO_HOME}/.bun/bin/omp")),
            is_batch_script: false,
            version: Some("18.6.1".to_owned()),
            version_support: VersionSupport::Supported,
            config_dir: Some(format!("{DEMO_HOME}/.omp")),
            config_dir_exists: true,
            install_hint: None,
            error: None,
        },
    };

    let pi = HarnessSummary {
        info: Pi.info(),
        capabilities: Pi.capabilities(),
        detection: Detection {
            installed: false,
            binary_path: None,
            is_batch_script: false,
            version: None,
            version_support: VersionSupport::Unknown,
            config_dir: Some(format!("{DEMO_HOME}/.pi")),
            config_dir_exists: true,
            install_hint: Some(Pi.install_hint()),
            error: None,
        },
    };

    vec![omp, pi]
}

/// Recent projects, most recent first: the demo repo, a plain folder and one that was deleted.
fn project_recent() -> Vec<ProjectInfo> {
    let recent = |path: &str, exists: bool, is_git_repo: bool| ProjectInfo {
        path: path.to_owned(),
        name: path.rsplit('/').next().unwrap_or(path).to_owned(),
        exists,
        is_git_repo,
    };

    vec![
        recent(DEMO_PROJECT, true, true),
        recent(&format!("{DEMO_HOME}/Notes/research"), true, false),
        recent(&format!("{DEMO_HOME}/Archive/legacy-api"), false, false),
    ]
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/ipc/mock/fixtures")
}

#[test]
fn export_mock_fixtures() {
    let dir = fixtures_dir();
    fs::create_dir_all(&dir).expect("create fixtures dir");

    for (name, value) in fixtures() {
        let path = dir.join(name);
        let mut text = serde_json::to_string_pretty(&value).expect("serialize fixture");
        text.push('\n');

        fs::create_dir_all(path.parent().expect("fixture has a parent")).expect("create fixture dir");
        fs::write(path, text).expect("write fixture");
    }
}

/// True when `value` or any nested value satisfies `predicate`.
fn any_node(value: &Value, predicate: &dyn Fn(&Value) -> bool) -> bool {
    if predicate(value) {
        return true;
    }

    match value {
        Value::Array(items) => items.iter().any(|item| any_node(item, predicate)),
        Value::Object(map) => map.values().any(|item| any_node(item, predicate)),
        _ => false,
    }
}

#[test]
fn fixtures_hide_every_fixture_secret() {
    for (name, value) in fixtures() {
        assert!(!value.to_string().contains(CANARY), "{name} exposes the fixture canary");
    }
}

#[test]
fn fixtures_show_secret_kinds_instead_of_values() {
    let pi_config = &fixture("config_view.pi.json");

    let secret = |kind: Value| move |node: &Value| node["kind"] == "secret" && node["value"] == kind;

    assert!(any_node(pi_config, &secret(json!({ "type": "command" }))));
    assert!(any_node(
        pi_config,
        &secret(json!({ "type": "envRef", "name": "OPENAI_API_KEY" }))
    ));
    assert!(any_node(pi_config, &secret(json!({ "type": "value" }))));
}

#[test]
fn fixtures_contain_no_machine_paths() {
    let machine_root = env!("CARGO_MANIFEST_DIR");

    for (name, value) in fixtures() {
        assert!(
            !value.to_string().contains(machine_root),
            "{name} contains a machine path"
        );
    }
}

fn fixture(name: &str) -> Value {
    fixtures()
        .into_iter()
        .find(|(fixture_name, _)| *fixture_name == name)
        .unwrap_or_else(|| panic!("missing fixture {name}"))
        .1
}

#[test]
fn agent_streams_start_ready_and_number_events_without_gaps() {
    for (name, value) in fixtures() {
        if !name.starts_with("agent/stream_") {
            continue;
        }

        let events = value.as_array().expect("a stream is an array");
        assert_eq!(events[0]["event"]["type"], "ready", "{name}");

        for (index, envelope) in events.iter().enumerate() {
            assert_eq!(envelope["seq"], index + 1, "{name} event {index}");
        }
    }
}

#[test]
fn denied_approval_replays_the_approval_stream_up_to_the_request() {
    let approved = fixture("agent/stream_approval.json");
    let denied = fixture("agent/stream_approval_denied.json");
    let (approved, denied) = (approved.as_array().unwrap(), denied.as_array().unwrap());

    let request = approved
        .iter()
        .position(|envelope| envelope["event"]["type"] == "uiRequest")
        .expect("approval request");

    assert_eq!(approved[..=request], denied[..=request]);
    assert_ne!(approved[request + 1], denied[request + 1]);
}

fn ids(nodes: &[Value], predicate: impl Fn(&Value) -> bool) -> Vec<&str> {
    nodes
        .iter()
        .filter(|node| predicate(node))
        .map(|node| node["id"].as_str().expect("node id"))
        .collect()
}

#[test]
fn session_tree_marks_the_path_from_the_root_to_the_leaf() {
    let tree = fixture("sessions/session_tree.omp.json");
    let nodes = tree["nodes"].as_array().expect("nodes");
    let node = |id: &str| nodes.iter().find(|node| node["id"] == id).expect("known node");

    let mut path = Vec::new();
    let mut current = tree["leafId"].as_str();
    while let Some(id) = current {
        path.push(id);
        current = node(id)["parentId"].as_str();
    }
    path.reverse();

    assert_eq!(ids(nodes, |node| node["onActivePath"] == true), path);

    for parent in nodes {
        let children = ids(nodes, |node| node["parentId"] == parent["id"]).len();
        assert_eq!(parent["childCount"], children, "{}", parent["id"]);
    }

    let branch_points = ids(nodes, |node| node["childCount"].as_u64() > Some(1)).len();
    assert_eq!(tree["branchCount"], branch_points);
    assert!(branch_points >= 2);
}

#[test]
fn session_transcript_follows_the_active_path() {
    let tree = fixture("sessions/session_tree.omp.json");
    let page = fixture("sessions/session_messages.omp.json");
    let active = ids(tree["nodes"].as_array().unwrap(), |node| node["onActivePath"] == true);
    let messages = page["messages"].as_array().expect("messages");

    let message_ids: Vec<&str> = messages.iter().map(|message| message["id"].as_str().unwrap()).collect();
    let on_path: Vec<&str> = active.into_iter().filter(|id| message_ids.contains(id)).collect();

    assert_eq!(message_ids, on_path);
    assert!(page["total"].as_u64() >= Some(messages.len() as u64));
}

/// Counts the hunk's lines whose kind is one of `kinds`.
fn lines_of(hunk: &Value, kinds: &[&str]) -> u64 {
    let lines = hunk["lines"].as_array().expect("lines");
    lines
        .iter()
        .filter(|line| kinds.iter().any(|kind| line["kind"] == *kind))
        .count() as u64
}

/// The line numbers of `side` (`oldNo`/`newNo`) in order, skipping lines without one.
fn numbers(hunk: &Value, side: &str) -> Vec<u64> {
    let lines = hunk["lines"].as_array().expect("lines");
    lines.iter().filter_map(|line| line[side].as_u64()).collect()
}

#[test]
fn diff_hunks_agree_with_their_ranges_and_file_totals() {
    let diff = fixture("git/git_diff.json");

    for file in diff.as_array().expect("files") {
        let hunks = file["hunks"].as_array().expect("hunks");
        let total = |kind: &str| hunks.iter().map(|hunk| lines_of(hunk, &[kind])).sum::<u64>();

        assert_eq!(file["additions"], total("add"), "{}", file["path"]);
        assert_eq!(file["deletions"], total("remove"), "{}", file["path"]);
        assert!(file["isBinary"] == false || hunks.is_empty(), "{}", file["path"]);

        for hunk in hunks {
            for (side, start, count, kind) in [
                ("oldNo", "oldStart", "oldLines", "remove"),
                ("newNo", "newStart", "newLines", "add"),
            ] {
                let count = hunk[count].as_u64().unwrap();
                let first = hunk[start].as_u64().unwrap().max(1);

                assert_eq!(lines_of(hunk, &["context", kind]), count, "{}", hunk["header"]);
                assert_eq!(
                    numbers(hunk, side),
                    (first..first + count).collect::<Vec<_>>(),
                    "{}",
                    hunk["header"]
                );
            }
        }
    }
}

#[test]
fn diff_fixture_covers_every_rendered_case() {
    let diff = fixture("git/git_diff.json");
    let files = diff.as_array().expect("files");
    let has = |predicate: &dyn Fn(&Value) -> bool| any_node(&diff, predicate);

    for status in ["modified", "added", "renamed", "untracked"] {
        assert!(files.iter().any(|file| file["status"] == status), "{status}");
    }
    assert!(files.iter().any(|file| file["isBinary"] == true));
    assert!(files.iter().any(|file| file["hunks"].as_array().unwrap().len() >= 2));
    assert!(has(&|node| node["kind"] == "remove"));
    assert!(has(&|node| node["kind"] == "noNewline"));
}

#[test]
fn composed_review_reports_its_prompt_size() {
    let review = fixture("review/review_composed.json");
    let prompt = review["prompt"].as_str().expect("prompt");

    assert_eq!(review["bytes"], prompt.len());
    assert_eq!(review["commentCount"], prompt.matches("\n- Line").count());
    assert_eq!(review["fileCount"], prompt.matches("\n### ").count());
}
