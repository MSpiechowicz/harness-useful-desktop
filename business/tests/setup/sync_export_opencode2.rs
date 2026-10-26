//! Export of "Share setup" for OpenCode 2: the plugins of `plugin` and `plugins` and the MCP servers
//! of `mcp.servers` and the legacy `mcp.<name>` in its global config, shared as references only.
//! The same files read as OpenCode 1 show what changed.

use crate::sync_support;

use harness_business::harness::{HarnessRegistry, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::{HarnessId, SyncExcluded, SyncExcludedReason, SyncItemKind, SyncNote};
use harness_business::sync::schema::{Plugin, Transport};
use harness_business::sync::{ExportCollection, build_bundle, export_preview};
use pretty_assertions::assert_eq;
use sync_support::servers::{github_server, http};
use sync_support::{BOTH, Home, ids};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];
const CONFIG_JSONC: &str = ".config/opencode/opencode.jsonc";
const CONFIG_JSON: &str = ".config/opencode/opencode.json";

/// Secrets built at run time, so no source line looks like a credential to a secret scanner.
fn secrets() -> [String; 3] {
    [
        ["env", "-value-", "Q7wZ"].concat(),
        ["options", "-value-", "T1uW"].concat(),
        ["tuple", "-value-", "S9vX"].concat(),
    ]
}

/// A global `opencode.jsonc` with comments, legacy and native keys, and secrets in values.
fn opencode2_home() -> Home {
    let [env_value, option_value, tuple_value] = secrets();
    let home = Home::empty();

    home.write_text(
        CONFIG_JSONC,
        &format!(
            r#"{{
  // Global OpenCode 2 settings
  "plugin": ["legacy-plugin@1.0.0", ["legacy-tuple", {{ "apiKey": "{tuple_value}" }}], "dropped-plugin"],
  "plugins": [
    "native-plugin@2.0.0",
    {{ "package": "plain-object@1.2.0" }},
    {{ "package": "@acme/with-options", "options": {{ "token": "{option_value}" }} }},
    "-dropped-plugin",
  ],
  "mcp": {{
    "legacy": {{ "type": "remote", "url": "https://legacy.example.com/mcp" }},
    "toggle": {{ "enabled": false }},
    "servers": {{
      "github": {{
        "type": "local",
        "command": ["npx", "-y", "@modelcontextprotocol/server-github"],
        "environment": {{ "GITHUB_TOKEN": "{env_value}" }},
        "disabled": true,
      }},
      "legacy": {{ "type": "remote", "url": "https://native.example.com/mcp" }},
      "tuned": {{ "type": "local", "command": ["tuned-server"], "timeout": {{ "startup": 5000 }} }},
    }},
  }},
}}"#
        ),
    );
    home.write(
        ".config/opencode/config.json",
        &serde_json::json!({ "plugin": ["config-json-plugin"] }),
    );

    home
}

/// The export with OpenCode 2 detected ([`Home::collect`] is OpenCode 1).
fn collect_v2(home: &Home, installed: &[HarnessId]) -> ExportCollection {
    let registry = HarnessRegistry::new(vec![Box::new(OpenCode::with_generation(OpenCodeGeneration::V2))]);
    home.collect_from(&registry, installed)
}

fn plugin(id: &str, version: Option<&str>) -> Plugin {
    Plugin {
        id: id.to_owned(),
        marketplace: None,
        version: version.map(str::to_owned),
    }
}

fn excluded(kind: SyncItemKind, label: &str, reason: SyncExcludedReason) -> SyncExcluded {
    SyncExcluded {
        harness: HarnessId::OpenCode,
        kind,
        label: label.to_owned(),
        reason,
    }
}

