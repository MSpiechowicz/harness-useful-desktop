//! Export of "Share setup" for Claude Code: marketplaces from `known_marketplaces.json`, user
//! plugins from `installed_plugins.json` and the user scope's MCP servers from the global file,
//! shared as references only. Everything else is named with a reason and no value.

use crate::sync_support;

use std::path::PathBuf;

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{HarnessId, SyncExcluded, SyncExcludedReason, SyncExportPreview, SyncItemKind, SyncNote};
use harness_business::sync::codec::{decode_input, to_json_pretty};
use harness_business::sync::schema::{Bundle, Marketplace, McpServer, Plugin, Transport};
use harness_business::sync::{ExportCollection, build_bundle, collect_export, export_code, export_preview};
use pretty_assertions::assert_eq;
use serde_json::json;
use sync_support::{BOTH, Home, ids, summaries};

const CLAUDE: &[HarnessId] = &[HarnessId::Claude];

/// The value the fixture home hides in credentials, settings and server arguments.
const FIXTURE_CANARY: &str = "CANARY-7f3a9c";

fn fixture_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-home")
}

fn collect(env: &Env) -> ExportCollection {
    collect_export(env, &HarnessRegistry::with_defaults(), &summaries(CLAUDE))
}

fn everything(collection: &ExportCollection) -> Bundle {
    build_bundle(collection, &ids(&export_preview(collection)), None, None).unwrap()
}

fn excluded(kind: SyncItemKind, label: &str, reason: SyncExcludedReason) -> SyncExcluded {
    SyncExcluded {
        harness: HarnessId::Claude,
        kind,
        label: label.to_owned(),
        reason,
    }
}

fn plugin(id: &str, marketplace: &str, version: &str) -> Plugin {
    Plugin {
        id: id.to_owned(),
        marketplace: Some(marketplace.to_owned()),
        version: Some(version.to_owned()),
    }
}

fn excluded_of(preview: &SyncExportPreview, kind: SyncItemKind) -> Vec<SyncExcluded> {
    preview
        .excluded
        .iter()
        .filter(|item| item.kind == kind)
        .cloned()
        .collect()
}

#[test]
fn the_fixture_home_shares_marketplaces_user_plugins_and_servers_by_reference() {
    let env = Env::for_tests(fixture_home());
    let collection = collect(&env);
    let preview = export_preview(&collection);

    assert_eq!(
        ids(&preview),
        [
            "claude:marketplace:demo-market",
            "claude:marketplace:fresh",
            "claude:marketplace:useful-skills-local",
            "claude:plugin:escaping@demo-market",
            "claude:plugin:lint-tools@demo-market",
            "claude:plugin:useful-skills@useful-skills-local",
            "claude:mcp:linear",
        ]
    );

    let items = &preview.sections[0].items;
    let lint = items
        .iter()
        .find(|item| item.id == "claude:plugin:lint-tools@demo-market")
        .unwrap();
    assert_eq!(lint.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        lint.notes,
        [SyncNote::VersionInformational],
        "switched off, still shared"
    );

    let linear = items.iter().find(|item| item.id == "claude:mcp:linear").unwrap();
    assert_eq!(linear.header_keys, ["Authorization"]);
    assert!(!linear.runs_program);
    assert_eq!(linear.notes, [SyncNote::HeaderValuesNotShared]);

    let section = &everything(&collection).harnesses[&HarnessId::Claude];
    assert_eq!(
        section.marketplaces,
        [
            Marketplace {
                name: "demo-market".to_owned(),
                source: "https://git.example.com/demo/market.git".to_owned(),
            },
            Marketplace {
                name: "fresh".to_owned(),
                source: "demo-org/fresh-plugins".to_owned(),
            },
            Marketplace {
                name: "useful-skills-local".to_owned(),
                source: "demo-org/useful-skills".to_owned(),
            },
        ]
    );
    assert_eq!(
        section.plugins,
        [
            plugin("escaping@demo-market", "demo-market", "0.0.1"),
            plugin("lint-tools@demo-market", "demo-market", "1.0.0"),
            plugin("useful-skills@useful-skills-local", "useful-skills-local", "0.3.1"),
        ]
    );
    assert_eq!(
        section.mcp_servers,
        [McpServer {
            name: "linear".to_owned(),
            transport: Transport::Http,
            command: None,
            args: Vec::new(),
            url: Some("https://mcp.linear.app/mcp".to_owned()),
            env_keys: Vec::new(),
            header_keys: vec!["Authorization".to_owned()],
        }]
    );
}

