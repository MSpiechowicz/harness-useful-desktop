//! Export of "Share setup" for Codex: the marketplaces, plugins and MCP servers of its
//! `config.toml`, shared as references only (key and header names, never a value). Everything else
//! is named with a reason and no value.

use crate::sync_support;

use harness_business::env::Env;
use harness_business::model::{HarnessId, SyncExcluded, SyncExcludedReason, SyncItemKind, SyncNote};
use harness_business::sync::schema::{Marketplace, McpServer, Plugin, Transport};
use harness_business::sync::{ExportCollection, export_preview};
use pretty_assertions::assert_eq;
use sync_support::{
    BOTH, Home, assert_imports_again, assert_nothing_shows, bundle_of_everything, excluded_of_kind, ids,
};

const CODEX: &[HarnessId] = &[HarnessId::Codex];

/// The value the fixture home hides in its secrets and private files.
const FIXTURE_CANARY: &str = "CANARY-7f3a9c";

/// The export of the fixture home (`tests/fixtures/codex-home`), with `CODEX_HOME` naming its
/// folder `home` when given.
fn fixture(codex_home: Option<&str>) -> ExportCollection {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-home");
    let mut env = Env::for_tests(&root);

    if let Some(folder) = codex_home {
        env = env.with_var("CODEX_HOME", root.join(folder).to_string_lossy());
    }

    harness_business::sync::collect_export(
        &env,
        &harness_business::harness::HarnessRegistry::with_defaults(),
        &sync_support::summaries(CODEX),
    )
}

fn left_out(kind: SyncItemKind, label: &str, reason: SyncExcludedReason) -> SyncExcluded {
    SyncExcluded {
        harness: HarnessId::Codex,
        kind,
        label: label.to_owned(),
        reason,
    }
}

#[test]
fn the_fixture_home_shares_its_git_marketplace_plugins_and_servers_by_reference() {
    let collection = fixture(None);
    let preview = export_preview(&collection);

    assert_eq!(
        ids(&preview),
        [
            "codex:marketplace:demo-market",
            "codex:plugin:ghost@demo-market",
            "codex:plugin:lint-tools@demo-market",
            "codex:mcp:docs",
        ]
    );

    let docs = preview.sections[0]
        .items
        .iter()
        .find(|item| item.id == "codex:mcp:docs")
        .unwrap();
    assert_eq!(docs.header_keys, ["Authorization"]);
    assert!(!docs.runs_program);
    assert_eq!(docs.notes, [SyncNote::HeaderValuesNotShared]);

    let section = &bundle_of_everything(&collection).harnesses[&HarnessId::Codex];
    let demo = |id: &str, version: Option<&str>| Plugin {
        id: id.to_owned(),
        marketplace: Some("demo-market".to_owned()),
        version: version.map(str::to_owned),
    };
    assert_eq!(
        section.marketplaces,
        [Marketplace {
            name: "demo-market".to_owned(),
            source: "acme/demo-market".to_owned(),
        }]
    );
    assert_eq!(
        section.plugins,
        [
            demo("ghost@demo-market", None),
            demo("lint-tools@demo-market", Some("1.0.0"))
        ]
    );
    assert_eq!(
        section.mcp_servers[0],
        McpServer {
            name: "docs".to_owned(),
            transport: Transport::Http,
            command: None,
            args: Vec::new(),
            url: Some("https://docs.example.com/mcp".to_owned()),
            env_keys: Vec::new(),
            header_keys: vec!["Authorization".to_owned()],
        }
    );
}

#[test]
fn the_fixture_home_names_what_it_leaves_out_and_never_a_value() {
    let collection = fixture(None);

    assert_eq!(
        export_preview(&collection).excluded,
        [
            left_out(
                SyncItemKind::Marketplace,
                "openai-bundled",
                SyncExcludedReason::LocalPath
            ),
            left_out(
                SyncItemKind::Plugin,
                "pdf@openai-bundled",
                SyncExcludedReason::LocalPath
            ),
            left_out(SyncItemKind::McpServer, "github", SyncExcludedReason::ContainsSecret),
            left_out(SyncItemKind::McpServer, "paused", SyncExcludedReason::SwitchedOff),
        ]
    );
    assert_nothing_shows(&collection, &[FIXTURE_CANARY, "bundled-marketplaces"]);
}

#[test]
fn the_config_follows_codex_home() {
    assert_eq!(ids(&export_preview(&fixture(Some("codex-work")))), ["codex:mcp:notes"]);
}

