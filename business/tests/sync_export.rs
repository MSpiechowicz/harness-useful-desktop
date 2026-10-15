//! Export of "Share setup": what a machine full of secrets shares (references only), what it leaves
//! out (named, with a typed reason, never a value), stable ids, and bundles that always import again.

mod sync_support;

use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use harness_business::BusinessError;
use harness_business::model::{HarnessId, SyncExcluded, SyncExcludedReason, SyncExportPreview, SyncItemKind, SyncNote};
use harness_business::sync::codec::{decode_input, to_json_pretty};
use harness_business::sync::schema::{Bundle, McpServer, Transport};
use harness_business::sync::{ExportCollection, build_bundle, export_code, export_preview};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_support::{BOTH, Home, ids};

const GITHUB_TOKEN: &str = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
const BEARER: &str = "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2lnbmF0dXJlLXZhbHVl";
const ARG_TOKEN: &str = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
const URL_KEY: &str = "QWERTY123secretvalue987zz";
const GIT_TOKEN: &str = "ghs_16C7e42F292c6912E7710c838347Ae178B4a";
// Built from two pieces so that no source line looks like a real GitLab token to a secret scanner.
const MARKETPLACE_TOKEN: &str = concat!("glpa", "t-AbCdEf1234567890XyZw");
const AUTH_KEY: &str = "sk-ant-api03-AUTHJSONSECRETvalue0123456789";
const PASSWORD: &str = "hunter2pass";

/// Every secret literal of the fixture plus the machine paths it holds.
const NEVER_SHARED: &[&str] = &[
    GITHUB_TOKEN,
    "eyJhbGciOiJIUzI1NiJ9",
    ARG_TOKEN,
    URL_KEY,
    GIT_TOKEN,
    MARKETPLACE_TOKEN,
    AUTH_KEY,
    PASSWORD,
    "/home/someone",
    "file:",
    "link:",
    "./local-pkg",
];

/// OMP and PI configs full of secrets, local paths and shareable references.
fn full_home() -> Home {
    let home = Home::empty();
    home.write(".omp/marketplaces.json", &omp_marketplaces());
    home.write(".omp/plugins/installed_plugins.json", &omp_plugins());
    home.write(".omp/plugins/package.json", &omp_npm_plugins());
    home.write(
        ".omp/plugins/node_modules/npm-helper/package.json",
        &json!({ "name": "npm-helper", "version": "0.4.2" }),
    );
    home.write(".omp/agent/mcp.json", &omp_mcp());
    home.write(".pi/agent/settings.json", &pi_settings());
    home.write(".pi/agent/mcp.json", &pi_mcp());
    home.write(
        ".pi/agent/auth.json",
        &json!({ "anthropic": { "type": "api_key", "key": AUTH_KEY } }),
    );
    home
}

fn omp_marketplaces() -> Value {
    json!({ "version": 1, "marketplaces": [
        { "name": "useful", "sourceType": "github", "sourceUri": "acme/useful-skills" },
        { "name": "demo", "sourceType": "git", "sourceUri": "https://github.com/demo-org/demo-marketplace.git" },
        { "name": "private", "sourceType": "git",
          "sourceUri": format!("https://oauth2:{MARKETPLACE_TOKEN}@gitlab.com/acme/plugins.git") },
        { "name": "mine", "sourceType": "local", "sourceUri": "/home/someone/marketplace" },
    ]})
}

fn omp_plugins() -> Value {
    json!({ "version": 2, "plugins": {
        "harness-useful-skills@useful": [{ "scope": "user", "installPath": "/home/someone/x", "version": "0.2.36" }],
        "demo-plugin@demo": [{ "scope": "user", "version": "1.2.0", "enabled": false }],
        "team-only@demo": [{ "scope": "project", "projectPath": "/home/someone/work" }],
    }})
}

fn omp_npm_plugins() -> Value {
    json!({ "name": "omp-plugins", "private": true, "dependencies": {
        "npm-helper": "^0.4.0",
        "my-plugin": "file:/home/someone/dev/my-plugin",
        "linked-plugin": "link:../linked-plugin",
        "git-plugin": "github:acme/git-plugin",
    }})
}

fn omp_mcp() -> Value {
    json!({ "mcpServers": {
        "github": {
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-github"],
            "env": { "GITHUB_TOKEN": GITHUB_TOKEN },
            "cwd": "/home/someone/work",
            "timeout": 30,
        },
        "remote": { "type": "http", "url": "https://mcp.example.com/mcp", "headers": { "Authorization": BEARER } },
        "leaky-args": { "command": "my-server", "args": ["--token", ARG_TOKEN] },
        "leaky-url": { "type": "sse", "url": format!("https://mcp.example.com/sse?api_key={URL_KEY}") },
        "creds-url": { "type": "http", "url": format!("https://admin:{PASSWORD}@mcp.example.com/mcp") },
        "local-bin": { "command": "/home/someone/bin/server" },
        "shell": { "command": "bash", "args": ["-c", "echo hi"] },
        "socket": { "type": "websocket", "url": "wss://mcp.example.com" },
    }})
}