#[test]
fn the_fixture_home_names_what_it_leaves_out_and_never_a_value() {
    let env = Env::for_tests(fixture_home());
    let collection = collect(&env);
    let preview = export_preview(&collection);

    assert_eq!(
        preview.excluded,
        [
            excluded(SyncItemKind::Marketplace, "local-market", SyncExcludedReason::LocalPath),
            excluded(SyncItemKind::Marketplace, "escape", SyncExcludedReason::Credentials),
            excluded(
                SyncItemKind::Plugin,
                "release-notes@demo-market",
                SyncExcludedReason::ProjectScope
            ),
            excluded(SyncItemKind::McpServer, "github", SyncExcludedReason::ContainsSecret),
        ]
    );

    let bundle = everything(&collection);
    let exported = export_code(&bundle).unwrap();
    for (what, text) in [
        ("preview", format!("{preview:?}")),
        ("file", to_json_pretty(&bundle).unwrap()),
        ("code", format!("{:?}", decode_input(&exported.code).unwrap())),
    ] {
        assert!(!text.contains(FIXTURE_CANARY), "the {what} leaks the canary");
        assert!(
            !text.contains("git.example.com/team"),
            "the {what} names a left-out source"
        );
    }
}

#[test]
fn the_global_file_follows_claude_config_dir() {
    let env = Env::for_tests(fixture_home()).with_var(
        "CLAUDE_CONFIG_DIR",
        fixture_home().join("claude-work").to_string_lossy().into_owned(),
    );
    let preview = export_preview(&collect(&env));

    assert_eq!(ids(&preview), ["claude:mcp:work-docs"], "no registries in that folder");
}

/// A home with Claude's edge cases: pseudo marketplaces, pinned and unsupported sources, plugins
/// of left-out marketplaces, `${…}` substitutions and the app's own servers.
fn edge_home() -> Home {
    let home = Home::empty();
    let token = ["arg", "-value-", "Q7wZ"].concat();

    home.write(
        ".claude/plugins/known_marketplaces.json",
        &json!({
            "tools": { "source": { "source": "github", "repo": "acme/claude-tools" } },
            "catalog": { "source": { "source": "url", "url": "https://plugins.example.com/marketplace.json" } },
            "pinned": { "source": { "source": "github", "repo": "acme/pinned", "ref": "v1" } },
            "nested": { "source": { "source": "git", "url": "https://git.example.com/mono.git", "path": "plugins" } },
            "over-ssh": { "source": { "source": "git", "url": "git@github.com:acme/ssh.git" } },
            "from-npm": { "source": { "source": "npm", "package": "@acme/market" } },
            "picked": { "source": { "source": "file", "path": "/home/someone/market.json" } },
            "odd": "not an object",
        }),
    );
    home.write(
        ".claude/plugins/installed_plugins.json",
        &json!({ "version": 2, "plugins": {
            "formatter@tools": [{ "scope": "user", "version": "2.0.0" }],
            "unversioned@tools": [{}],
            "odd-version@tools": [{ "scope": "user", "version": "main branch" }],
            "dev@inline": [{ "scope": "user" }],
            "notes@skills-dir": [{ "scope": "user" }],
            "account@synced": [{ "scope": "user" }],
            "org-only@tools": [{ "scope": "managed", "version": "1.0.0" }],
            "mine@tools": [{ "scope": "local", "projectPath": "/x" }, { "scope": "user", "version": "3.0.0" }],
            "nested-plugin@nested": [{ "scope": "user" }],
            "picked-plugin@picked": [{ "scope": "user" }],
            "bare": [{ "scope": "user" }],
        }}),
    );
    home.write(
        ".claude.json",
        &json!({
            "oauthAccount": { "emailAddress": "someone@example.com" },
            "mcpServers": {
                "docs": { "type": "stdio", "command": "docs-server", "args": ["--stdio"], "env": { "DOCS_TOKEN": token } },
                "untyped": { "command": "uvx", "args": ["mcp-server-time"] },
                "helper": { "type": "http", "url": "https://mcp.example.com/mcp", "headersHelper": "print-headers" },
                "home-arg": { "command": "server", "args": ["${HOME}/notes"] },
                "declared-arg": { "command": "server", "args": ["--token-file", "${DOCS_TOKEN}"], "env": { "DOCS_TOKEN": "x" } },
                "env-url": { "type": "http", "url": "https://${HOST}/mcp" },
                "env-command": { "command": "${SERVER_BIN}" },
                "socket": { "type": "ws", "url": "wss://ws.example.com" },
                "harness-memory": { "type": "stdio", "command": "harness-memory", "args": ["mcp"] },
                "harness-dashboard": { "type": "stdio", "command": "harness-dashboard", "args": ["mcp"] },
            },
            "projects": { "/x": { "mcpServers": { "project-only": { "command": "x" } } } },
        }),
    );

    home
}