/// A home with Codex's edge cases: pinned, local and SSH marketplaces, env names, header names, a
/// literal env value and the app's own servers.
fn edge_home() -> Home {
    let home = Home::empty();
    let secret = ["arg", "-value-", "Q7wZ"].concat();

    home.write_text(
        ".codex/config.toml",
        &format!(
            r#"
[marketplaces.tools]
source_type = "git"
source = "https://github.com/acme/codex-tools.git"

[marketplaces.pinned]
source_type = "git"
source = "acme/pinned"
ref = "v1"

[marketplaces.sparse]
source_type = "git"
source = "acme/mono"
sparse_paths = ["plugins/one"]

[marketplaces.over-ssh]
source_type = "git"
source = "git@github.com:acme/ssh.git"

[marketplaces.mine]
source_type = "local"
source = "/home/someone/market"

[marketplaces.odd]
source = "acme/odd"

[plugins."formatter@tools"]
enabled = false

[plugins."pinned-plugin@pinned"]

[plugins."bare"]

[mcp_servers.docs]
command = "docs-server"
args = ["--stdio"]
env = {{ DOCS_TOKEN = "{secret}" }}
env_vars = ["DOCS_REGION"]
startup_timeout_sec = 20

[mcp_servers.remote]
url = "https://mcp.example.com/mcp"
env_http_headers = {{ "X-Team" = "TEAM_HEADER" }}
bearer_token_env_var = "REMOTE_TOKEN"

[mcp_servers.home-arg]
command = "server"
args = ["${{HOME}}/notes"]

[mcp_servers.nothing]
startup_timeout_sec = 5

[mcp_servers.off]
command = "off-server"
enabled = false

[mcp_servers.harness-memory]
command = "harness-memory"
args = ["mcp"]
"#
        ),
    );

    home
}

#[test]
fn only_git_sources_another_machine_adds_the_same_way_are_shared() {
    let preview = export_preview(&edge_home().collect(CODEX));
    let marketplaces: Vec<String> = ids(&preview)
        .into_iter()
        .filter(|id| id.starts_with("codex:marketplace:"))
        .collect();

    assert_eq!(marketplaces, ["codex:marketplace:tools"]);
    assert_eq!(
        excluded_of_kind(&preview, SyncItemKind::Marketplace),
        [
            left_out(SyncItemKind::Marketplace, "pinned", SyncExcludedReason::Unsupported),
            left_out(SyncItemKind::Marketplace, "sparse", SyncExcludedReason::Unsupported),
            left_out(SyncItemKind::Marketplace, "over-ssh", SyncExcludedReason::Unsupported),
            left_out(SyncItemKind::Marketplace, "mine", SyncExcludedReason::LocalPath),
            left_out(SyncItemKind::Marketplace, "odd", SyncExcludedReason::Unsupported),
        ]
    );
    assert_eq!(
        excluded_of_kind(&preview, SyncItemKind::Plugin),
        [
            left_out(
                SyncItemKind::Plugin,
                "pinned-plugin@pinned",
                SyncExcludedReason::Unsupported
            ),
            left_out(SyncItemKind::Plugin, "bare", SyncExcludedReason::Invalid),
        ]
    );
}

#[test]
fn servers_share_variable_and_header_names_only() {
    let collection = edge_home().collect(CODEX);
    let preview = export_preview(&collection);
    let servers: Vec<String> = ids(&preview)
        .into_iter()
        .filter(|id| id.starts_with("codex:mcp:"))
        .collect();

    assert_eq!(servers, ["codex:mcp:docs", "codex:mcp:remote"]);
    assert_eq!(
        excluded_of_kind(&preview, SyncItemKind::McpServer),
        [
            left_out(SyncItemKind::McpServer, "home-arg", SyncExcludedReason::Invalid),
            left_out(SyncItemKind::McpServer, "nothing", SyncExcludedReason::Unsupported),
            left_out(SyncItemKind::McpServer, "off", SyncExcludedReason::SwitchedOff),
        ]
    );

    let items = &preview.sections[0].items;
    let docs = items.iter().find(|item| item.id == "codex:mcp:docs").unwrap();
    assert_eq!(docs.env_keys, ["DOCS_TOKEN", "DOCS_REGION"]);
    assert_eq!(
        docs.notes,
        [SyncNote::EnvValuesNotShared, SyncNote::OtherFieldsNotShared]
    );

    let remote = items.iter().find(|item| item.id == "codex:mcp:remote").unwrap();
    assert_eq!(remote.header_keys, ["X-Team", "Authorization"]);

    let secret = ["arg", "-value-", "Q7wZ"].concat();
    assert_nothing_shows(&collection, &[&secret, "TEAM_HEADER"]);
}

#[test]
fn every_codex_export_imports_again_without_invalid_items() {
    assert_imports_again(&fixture(None));
    assert_imports_again(&edge_home().collect(CODEX));
}

#[test]
fn codex_that_is_not_installed_is_not_read() {
    let preview = export_preview(&edge_home().collect(BOTH));

    assert!(preview.sections.is_empty());
    assert!(preview.excluded.is_empty());
}