fn pi_settings() -> Value {
    json!({ "defaultModel": "x", "packages": [
        "npm:@demo/pi-review-kit@1.0.0",
        { "source": "git:github.com/demo/pi-prompts@v1", "skills": [] },
        format!("https://x-access-token:{GIT_TOKEN}@github.com/acme/private-pkg.git"),
        "./local-pkg",
        "weird-thing",
    ]})
}

fn pi_mcp() -> Value {
    json!({ "mcpServers": { "docs": { "url": "https://docs.example.com/mcp", "auth": { "token": AUTH_KEY } } } })
}

fn bundle_of_everything(collection: &ExportCollection) -> Bundle {
    let all = ids(&export_preview(collection));
    build_bundle(
        collection,
        &all,
        Some("0.15.0".to_owned()),
        Some("2026-10-15".to_owned()),
    )
    .unwrap()
}

/// The JSON inside a `hud1:` string.
fn unpack(code: &str) -> String {
    let compressed = URL_SAFE_NO_PAD.decode(code.strip_prefix("hud1:").unwrap()).unwrap();
    let mut json = String::new();
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .read_to_string(&mut json)
        .unwrap();
    json
}

fn assert_no_secrets(what: &str, text: &str) {
    for secret in NEVER_SHARED {
        assert!(!text.contains(secret), "{what} leaks {secret:?}");
    }
}

#[test]
fn shares_only_safe_references() {
    let home = full_home();
    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(
        ids(&preview),
        [
            "omp:marketplace:demo",
            "omp:marketplace:useful",
            "omp:plugin:demo-plugin@demo",
            "omp:plugin:harness-useful-skills@useful",
            "omp:plugin:npm-helper",
            "omp:mcp:github",
            "omp:mcp:remote",
            "pi:package:git:github.com/demo/pi-prompts",
            "pi:package:npm:@demo/pi-review-kit",
            "pi:mcp:docs",
        ]
    );

    let github = &preview.sections[0].items[5];
    assert_eq!(
        github.detail.as_deref(),
        Some("npx -y @modelcontextprotocol/server-github")
    );
    assert!(github.runs_program);
    assert_eq!(github.env_keys, ["GITHUB_TOKEN"]);
    assert_eq!(
        github.notes,
        [
            SyncNote::EnvValuesNotShared,
            SyncNote::CwdNotShared,
            SyncNote::OtherFieldsNotShared
        ]
    );

    let remote = &preview.sections[0].items[6];
    assert_eq!(remote.header_keys, ["Authorization"]);
    assert!(!remote.runs_program);

    let npm = &preview.sections[0].items[4];
    assert_eq!(npm.version.as_deref(), Some("0.4.2"));
    assert_eq!(npm.notes, [SyncNote::VersionInformational]);

    let docs = &preview.sections[1].items[2];
    assert_eq!(docs.notes, [SyncNote::AuthNotShared]);
    assert_eq!(preview.sections[1].items[1].label, "npm:@demo/pi-review-kit@1.0.0");
}

#[test]
fn excluded_items_are_named_with_a_reason_and_no_value() {
    let home = full_home();
    let preview = export_preview(&home.collect(BOTH));

    let excluded = |harness, kind, label: &str, reason| SyncExcluded {
        harness,
        kind,
        label: label.to_owned(),
        reason,
    };
    use HarnessId::{Omp, Pi};
    use SyncExcludedReason::*;
    use SyncItemKind::{Marketplace, McpServer, Package, Plugin};

    assert_eq!(
        preview.excluded,
        [
            excluded(Omp, Marketplace, "private", Credentials),
            excluded(Omp, Marketplace, "mine", LocalPath),
            excluded(Omp, Plugin, "team-only@demo", ProjectScope),
            excluded(Omp, Plugin, "my-plugin", LocalPath),
            excluded(Omp, Plugin, "linked-plugin", LocalPath),
            excluded(Omp, Plugin, "git-plugin", Unsupported),
            excluded(Omp, McpServer, "leaky-args", ContainsSecret),
            excluded(Omp, McpServer, "leaky-url", ContainsSecret),
            excluded(Omp, McpServer, "creds-url", Credentials),
            excluded(Omp, McpServer, "local-bin", LocalPath),
            excluded(Omp, McpServer, "shell", Invalid),
            excluded(Omp, McpServer, "socket", Unsupported),
            excluded(Pi, Package, "private-pkg", Credentials),
            excluded(Pi, Package, "local-pkg", LocalPath),
            excluded(Pi, Package, "weird-thing", Unsupported),
        ]
    );
}