#[test]
fn plugins_come_from_both_keys_without_config_json_and_with_removals() {
    let home = opencode2_home();
    let collection = collect_v2(&home, OPENCODE);
    let preview = export_preview(&collection);

    assert_eq!(
        ids(&preview)[..3],
        [
            "opencode:plugin:legacy-plugin",
            "opencode:plugin:native-plugin",
            "opencode:plugin:plain-object",
        ]
    );

    let bundle = build_bundle(&collection, &ids(&preview), None, None).unwrap();
    assert_eq!(
        bundle.harnesses[&HarnessId::OpenCode].plugins,
        [
            plugin("legacy-plugin", Some("1.0.0")),
            plugin("native-plugin", Some("2.0.0")),
            plugin("plain-object", Some("1.2.0")),
        ]
    );

    let plugins: Vec<&SyncExcluded> = preview
        .excluded
        .iter()
        .filter(|item| item.kind == SyncItemKind::Plugin)
        .collect();
    assert_eq!(
        plugins,
        [
            &excluded(SyncItemKind::Plugin, "legacy-tuple", SyncExcludedReason::Unsupported),
            &excluded(
                SyncItemKind::Plugin,
                "@acme/with-options",
                SyncExcludedReason::Unsupported
            ),
        ]
    );

    let v1 = ids(&export_preview(&home.collect(OPENCODE)));
    assert!(v1.contains(&"opencode:plugin:config-json-plugin".to_owned()));
    assert!(v1.contains(&"opencode:plugin:dropped-plugin".to_owned()));
    assert!(!v1.contains(&"opencode:plugin:native-plugin".to_owned()));
}

#[test]
fn servers_come_from_mcp_servers_and_legacy_entries_with_the_nested_one_winning() {
    let home = opencode2_home();
    let collection = collect_v2(&home, OPENCODE);
    let preview = export_preview(&collection);

    assert_eq!(
        ids(&preview)[3..],
        ["opencode:mcp:github", "opencode:mcp:legacy", "opencode:mcp:tuned"]
    );

    let items = &preview.sections[0].items;
    assert_eq!(
        items[3].notes,
        [SyncNote::EnvValuesNotShared],
        "disabled is dropped without a note"
    );
    assert_eq!(items[4].detail.as_deref(), Some("https://native.example.com/mcp"));
    assert_eq!(items[5].notes, [SyncNote::OtherFieldsNotShared]);

    let bundle = build_bundle(&collection, &ids(&preview), None, None).unwrap();
    let servers = &bundle.harnesses[&HarnessId::OpenCode].mcp_servers;
    assert_eq!(servers[0], github_server());
    assert_eq!(servers[1], http("legacy", "https://native.example.com/mcp"));
    assert_eq!(servers[2].transport, Transport::Stdio);

    let text = serde_json::to_string(&preview).unwrap() + &serde_json::to_string(&bundle).unwrap();
    for secret in secrets() {
        assert!(!text.contains(&secret), "leaks {secret:?}");
    }
}

#[test]
fn opencode_config_dir_replaces_the_global_directory() {
    let mut home = Home::empty();
    home.write(CONFIG_JSON, &serde_json::json!({ "plugin": ["global-plugin"] }));
    home.write(
        "team/opencode.json",
        &serde_json::json!({
            "plugins": ["team-plugin"],
            "mcp": { "servers": { "team": { "type": "remote", "url": "https://team.example.com/mcp" } } },
        }),
    );
    let team = home.path("team");
    home.set_var("OPENCODE_CONFIG_DIR", &team.to_string_lossy());

    assert_eq!(
        ids(&export_preview(&collect_v2(&home, OPENCODE))),
        ["opencode:plugin:team-plugin", "opencode:mcp:team"]
    );
    assert_eq!(
        ids(&export_preview(&home.collect(OPENCODE))),
        ["opencode:plugin:global-plugin"]
    );
    assert!(
        export_preview(&collect_v2(&home, BOTH)).sections.is_empty(),
        "OpenCode is not installed"
    );
}

#[test]
fn nested_servers_that_are_not_an_object_make_the_file_invalid() {
    let home = Home::empty();
    home.write(CONFIG_JSON, &serde_json::json!({ "mcp": { "servers": [] } }));

    let preview = export_preview(&collect_v2(&home, OPENCODE));

    assert_eq!(
        preview.excluded,
        [excluded(
            SyncItemKind::McpServer,
            "~/.config/opencode/opencode.json",
            SyncExcludedReason::Invalid
        )]
    );
}
