//! Import of "Share setup" for OpenCode 2: plugins install with `opencode plugin add <spec>` and
//! are found in `plugins` and the legacy `plugin` of every layer OpenCode 2 reads; servers are
//! found under `mcp.servers` and the legacy `mcp.<name>`, written under `mcp.servers`, and checked
//! first against the names OpenCode 2 merges, asked with `opencode api --standalone GET
//! /api/config` (never its background service).

#[path = "sync_support/import.rs"]
mod import_support;

use std::ffi::OsString;

use harness_business::harness::{HarnessRegistry, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::harnesses::opencode::{PROBE_ARGS_V2, PROBE_MAX_BYTES, PROBE_TIMEOUT};
use harness_business::model::{HarnessId, SyncBlockReason, SyncItemStatus, SyncNote, SyncOutcome, SyncStep};
use harness_business::process::ExecOutput;
use harness_business::sync::codec::to_json_pretty;
use harness_business::sync::{build_bundle, collect_export, export_preview};
use import_support::{
    BOTH, Machine, OPENCODE_JSON, OPENCODE_JSONC, apply_in, argv, bundle, fake, item, outcome_of, statuses, summaries,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];

fn registry() -> HarnessRegistry {
    HarnessRegistry::new(vec![Box::new(OpenCode::with_generation(OpenCodeGeneration::V2))])
}

/// An env value built at run time, so no source line looks like a credential.
fn secret() -> String {
    ["env", "-value-", "R4kP"].concat()
}

/// A machine whose global OpenCode 2 `opencode.jsonc` (with comments) has plugins under both keys
/// and servers in both shapes.
fn source_machine() -> Machine {
    let machine = Machine::new(OPENCODE);
    let path = machine.home().join(OPENCODE_JSONC);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            r#"{{
  // shared from here
  "plugin": ["opencode-wakatime"],
  "plugins": [{{ "package": "@acme/opencode-plugin@1.2.3" }}],
  "mcp": {{
    "docs": {{ "type": "remote", "url": "https://docs.example.com/mcp" }},
    "servers": {{
      "github": {{ "type": "local", "command": ["npx", "-y", "server-github"],
                  "environment": {{ "GITHUB_TOKEN": "{}" }} }},
    }},
  }},
}}"#,
            secret()
        ),
    )
    .unwrap();
    machine
}

/// The share file of everything an OpenCode 2 `machine` can export.
fn exported(machine: &Machine) -> Value {
    let collection = collect_export(&machine.env, &registry(), &summaries(OPENCODE));
    let all: Vec<String> = export_preview(&collection)
        .sections
        .iter()
        .flat_map(|section| section.items.iter().map(|item| item.id.clone()))
        .collect();
    let bundle = build_bundle(&collection, &all, None, None).unwrap();

    serde_json::from_str(&to_json_pretty(&bundle).unwrap()).unwrap()
}

/// The summary of an OpenCode server left unwritten because the merged names are unknown.
const UNCHECKED: &str =
    "Could not check the MCP servers OpenCode merges from all its config layers. Nothing was written.";

