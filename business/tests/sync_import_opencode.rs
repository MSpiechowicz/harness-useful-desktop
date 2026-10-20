//! Import of "Share setup" for OpenCode: a round trip from one machine's global config to a clean
//! one (`opencode plugin <spec> -g`, `mcp` entries in `opencode.json`), what is already there, the
//! manual edit of a commented config, and OpenCode's own validation of received items.

#[path = "sync_support/import.rs"]
mod import_support;

use harness_business::harness::HarnessRegistry;
use harness_business::model::{
    HarnessId, McpTransport, SyncBlockReason, SyncInvalidReason, SyncItemStatus, SyncMcpPreview, SyncNote, SyncOutcome,
    SyncSkipReason, SyncStep,
};
use harness_business::sync::codec::to_json_pretty;
use harness_business::sync::{build_bundle, collect_export, export_preview};
use import_support::{
    BOTH, Machine, OPENCODE_JSON, OPENCODE_JSONC, apply, argv, bundle, fake, item, outcome_of, selectable, statuses,
    summaries,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];

/// An env value built at run time, so no source line looks like a credential.
fn secret() -> String {
    ["env", "-value-", "Q7wZ"].concat()
}

/// A machine whose global `opencode.jsonc` (with comments) has two plugins and two servers.
fn source_machine() -> Machine {
    let machine = Machine::new(OPENCODE);
    let path = machine.home().join(OPENCODE_JSONC);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            r#"{{
  // shared from here
  "plugin": ["@acme/opencode-plugin@1.2.3", "opencode-wakatime"],
  "mcp": {{
    "github": {{ "type": "local", "command": ["npx", "-y", "server-github"],
                "environment": {{ "GITHUB_TOKEN": "{}" }}, "enabled": true }},
    "docs": {{ "type": "remote", "url": "https://docs.example.com/mcp", "enabled": false }},
  }},
}}"#,
            secret()
        ),
    )
    .unwrap();
    machine
}

/// The share file of everything `machine` can export.
fn exported(machine: &Machine) -> Value {
    let collection = collect_export(&machine.env, &HarnessRegistry::with_defaults(), &summaries(OPENCODE));
    let all: Vec<String> = export_preview(&collection)
        .sections
        .iter()
        .flat_map(|section| section.items.iter().map(|item| item.id.clone()))
        .collect();
    let bundle = build_bundle(&collection, &all, None, None).unwrap();

    serde_json::from_str(&to_json_pretty(&bundle).unwrap()).unwrap()
}