#[test]
fn only_sources_another_machine_adds_the_same_way_are_shared() {
    let preview = export_preview(&edge_home().collect(CLAUDE));
    let shared: Vec<String> = ids(&preview)
        .into_iter()
        .filter(|id| id.starts_with("claude:marketplace:"))
        .collect();

    assert_eq!(shared, ["claude:marketplace:catalog", "claude:marketplace:tools"]);
    assert_eq!(
        excluded_of(&preview, SyncItemKind::Marketplace),
        [
            excluded(SyncItemKind::Marketplace, "pinned", SyncExcludedReason::Unsupported),
            excluded(SyncItemKind::Marketplace, "nested", SyncExcludedReason::Unsupported),
            excluded(SyncItemKind::Marketplace, "over-ssh", SyncExcludedReason::Unsupported),
            excluded(SyncItemKind::Marketplace, "from-npm", SyncExcludedReason::Unsupported),
            excluded(SyncItemKind::Marketplace, "picked", SyncExcludedReason::LocalPath),
        ]
    );
}

#[test]
fn only_user_plugins_of_shared_marketplaces_are_shared() {
    let collection = edge_home().collect(CLAUDE);
    let preview = export_preview(&collection);
    let shared: Vec<String> = ids(&preview)
        .into_iter()
        .filter(|id| id.starts_with("claude:plugin:"))
        .collect();

    assert_eq!(
        shared,
        [
            "claude:plugin:formatter@tools",
            "claude:plugin:mine@tools",
            "claude:plugin:odd-version@tools",
            "claude:plugin:unversioned@tools",
        ]
    );
    assert_eq!(
        excluded_of(&preview, SyncItemKind::Plugin),
        [
            excluded(SyncItemKind::Plugin, "dev@inline", SyncExcludedReason::LocalPath),
            excluded(SyncItemKind::Plugin, "notes@skills-dir", SyncExcludedReason::LocalPath),
            excluded(SyncItemKind::Plugin, "account@synced", SyncExcludedReason::Unsupported),
            excluded(SyncItemKind::Plugin, "org-only@tools", SyncExcludedReason::Unsupported),
            excluded(
                SyncItemKind::Plugin,
                "nested-plugin@nested",
                SyncExcludedReason::Unsupported
            ),
            excluded(
                SyncItemKind::Plugin,
                "picked-plugin@picked",
                SyncExcludedReason::LocalPath
            ),
            excluded(SyncItemKind::Plugin, "bare", SyncExcludedReason::Invalid),
        ]
    );

    let plugins = &everything(&collection).harnesses[&HarnessId::Claude].plugins;
    let versions: Vec<Option<&str>> = plugins.iter().map(|plugin| plugin.version.as_deref()).collect();
    assert_eq!(
        versions,
        [Some("2.0.0"), Some("3.0.0"), None, None],
        "the user install's version"
    );
}

#[test]
fn servers_with_substitutions_are_left_out_and_the_apps_own_are_skipped() {
    let collection = edge_home().collect(CLAUDE);
    let preview = export_preview(&collection);
    let shared: Vec<String> = ids(&preview)
        .into_iter()
        .filter(|id| id.starts_with("claude:mcp:"))
        .collect();

    assert_eq!(shared, ["claude:mcp:docs", "claude:mcp:helper", "claude:mcp:untyped"]);
    assert_eq!(
        excluded_of(&preview, SyncItemKind::McpServer),
        [
            excluded(SyncItemKind::McpServer, "home-arg", SyncExcludedReason::Invalid),
            excluded(SyncItemKind::McpServer, "declared-arg", SyncExcludedReason::Invalid),
            excluded(SyncItemKind::McpServer, "env-url", SyncExcludedReason::Invalid),
            excluded(SyncItemKind::McpServer, "env-command", SyncExcludedReason::Invalid),
            excluded(SyncItemKind::McpServer, "socket", SyncExcludedReason::Unsupported),
        ]
    );

    let items = &preview.sections[0].items;
    let helper = items.iter().find(|item| item.id == "claude:mcp:helper").unwrap();
    assert_eq!(
        helper.notes,
        [SyncNote::OtherFieldsNotShared],
        "headersHelper stays here"
    );

    let docs = items.iter().find(|item| item.id == "claude:mcp:docs").unwrap();
    assert_eq!(docs.env_keys, ["DOCS_TOKEN"]);
    assert!(docs.runs_program);

    let text = to_json_pretty(&everything(&collection)).unwrap();
    assert!(!text.contains(&["arg", "-value-", "Q7wZ"].concat()));
    assert!(!text.contains("someone@example.com"));
    assert!(!text.contains("project-only"));
}

#[test]
fn every_claude_export_imports_again_without_invalid_items() {
    for collection in [collect(&Env::for_tests(fixture_home())), edge_home().collect(CLAUDE)] {
        let bundle = everything(&collection);

        for input in [export_code(&bundle).unwrap().code, to_json_pretty(&bundle).unwrap()] {
            let decoded = decode_input(&input).unwrap();

            assert_eq!(decoded.invalid_count(), 0);
            assert!(decoded.unsupported.is_empty());
        }
    }
}

#[test]
fn claude_code_that_is_not_installed_is_not_read() {
    let preview = export_preview(&edge_home().collect(BOTH));

    assert!(preview.sections.is_empty());
    assert!(preview.excluded.is_empty());
}
