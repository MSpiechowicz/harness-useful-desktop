//! Export limits of "Share setup": items past the bundle limits, share strings over 64 KiB, what the
//! import would refuse, and the app's own companion sources and MCP entry.

use crate::sync_support;

use harness_business::BusinessError;
use harness_business::model::{HarnessId, SyncExcludedReason};
use harness_business::sync::codec::{decode_input, to_json_pretty};
use harness_business::sync::schema::{Bundle, McpServer, Transport};
use harness_business::sync::{build_bundle, export_code, export_preview};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_support::{BOTH, Home, ids};

#[test]
fn more_items_than_a_bundle_holds_are_excluded_as_too_many() {
    let home = Home::empty();
    let servers: serde_json::Map<String, Value> = (0..201)
        .map(|index| {
            (
                format!("server-{index:03}"),
                json!({ "url": "https://mcp.example.com/mcp" }),
            )
        })
        .collect();
    home.write(".pi/agent/mcp.json", &json!({ "mcpServers": servers }));

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(preview.sections[0].items.len(), 200);
    assert_eq!(preview.excluded.len(), 1);
    assert_eq!(preview.excluded[0].label, "server-200");
    assert_eq!(preview.excluded[0].reason, SyncExcludedReason::TooMany);
}

#[test]
fn a_code_over_64_kib_points_to_the_file() {
    // Hex digits of a simple generator do not compress below 64 KiB.
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let args = (0..160)
        .map(|_| {
            (0..1000)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    char::from_digit((state % 16) as u32, 16).unwrap()
                })
                .collect()
        })
        .collect();
    let mut bundle = Bundle::new(None, None);
    bundle
        .harnesses
        .entry(HarnessId::Pi)
        .or_default()
        .mcp_servers
        .push(McpServer {
            name: "big".to_owned(),
            transport: Transport::Stdio,
            command: Some("big".to_owned()),
            args,
            url: None,
            env_keys: Vec::new(),
            header_keys: Vec::new(),
        });

    match export_code(&bundle) {
        Err(BusinessError::InvalidInput(message)) => assert!(message.contains("file"), "{message}"),
        other => panic!("expected invalidInput, got {other:?}"),
    }
}

/// What the import refuses never leaves: env references it would expand from the importer's
/// environment, package specs naming a path, an archive or a plain transport, and the dashboard's
/// own MCP entry (Recommended setup installs it, so it is skipped without a row).
#[test]
fn what_the_import_refuses_is_left_out_and_the_rest_imports_again() {
    let home = Home::empty();
    home.write(
        ".omp/agent/mcp.json",
        &json!({ "mcpServers": {
            "harness-dashboard": { "command": "harness-dashboard", "args": ["mcp"] },
            "query-ref": { "type": "http", "url": "https://c.example.com/mcp?k=${GITHUB_TOKEN}" },
            "home-arg": { "command": "my-server", "args": ["$HOME"] },
            "declared": { "command": "my-server", "args": ["--api-key", "${API_KEY}"], "env": { "API_KEY": "" } },
        }}),
    );
    home.write(
        ".pi/agent/settings.json",
        &json!({ "packages": [
            "npm:/data/pkg",
            "npm:pkg.tgz",
            "npm:git+http://example.com/owner/repo",
            "git:file:///srv/x",
            "https://-x/a/b",
            "npm:fine@1.0.0",
        ]}),
    );

    let collection = home.collect(BOTH);
    let preview = export_preview(&collection);

    assert_eq!(ids(&preview), ["omp:mcp:declared", "pi:package:npm:fine"]);
    let excluded: Vec<(&str, SyncExcludedReason)> = preview
        .excluded
        .iter()
        .map(|entry| (entry.label.as_str(), entry.reason))
        .collect();
    assert_eq!(
        excluded,
        [
            ("query-ref", SyncExcludedReason::Invalid),
            ("home-arg", SyncExcludedReason::Invalid),
            ("pkg", SyncExcludedReason::LocalPath),
            ("pkg.tgz", SyncExcludedReason::LocalPath),
            ("repo", SyncExcludedReason::Unsupported),
            ("x", SyncExcludedReason::LocalPath),
            ("b", SyncExcludedReason::Invalid),
        ]
    );

    let bundle = build_bundle(&collection, &ids(&preview), None, None).unwrap();
    let decoded = decode_input(&to_json_pretty(&bundle).unwrap()).unwrap();
    assert_eq!(decoded.invalid_count(), 0);
}

/// The app's own companions live under `MSpiechowicz/…`, which the redactor currently hides as a
/// random-looking token, so `sync::secrets::exportable` refuses them on both sides.
#[test]
fn the_companion_sources_are_shared() {
    let home = Home::empty();
    home.write(
        ".omp/marketplaces.json",
        &json!({ "version": 1, "marketplaces": [
            { "name": "useful", "sourceType": "github", "sourceUri": "MSpiechowicz/oh-my-pi-useful-skills" },
        ]}),
    );
    home.write(
        ".pi/agent/settings.json",
        &json!({ "packages": ["git:github.com/MSpiechowicz/oh-my-pi-useful-skills"] }),
    );

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(preview.excluded, []);
    assert_eq!(
        ids(&preview),
        [
            "omp:marketplace:useful",
            "pi:package:git:github.com/MSpiechowicz/oh-my-pi-useful-skills"
        ]
    );
}
