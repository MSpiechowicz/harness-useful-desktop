//! The dashboard's MCP server under its old name `harness`, renamed by the launch migration: through
//! the harness's CLI where only it writes the file (Claude Code), in place for the others, never
//! over an entry the user owns, and once per harness.

use crate::claude_support::write_json;
use harness_business::ecosystem::{McpFile, Shape, rename_dashboard_mcp_file};
use harness_business::model::{HarnessId, SkillsMigrationFailure, SkillsMigrationReport};
use pretty_assertions::assert_eq;
use serde_json::json;

use super::{ALL, Executor, Machine, migrated, write};

const DASHBOARD_ENTRY: &str = r#"{"type":"stdio","command":"/home/u/.local/bin/harness-dashboard","args":["mcp"]}"#;

fn dashboard_entry() -> serde_json::Value {
    serde_json::from_str(DASHBOARD_ENTRY).unwrap()
}

fn claude_global(machine: &Machine, servers: &serde_json::Value) {
    write_json(
        &machine.home.path().join(".claude.json"),
        &json!({ "mcpServers": servers }),
    );
}

#[tokio::test]
async fn a_claude_server_named_harness_that_runs_the_dashboard_is_renamed_through_the_cli() {
    let machine = Machine::new();
    let entry = dashboard_entry();
    claude_global(
        &machine,
        &json!({ "harness": entry, "other": { "type": "stdio", "command": "x", "args": [] } }),
    );
    let executor = Executor::default();

    let report = machine.migrate(&ALL, None, &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Claude]), "no skills release is needed");
    assert_eq!(
        executor.calls(),
        [
            format!("Claude cli: mcp add-json --scope user harness-dashboard {DASHBOARD_ENTRY}"),
            "Claude cli: mcp remove --scope user harness".to_owned(),
        ]
    );

    // Recorded: not looked at again, even when the entry comes back.
    machine.migrate(&ALL, None, &executor).await;
    assert_eq!(executor.calls().len(), 2);
}

#[tokio::test]
async fn a_server_named_harness_that_runs_something_else_is_left_alone() {
    let machine = Machine::new();
    claude_global(
        &machine,
        &json!({ "harness": { "type": "stdio", "command": "other-tool", "args": ["mcp"] } }),
    );
    let executor = Executor::default();

    assert_eq!(
        machine.migrate(&ALL, None, &executor).await,
        SkillsMigrationReport::default()
    );
    assert_eq!(executor.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn a_different_harness_dashboard_server_keeps_the_old_one_where_it_is() {
    let machine = Machine::new();
    let entry = dashboard_entry();
    claude_global(
        &machine,
        &json!({ "harness": entry, "harness-dashboard": { "type": "stdio", "command": "mine", "args": [] } }),
    );
    let executor = Executor::default();

    machine.migrate(&ALL, None, &executor).await;

    assert_eq!(executor.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn an_identical_harness_dashboard_server_means_only_the_old_one_is_removed() {
    let machine = Machine::new();
    let entry = dashboard_entry();
    claude_global(
        &machine,
        &json!({ "harness": entry.clone(), "harness-dashboard": entry }),
    );
    let executor = Executor::default();

    let report = machine.migrate(&ALL, None, &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Claude]));
    assert_eq!(executor.calls(), ["Claude cli: mcp remove --scope user harness"]);
}

#[tokio::test]
async fn a_failed_add_removes_nothing_and_is_retried_at_the_next_launch() {
    let machine = Machine::new();
    let entry = dashboard_entry();
    claude_global(&machine, &json!({ "harness": entry }));
    let executor = Executor::failing(&["claude is busy"]);

    let report = machine.migrate(&ALL, None, &executor).await;

    assert_eq!(report.migrated, []);
    assert_eq!(
        report.failed,
        [SkillsMigrationFailure {
            harness: HarnessId::Claude,
            reason: "The harness-dashboard MCP server was not added: claude is busy".to_owned()
        }]
    );
    assert_eq!(executor.calls().len(), 1, "the old server is not removed");

    assert_eq!(
        machine.migrate(&ALL, None, &executor).await,
        migrated(&[HarnessId::Claude])
    );
}

#[tokio::test]
async fn an_omp_mcp_file_is_renamed_in_place_without_the_cli() {
    let machine = Machine::new();
    let path = machine.home.path().join(".omp/agent/mcp.json");
    write(
        &path,
        r#"{"mcpServers":{"first":{"command":"a","args":[]},"harness":{"command":"harness-dashboard","args":["mcp"]}},"other":1}"#,
    );
    let executor = Executor::default();

    let report = machine.migrate(&ALL, None, &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Omp]));
    assert_eq!(executor.calls(), Vec::<String>::new());

    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        written,
        json!({
            "mcpServers": {
                "first": { "command": "a", "args": [] },
                "harness-dashboard": { "command": "harness-dashboard", "args": ["mcp"] },
            },
            "other": 1,
        })
    );
}

#[test]
fn an_opencode_two_server_keeps_its_switch_in_the_new_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opencode.json");
    write(
        &path,
        r#"{"mcp":{"harness":{"type":"local","command":["harness-dashboard","mcp"],"enabled":false}}}"#,
    );
    let file = McpFile {
        path: path.clone(),
        also_read: Vec::new(),
        shape: Shape::OpenCodeV2,
    };

    assert_eq!(rename_dashboard_mcp_file(&file), Ok(true));

    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        written,
        json!({ "mcp": { "servers": { "harness-dashboard": {
            "type": "local", "command": ["harness-dashboard", "mcp"], "disabled": true
        } } } })
    );
}