#[tokio::test]
async fn a_round_trip_adds_plugins_with_plugin_add_and_writes_nested_servers() {
    let shared = exported(&source_machine());
    assert!(!shared.to_string().contains(&secret()));

    let machine = Machine::new(OPENCODE);
    let runner = fake(&machine);
    let built = machine.plan_in(&registry(), &shared);

    assert_eq!(
        statuses(&built.plan),
        [
            ("opencode:plugin:@acme/opencode-plugin".to_owned(), SyncItemStatus::New),
            ("opencode:plugin:opencode-wakatime".to_owned(), SyncItemStatus::New),
            ("opencode:mcp:docs".to_owned(), SyncItemStatus::New),
            ("opencode:mcp:github".to_owned(), SyncItemStatus::New),
        ]
    );
    assert_eq!(
        item(&built.plan, "opencode:plugin:@acme/opencode-plugin").steps,
        [SyncStep::Cli {
            command: argv(&["opencode", "plugin", "add", "@acme/opencode-plugin@1.2.3"])
        }]
    );

    let github = item(&built.plan, "opencode:mcp:github");
    assert!(github.notes.contains(&SyncNote::OpenCodeMergedCheck));
    assert!(matches!(&github.steps[..], [SyncStep::McpMerge { file, .. }]
        if file == "~/.config/opencode/opencode.json"));

    let plugins = [
        "opencode:plugin:@acme/opencode-plugin",
        "opencode:plugin:opencode-wakatime",
    ]
    .map(str::to_owned);
    let (result, _) = apply_in(&machine, &runner, &registry(), &built.stored, &plugins, true).await;
    let result = result.expect("applied");

    assert!(result.ok, "{result:?}");
    assert_eq!(
        runner.seen(),
        [
            argv(&["opencode", "plugin", "add", "@acme/opencode-plugin@1.2.3"]),
            argv(&["opencode", "plugin", "add", "opencode-wakatime"]),
        ]
    );
    assert_eq!(
        machine.json(OPENCODE_JSON)["plugins"],
        json!(["@acme/opencode-plugin@1.2.3", "opencode-wakatime"])
    );

    let servers = ["opencode:mcp:docs", "opencode:mcp:github"].map(str::to_owned);
    let (result, _) = apply_in(&machine, &runner, &registry(), &built.stored, &servers, true).await;
    let result = result.expect("applied");

    assert!(result.ok, "{result:?}");
    assert_eq!(
        machine.json(OPENCODE_JSON),
        json!({
            "plugins": ["@acme/opencode-plugin@1.2.3", "opencode-wakatime"],
            "mcp": { "servers": {
                "docs": { "type": "remote", "url": "https://docs.example.com/mcp" },
                "github": { "type": "local", "command": ["npx", "-y", "server-github"], "disabled": true },
            } },
        }),
        "no `enabled`, switched off while it needs its env values"
    );

    // The check: `opencode api --standalone GET /api/config` once, in the home directory,
    // without self-update, capped, and read through a file.
    let probes = runner.probes();
    let [probe] = probes.as_slice() else {
        panic!("one check, got {probes:?}");
    };
    assert_eq!(probe.args, PROBE_ARGS_V2.map(OsString::from));
    assert_eq!(probe.cwd.as_deref(), Some(machine.home()));
    assert!(
        probe
            .env
            .iter()
            .any(|(name, value)| name == "OPENCODE_DISABLE_AUTOUPDATE" && value == "1")
    );
    assert_eq!(probe.timeout, PROBE_TIMEOUT);
    assert_eq!(probe.max_output_bytes, PROBE_MAX_BYTES);
    assert!(probe.stdout_through_file);

    // Everything is found the second time.
    let again = machine.plan_in(&registry(), &shared);
    assert!(
        statuses(&again.plan)
            .iter()
            .all(|(_, status)| *status == SyncItemStatus::AlreadyPresent),
        "{:?}",
        statuses(&again.plan)
    );
}