#[test]
fn machine_paths_inside_mcp_args_or_urls_leave_the_server_out() {
    let home = Home::empty();
    home.write(
        ".omp/agent/mcp.json",
        &json!({ "mcpServers": {
            "root-arg": { "command": "server", "args": ["--root=/home/alice/proj"] },
            "tilde-arg": { "command": "server", "args": ["--config=~/x"] },
            "drive-arg": { "command": "server", "args": ["--dir=C:/Users/x"] },
            "path-url": { "type": "http", "url": "https://mcp.example.com/mcp?root=/home/alice" },
            "fine": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github", "--flag=value"] },
            "fine-url": { "type": "http", "url": "https://mcp.example.com/home/mcp" },
        }}),
    );

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(ids(&preview), ["omp:mcp:fine", "omp:mcp:fine-url"]);
    let excluded: Vec<(&str, SyncExcludedReason)> = preview
        .excluded
        .iter()
        .map(|entry| (entry.label.as_str(), entry.reason))
        .collect();
    assert_eq!(
        excluded,
        [
            ("root-arg", SyncExcludedReason::LocalPath),
            ("tilde-arg", SyncExcludedReason::LocalPath),
            ("drive-arg", SyncExcludedReason::LocalPath),
            ("path-url", SyncExcludedReason::LocalPath),
        ]
    );
}

#[test]
fn option_like_local_and_plain_http_package_specs_and_short_secret_flags_are_left_out() {
    let home = Home::empty();
    home.write(
        ".pi/agent/settings.json",
        &json!({ "packages": [
            "npm:--registry=https://evil.example.com",
            "npm:linked@file:../linked",
            "npm:abs@/opt/abs",
            "npm:tarball@http://registry.example.com/t.tgz",
            "npm:fine@1.0.0",
        ]}),
    );
    home.write(
        ".pi/agent/mcp.json",
        &json!({ "mcpServers": {
            "short-pass": { "command": "db-server", "args": ["-p", "hunter2pass"] },
            "dash-token": { "command": "db-server", "args": ["--token", "-starts-with-dash"] },
        }}),
    );

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(ids(&preview), ["pi:package:npm:fine"]);
    let excluded: Vec<(SyncItemKind, SyncExcludedReason)> = preview
        .excluded
        .iter()
        .map(|entry| (entry.kind, entry.reason))
        .collect();
    assert_eq!(
        excluded,
        [
            (SyncItemKind::Package, SyncExcludedReason::Invalid),
            (SyncItemKind::Package, SyncExcludedReason::LocalPath),
            (SyncItemKind::Package, SyncExcludedReason::LocalPath),
            (SyncItemKind::Package, SyncExcludedReason::Unsupported),
            (SyncItemKind::McpServer, SyncExcludedReason::ContainsSecret),
            (SyncItemKind::McpServer, SyncExcludedReason::ContainsSecret),
        ]
    );
}

#[test]
fn unreadable_mcp_files_and_package_filters_are_reported() {
    let home = Home::empty();
    home.write_text(".omp/agent/mcp.json", "{ not json");
    home.write(".pi/agent/mcp.json", &json!({ "mcpServers": ["not", "an", "object"] }));
    home.write(
        ".pi/agent/settings.json",
        &json!({ "packages": [
            "npm:plain",
            { "source": "npm:filtered", "skills": [], "extensions": ["only-this"] },
            { "source": "npm:object-only" },
        ]}),
    );

    let preview = export_preview(&home.collect(BOTH));

    let excluded = |harness, label: &str| SyncExcluded {
        harness,
        kind: SyncItemKind::McpServer,
        label: label.to_owned(),
        reason: SyncExcludedReason::Invalid,
    };
    assert_eq!(
        preview.excluded,
        [
            excluded(HarnessId::Omp, "~/.omp/agent/mcp.json"),
            excluded(HarnessId::Pi, "~/.pi/agent/mcp.json"),
        ]
    );

    let notes: Vec<(&str, &[SyncNote])> = preview.sections[0]
        .items
        .iter()
        .map(|item| (item.label.as_str(), item.notes.as_slice()))
        .collect();
    assert_eq!(
        notes,
        [
            ("npm:filtered", &[SyncNote::FiltersNotShared][..]),
            ("npm:object-only", &[][..]),
            ("npm:plain", &[][..]),
        ]
    );
}