#[tokio::test]
async fn a_round_trip_installs_pinned_plugins_and_writes_opencode_entries_into_a_new_config() {
    let shared = exported(&source_machine());
    assert!(!shared.to_string().contains(&secret()));

    let machine = Machine::new(OPENCODE);
    let runner = fake(&machine);
    let built = machine.plan(&shared);

    assert_eq!(
        statuses(&built.plan),
        [
            ("opencode:plugin:@acme/opencode-plugin".to_owned(), SyncItemStatus::New),
            ("opencode:plugin:opencode-wakatime".to_owned(), SyncItemStatus::New),
            ("opencode:mcp:docs".to_owned(), SyncItemStatus::New),
            ("opencode:mcp:github".to_owned(), SyncItemStatus::New),
        ]
    );

    let pinned = item(&built.plan, "opencode:plugin:@acme/opencode-plugin");
    assert_eq!(pinned.version.as_deref(), Some("1.2.3"));
    assert!(pinned.notes.is_empty(), "the version is installed as is");
    assert_eq!(
        pinned.steps,
        [SyncStep::Cli {
            command: argv(&["opencode", "plugin", "@acme/opencode-plugin@1.2.3", "-g"])
        }]
    );

    let github = item(&built.plan, "opencode:mcp:github");
    assert_eq!(github.notes, [SyncNote::EnvValuesNotShared, SyncNote::WrittenDisabled]);
    assert_eq!(
        github.steps,
        [SyncStep::McpMerge {
            file: "~/.config/opencode/opencode.json".to_owned(),
            server: SyncMcpPreview {
                name: "github".to_owned(),
                transport: McpTransport::Stdio,
                command: Some("npx".to_owned()),
                args: argv(&["-y", "server-github"]),
                url: None,
                enabled: false,
            },
        }]
    );

    // The servers first: the missing config file is created with OpenCode's `$schema`.
    let servers = ["opencode:mcp:docs", "opencode:mcp:github"].map(str::to_owned);
    let (result, _) = apply(&machine, &runner, &built.stored, &servers, true).await;

    assert!(result.expect("applied").ok);
    assert!(runner.seen().is_empty());
    assert_eq!(
        machine.json(OPENCODE_JSON),
        json!({
            "$schema": "https://opencode.ai/config.json",
            "mcp": {
                "docs": { "type": "remote", "url": "https://docs.example.com/mcp", "enabled": true },
                "github": { "type": "local", "command": ["npx", "-y", "server-github"], "enabled": false },
            },
        })
    );

    let plugins = [
        "opencode:plugin:@acme/opencode-plugin",
        "opencode:plugin:opencode-wakatime",
    ]
    .map(str::to_owned);
    let (result, _) = apply(&machine, &runner, &built.stored, &plugins, true).await;
    let result = result.expect("applied");

    assert!(result.ok, "{result:?}");
    assert!(result.items.iter().all(|item| item.outcome == SyncOutcome::Done));
    assert_eq!(
        runner.seen(),
        [
            argv(&["opencode", "plugin", "@acme/opencode-plugin@1.2.3", "-g"]),
            argv(&["opencode", "plugin", "opencode-wakatime", "-g"]),
        ]
    );
    assert_eq!(
        outcome_of(&result, &plugins[0]).argv,
        argv(&["opencode", "plugin", "@acme/opencode-plugin@1.2.3", "-g"])
    );

    // Everything is found the second time, and the first plan applied again runs nothing.
    let again = machine.plan(&shared);
    assert!(
        statuses(&again.plan)
            .iter()
            .all(|(_, status)| *status == SyncItemStatus::AlreadyPresent),
        "{:?}",
        statuses(&again.plan)
    );

    let before = machine.text(OPENCODE_JSON);
    let (repeated, _) = apply(&machine, &runner, &built.stored, &selectable(&built.plan), true).await;

    assert!(
        repeated
            .expect("applied again")
            .items
            .iter()
            .all(|item| item.outcome == SyncOutcome::AlreadyPresent)
    );
    assert_eq!(runner.seen().len(), 2, "nothing ran the second time");
    assert_eq!(machine.text(OPENCODE_JSON), before);
}

fn shared_bundle() -> Value {
    bundle(json!({ "opencode": {
        "plugins": [{ "id": "@acme/opencode-plugin", "version": "1.2.3" }, { "id": "opencode-wakatime" }],
        "mcpServers": [
            { "name": "github", "transport": "stdio", "command": "npx", "args": ["-y", "server-github"],
              "envKeys": ["GITHUB_TOKEN"] },
            { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
            { "name": "other", "transport": "http", "url": "https://other.example.com/mcp" },
        ],
    }}))
}

#[test]
fn what_any_user_layer_already_has_is_present_and_never_shown() {
    let machine = Machine::new(OPENCODE);
    let config = format!(
        "{{\n  // mine\n  \"mcp\": {{\n    \"github\": {{ \"type\": \"local\", \"command\": [\"npx\", \"-y\", \"server-github\"], \
         \"environment\": {{ \"GITHUB_TOKEN\": \"{}\" }} }},\n    \"other\": {{ \"type\": \"remote\", \"url\": \"https://mine.example.com\" }}\n  }}\n}}\n",
        secret()
    );
    let path = machine.home().join(OPENCODE_JSONC);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &config).unwrap();
    machine.write(
        ".opencode/opencode.json",
        &json!({ "plugin": ["@acme/opencode-plugin@1.2.3", "opencode-wakatime@0.9.0"],
                 "mcp": { "docs": { "type": "remote", "url": "https://docs.example.com/mcp" } } }),
    );

    let built = machine.plan(&shared_bundle());

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
            ("opencode:mcp:github".to_owned(), SyncItemStatus::AlreadyPresent),
            ("opencode:mcp:docs".to_owned(), SyncItemStatus::AlreadyPresent),
            ("opencode:mcp:other".to_owned(), SyncItemStatus::Conflict),
        ]
    );
    assert!(selectable(&built.plan).is_empty());

    let shown = serde_json::to_string(&built.plan).unwrap();
    for leaked in [secret().as_str(), "mine.example.com", "0.9.0"] {
        assert!(!shown.contains(leaked), "the plan leaks {leaked:?}");
    }
    assert_eq!(machine.text(OPENCODE_JSONC).as_deref(), Some(config.as_str()));
}