#[tokio::test]
async fn a_name_opencode_2_merges_from_another_layer_is_a_conflict() {
    let machine = Machine::new(OPENCODE);
    machine.opencode_merges(&["github"]);
    machine.write(
        OPENCODE_JSON,
        &json!({ "mcp": { "kept": { "type": "remote", "url": "https://kept.example.com" } } }),
    );
    let built = machine.plan_in(
        &registry(),
        &bundle(json!({ "opencode": { "mcpServers": [
            { "name": "github", "transport": "http", "url": "https://attacker.example.com/mcp" },
            { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
        ] } })),
    );

    let runner = fake(&machine);
    let servers = ["opencode:mcp:github", "opencode:mcp:docs"].map(str::to_owned);
    let (result, _) = apply_in(&machine, &runner, &registry(), &built.stored, &servers, true).await;
    let result = result.expect("applied");

    assert_eq!(outcome_of(&result, &servers[0]).outcome, SyncOutcome::Conflict);
    assert_eq!(outcome_of(&result, &servers[1]).outcome, SyncOutcome::Done);
    assert_eq!(
        machine.json(OPENCODE_JSON),
        json!({ "mcp": {
            "kept": { "type": "remote", "url": "https://kept.example.com" },
            "servers": { "docs": { "type": "remote", "url": "https://docs.example.com/mcp" } },
        } }),
        "github is never written"
    );
    assert!(!serde_json::to_string(&result).unwrap().contains("org.example.com"));
}

#[tokio::test]
async fn when_the_check_fails_no_server_is_written() {
    let machine = Machine::new(OPENCODE);
    machine.opencode_prints(ExecOutput {
        status: Some(1),
        stderr: "HTTP 500 Internal Server Error".to_owned(),
        ..ExecOutput::default()
    });
    let built = machine.plan_in(
        &registry(),
        &bundle(json!({ "opencode": { "mcpServers": [
            { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
        ] } })),
    );

    let runner = fake(&machine);
    let docs = ["opencode:mcp:docs".to_owned()];
    let (result, _) = apply_in(&machine, &runner, &registry(), &built.stored, &docs, true).await;
    let outcome = outcome_of(result.as_ref().expect("applied"), &docs[0]);

    assert_eq!(outcome.outcome, SyncOutcome::Failed);
    assert_eq!(outcome.summary.as_deref(), Some(UNCHECKED));
    assert_eq!(machine.text(OPENCODE_JSON), None);
}

#[test]
fn what_opencode_2_already_reads_is_present() {
    let machine = Machine::new(OPENCODE);
    machine.write(
        OPENCODE_JSON,
        &json!({
            "plugin": ["opencode-wakatime"],
            "plugins": [{ "package": "@acme/opencode-plugin@1.2.3" }, "gone@1.0.0", "-gone"],
            "mcp": {
                "docs": { "type": "remote", "url": "https://docs.example.com/mcp" },
                "servers": {
                    "github": { "type": "local", "command": ["npx", "-y", "server-github"] },
                    "other": { "type": "remote", "url": "https://mine.example.com" },
                },
            },
        }),
    );
    machine.write(
        ".config/opencode/config.json",
        &json!({ "plugin": ["only-in-config-json"], "mcp": { "legacy-only": { "type": "remote", "url": "https://x.example.com" } } }),
    );

    let built = machine.plan_in(
        &registry(),
        &bundle(json!({ "opencode": {
            "plugins": [
                { "id": "@acme/opencode-plugin", "version": "1.2.3" },
                { "id": "opencode-wakatime" },
                { "id": "gone" },
                { "id": "only-in-config-json" },
            ],
            "mcpServers": [
                { "name": "github", "transport": "stdio", "command": "npx", "args": ["-y", "server-github"] },
                { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
                { "name": "other", "transport": "http", "url": "https://other.example.com/mcp" },
                { "name": "legacy-only", "transport": "http", "url": "https://x.example.com" },
            ],
        }})),
    );

    assert_eq!(
        statuses(&built.plan),
        [
            (
                "opencode:plugin:@acme/opencode-plugin".to_owned(),
                SyncItemStatus::AlreadyPresent
            ),
            (
                "opencode:plugin:opencode-wakatime".to_owned(),
                SyncItemStatus::AlreadyPresent
            ),
            ("opencode:plugin:gone".to_owned(), SyncItemStatus::New),
            ("opencode:plugin:only-in-config-json".to_owned(), SyncItemStatus::New),
            ("opencode:mcp:github".to_owned(), SyncItemStatus::AlreadyPresent),
            ("opencode:mcp:docs".to_owned(), SyncItemStatus::AlreadyPresent),
            ("opencode:mcp:other".to_owned(), SyncItemStatus::Conflict),
            ("opencode:mcp:legacy-only".to_owned(), SyncItemStatus::New),
        ]
    );

    // Not installed: every item is blocked.
    let elsewhere = Machine::new(BOTH).plan_in(&registry(), &exported(&source_machine()));
    assert!(statuses(&elsewhere.plan).iter().all(|(_, status)| *status
        == SyncItemStatus::Blocked {
            reason: SyncBlockReason::HarnessNotInstalled
        }));
}