#[test]
fn no_secret_reaches_the_code_the_file_or_the_preview() {
    let home = full_home();
    let collection = home.collect(BOTH);
    let bundle = bundle_of_everything(&collection);
    let exported = export_code(&bundle).unwrap();
    let pretty = to_json_pretty(&bundle).unwrap();

    assert_no_secrets("preview", &serde_json::to_string(&export_preview(&collection)).unwrap());
    assert_no_secrets("code", &exported.code);
    assert_no_secrets("code JSON", &unpack(&exported.code));
    assert_no_secrets("file", &pretty);
    assert_no_secrets(
        "decoded bundle",
        &format!("{:?}", decode_input(&exported.code).unwrap()),
    );

    assert_eq!(exported.item_count, 10);
    assert_eq!(exported.bytes as usize, exported.code.len());
}

#[test]
fn every_export_imports_again_without_invalid_items() {
    let home = full_home();
    let bundle = bundle_of_everything(&home.collect(BOTH));

    for input in [export_code(&bundle).unwrap().code, to_json_pretty(&bundle).unwrap()] {
        let decoded = decode_input(&input).unwrap();
        let items: usize = decoded
            .sections
            .iter()
            .map(|section| {
                section.marketplaces.len() + section.plugins.len() + section.packages.len() + section.mcp_servers.len()
            })
            .sum();

        assert_eq!(decoded.invalid_count(), 0);
        assert_eq!(items, bundle.item_count());
        assert!(decoded.unsupported.is_empty());
    }

    let github = &bundle.harnesses[&HarnessId::Omp].mcp_servers[0];
    assert_eq!(
        github,
        &McpServer {
            name: "github".to_owned(),
            transport: Transport::Stdio,
            command: Some("npx".to_owned()),
            args: vec!["-y".to_owned(), "@modelcontextprotocol/server-github".to_owned()],
            url: None,
            env_keys: vec!["GITHUB_TOKEN".to_owned()],
            header_keys: Vec::new(),
        }
    );
}

#[test]
fn ids_are_stable_and_key_order_is_irrelevant() {
    let home = full_home();
    let first = home.collect(BOTH);
    assert_eq!(first, home.collect(BOTH));

    let reversed = |value: Value| -> Value {
        fn reverse(value: Value) -> Value {
            match value {
                Value::Object(map) => Value::Object(map.into_iter().rev().map(|(k, v)| (k, reverse(v))).collect()),
                other => other,
            }
        }
        reverse(value)
    };
    home.write(".omp/agent/mcp.json", &reversed(omp_mcp()));
    home.write(".omp/plugins/installed_plugins.json", &reversed(omp_plugins()));
    home.write(".omp/plugins/package.json", &reversed(omp_npm_plugins()));
    home.write(".omp/marketplaces.json", &reversed(omp_marketplaces()));

    let second = home.collect(BOTH);
    assert_eq!(ids(&export_preview(&first)), ids(&export_preview(&second)));
    assert_eq!(bundle_of_everything(&first), bundle_of_everything(&second));
}

#[test]
fn harnesses_that_are_not_installed_are_skipped() {
    let home = full_home();
    let preview = export_preview(&home.collect(&[HarnessId::Pi]));

    assert!(preview.sections.iter().all(|section| section.harness == HarnessId::Pi));
    assert!(preview.excluded.iter().all(|entry| entry.harness == HarnessId::Pi));
    assert_eq!(preview.sections.len(), 1);

    assert_eq!(
        export_preview(&home.collect(&[])),
        SyncExportPreview {
            sections: Vec::new(),
            excluded: Vec::new()
        }
    );
}

#[test]
fn an_empty_machine_has_nothing_to_share() {
    let home = Home::empty();
    let collection = home.collect(BOTH);

    assert_eq!(
        export_preview(&collection),
        SyncExportPreview {
            sections: Vec::new(),
            excluded: Vec::new()
        }
    );
    assert!(matches!(
        build_bundle(&collection, &["omp:mcp:github".to_owned()], None, None),
        Err(BusinessError::InvalidInput(_))
    ));
}

#[test]
fn bundles_hold_only_selected_known_ids() {
    let home = full_home();
    let collection = home.collect(BOTH);
    let selected = [
        "pi:mcp:docs",
        "omp:plugin:npm-helper",
        "omp:mcp:leaky-args",
        "made:up:id",
        "pi:mcp:docs",
    ]
    .map(str::to_owned);

    let bundle = build_bundle(&collection, &selected, None, Some("2026-10-15".to_owned())).unwrap();

    assert_eq!(bundle.item_count(), 2);
    assert_eq!(bundle.harnesses[&HarnessId::Omp].plugins[0].id, "npm-helper");
    assert_eq!(bundle.harnesses[&HarnessId::Pi].mcp_servers[0].name, "docs");
    assert_eq!(bundle.created_at.as_deref(), Some("2026-10-15"));

    let unknown = build_bundle(&collection, &["omp:mcp:leaky-args".to_owned()], None, None);
    assert!(matches!(unknown, Err(BusinessError::InvalidInput(_))));
}
