//! Wire format of the M4 contract types: extension actions, CLI results, dashboard availability
//! and results, and the ecosystem status.

use harness_business::model::{
    CliResult, DashboardAvailability, DashboardResult, DashboardTool, EcosystemId, EcosystemItem, ExtensionAction,
    InstallScope, InstallStep,
};
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Serializes `value`, checks it against `expected` and parses it back.
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T, expected: Value) {
    assert_eq!(serde_json::to_value(&value).unwrap(), expected);
    assert_eq!(serde_json::from_value::<T>(expected).unwrap(), value);
}

#[test]
fn extension_actions_are_tagged_by_type() {
    round_trip(
        ExtensionAction::PluginInstall {
            target: "harness-useful-sidebar@harness-useful-sidebar".to_owned(),
            scope: InstallScope::Project,
        },
        json!({
            "type": "pluginInstall",
            "target": "harness-useful-sidebar@harness-useful-sidebar",
            "scope": "project"
        }),
    );
    round_trip(
        ExtensionAction::PluginUpgrade { name: None },
        json!({ "type": "pluginUpgrade", "name": null }),
    );
    round_trip(
        ExtensionAction::MarketplaceAdd {
            source: "MSpiechowicz/harness-useful-skills".to_owned(),
        },
        json!({ "type": "marketplaceAdd", "source": "MSpiechowicz/harness-useful-skills" }),
    );
    round_trip(
        ExtensionAction::PackageUpdate {
            source: Some("npm:@demo/pi-tools".to_owned()),
        },
        json!({ "type": "packageUpdate", "source": "npm:@demo/pi-tools" }),
    );
}

#[test]
fn extension_actions_reject_unknown_types() {
    let unknown = serde_json::from_value::<ExtensionAction>(json!({ "type": "shell", "command": "rm -rf ~" }));
    assert!(unknown.is_err());

    let missing_scope = serde_json::from_value::<ExtensionAction>(json!({ "type": "pluginInstall", "target": "x" }));
    assert!(missing_scope.is_err());
}

#[test]
fn cli_results_use_camel_case_fields() {
    round_trip(
        CliResult {
            ok: false,
            summary: Some("✘ Failed to disable demo: Error: Plugin demo not found in runtime config".to_owned()),
            exit_code: Some(0),
            stdout: String::new(),
            stderr: "✘ Failed to disable demo: Error: Plugin demo not found in runtime config\n".to_owned(),
            truncated: false,
            timed_out: false,
            argv: vec![
                "omp".to_owned(),
                "plugin".to_owned(),
                "disable".to_owned(),
                "demo".to_owned(),
            ],
            command: vec![
                "omp".to_owned(),
                "plugin".to_owned(),
                "disable".to_owned(),
                "demo".to_owned(),
            ],
        },
        json!({
            "ok": false,
            "summary": "✘ Failed to disable demo: Error: Plugin demo not found in runtime config",
            "exitCode": 0,
            "stdout": "",
            "stderr": "✘ Failed to disable demo: Error: Plugin demo not found in runtime config\n",
            "truncated": false,
            "timedOut": false,
            "argv": ["omp", "plugin", "disable", "demo"],
            "command": ["omp", "plugin", "disable", "demo"]
        }),
    );
}

#[test]
fn dashboard_availability_is_tagged_by_type() {
    round_trip(DashboardAvailability::Available, json!({ "type": "available" }));
    round_trip(DashboardAvailability::NotInstalled, json!({ "type": "notInstalled" }));
    round_trip(
        DashboardAvailability::Failed {
            message: "harness-dashboard mcp exited during the handshake".to_owned(),
        },
        json!({ "type": "failed", "message": "harness-dashboard mcp exited during the handshake" }),
    );
}

#[test]
fn dashboard_results_carry_structured_content_as_data() {
    round_trip(
        DashboardResult {
            tool: DashboardTool::Limits,
            data: json!({ "windows": [], "note": "No plan-limit readings stored in the last 30 days." }),
            text: None,
            is_error: false,
        },
        json!({
            "tool": "limits",
            "data": { "windows": [], "note": "No plan-limit readings stored in the last 30 days." },
            "text": null,
            "isError": false
        }),
    );
}

#[test]
fn ecosystem_items_use_camel_case_fields() {
    let item = EcosystemItem {
        id: EcosystemId::Sidebar,
        name: "Useful Sidebar".to_owned(),
        tagline: "t".to_owned(),
        description: "d".to_owned(),
        repo_url: "https://github.com/MSpiechowicz/harness-useful-sidebar".to_owned(),
        docs_url: "https://github.com/MSpiechowicz/harness-useful-sidebar#readme".to_owned(),
        installed: false,
        version: None,
        via: Vec::new(),
        supported_on_this_os: false,
        install_steps: vec![InstallStep {
            label: "Add the marketplace".to_owned(),
            command: "omp plugin marketplace add MSpiechowicz/harness-useful-sidebar".to_owned(),
        }],
        notes: vec!["Linux and macOS only.".to_owned()],
    };

    round_trip(
        item,
        json!({
            "id": "sidebar",
            "name": "Useful Sidebar",
            "tagline": "t",
            "description": "d",
            "repoUrl": "https://github.com/MSpiechowicz/harness-useful-sidebar",
            "docsUrl": "https://github.com/MSpiechowicz/harness-useful-sidebar#readme",
            "installed": false,
            "version": null,
            "via": [],
            "supportedOnThisOs": false,
            "installSteps": [{
                "label": "Add the marketplace",
                "command": "omp plugin marketplace add MSpiechowicz/harness-useful-sidebar"
            }],
            "notes": ["Linux and macOS only."]
        }),
    );
}
