//! Exports the JSON fixtures served by the UI mock transport (`ui/src/lib/ipc/mock/fixtures`).
//!
//! Run `cargo test -p harness_business export_mock_fixtures`; CI re-runs it and fails when the
//! committed files differ. Views and catalogs are real adapter output over the synthetic fixture
//! homes (`tests/fixtures`) with the demo project open; paths are rewritten to `/home/demo/...` so
//! the files are machine-independent. Detection depends on real binaries, so `harness_list.json`
//! and `project_recent.json` stay synthetic.

mod common;

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

    vec![
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
    ]
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
        let mut text = serde_json::to_string_pretty(&value).expect("serialize fixture");
        text.push('\n');

        fs::write(dir.join(name), text).expect("write fixture");
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
    let all = fixtures();
    let pi_config = &all
        .iter()
        .find(|(name, _)| *name == "config_view.pi.json")
        .expect("pi config fixture")
        .1;

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