#[tokio::test]
async fn a_commented_config_is_never_rewritten_and_each_server_gets_its_entry_to_paste() {
    let machine = Machine::new(OPENCODE);
    let config = "{\n  // keep me\n  \"theme\": \"opencode\"\n}\n";
    let path = machine.home().join(OPENCODE_JSONC);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, config).unwrap();

    let runner = fake(&machine);
    let built = machine.plan(&shared_bundle());
    let servers = ["opencode:mcp:github", "opencode:mcp:docs"].map(str::to_owned);

    assert!(
        matches!(&item(&built.plan, &servers[1]).steps[..], [SyncStep::McpMerge { file, .. }]
            if file == "~/.config/opencode/opencode.jsonc")
    );

    let (result, _) = apply(&machine, &runner, &built.stored, &servers, true).await;
    let result = result.expect("applied");

    let pasted = |name: &str, entry: Value| SyncOutcome::Skipped {
        reason: SyncSkipReason::ManualEdit {
            file: "~/.config/opencode/opencode.jsonc".to_owned(),
            snippet: format!("\"{name}\": {}", serde_json::to_string_pretty(&entry).unwrap()),
        },
    };

    assert!(!result.ok);
    assert_eq!(
        outcome_of(&result, &servers[0]).outcome,
        pasted(
            "github",
            json!({ "type": "local", "command": ["npx", "-y", "server-github"], "enabled": false })
        )
    );
    assert_eq!(
        outcome_of(&result, &servers[1]).outcome,
        pasted(
            "docs",
            json!({ "type": "remote", "url": "https://docs.example.com/mcp", "enabled": true })
        )
    );
    assert_eq!(machine.text(OPENCODE_JSONC).as_deref(), Some(config));
    assert_eq!(machine.text(OPENCODE_JSON), None);
    assert!(runner.seen().is_empty());
}

#[test]
fn received_opencode_items_are_npm_names_without_substitutions() {
    let machine = Machine::new(OPENCODE);
    let built = machine.plan(&bundle(json!({ "opencode": {
        "marketplaces": [{ "name": "useful", "source": "acme/useful-skills" }],
        "plugins": [
            { "id": "@scope/name", "version": "2.0.0-beta.1" },
            { "id": "name@marketplace" },
            { "id": "helper", "marketplace": "useful" },
            { "id": "ranged", "version": "^1.0.0" },
            { "id": "--foo" },
        ],
        "packages": [{ "source": "npm:pkg" }],
        "mcpServers": [
            { "name": "env-url", "transport": "http", "url": "https://x.example.com/{env:HOME}" },
            { "name": "file-arg", "transport": "stdio", "command": "server", "args": ["{FILE:~/.ssh/id_rsa}"] },
            { "name": "sse", "transport": "sse", "url": "https://sse.example.com/events" },
        ],
    }})));

    let invalid = |reason| SyncItemStatus::Invalid { reason };
    let not_supported = SyncItemStatus::Blocked {
        reason: SyncBlockReason::NotSupported,
    };

    assert_eq!(
        statuses(&built.plan),
        [
            ("opencode:marketplace:useful".to_owned(), not_supported.clone()),
            ("opencode:plugin:@scope/name".to_owned(), SyncItemStatus::New),
            ("opencode:plugin:#2".to_owned(), invalid(SyncInvalidReason::BadName)),
            ("opencode:plugin:#3".to_owned(), invalid(SyncInvalidReason::Malformed)),
            ("opencode:plugin:#4".to_owned(), invalid(SyncInvalidReason::Malformed)),
            ("opencode:plugin:#5".to_owned(), invalid(SyncInvalidReason::UnsafeValue)),
            ("opencode:package:npm:pkg".to_owned(), not_supported),
            ("opencode:mcp:#1".to_owned(), invalid(SyncInvalidReason::UnsafeValue)),
            ("opencode:mcp:#2".to_owned(), invalid(SyncInvalidReason::UnsafeValue)),
            ("opencode:mcp:sse".to_owned(), SyncItemStatus::New),
        ]
    );

    // An SSE server is written as OpenCode's `remote`.
    assert!(matches!(
        &item(&built.plan, "opencode:mcp:sse").steps[..],
        [SyncStep::McpMerge { server, .. }] if server.transport == McpTransport::Sse && server.enabled
    ));

    // Not installed: every item is blocked and nothing is read.
    let elsewhere = Machine::new(BOTH).plan(&shared_bundle());
    assert!(statuses(&elsewhere.plan).iter().all(|(_, status)| *status
        == SyncItemStatus::Blocked {
            reason: SyncBlockReason::HarnessNotInstalled
        }));
}
