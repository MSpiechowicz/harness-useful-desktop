//! Export of "Share setup" for OpenCode: the npm plugins and the MCP servers of the global config
//! (JSON with comments), shared as references only; everything else is named with a reason and no
//! value.

mod sync_support;

use harness_business::model::{HarnessId, SyncExcluded, SyncExcludedReason, SyncItemKind, SyncNote};
use harness_business::sync::codec::{decode_input, to_json_pretty};
use harness_business::sync::schema::{Bundle, McpServer, Plugin, Transport};
use harness_business::sync::{ExportCollection, build_bundle, export_code, export_preview};
use pretty_assertions::assert_eq;
use sync_support::{BOTH, Home, ids};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];
const CONFIG_JSONC: &str = ".config/opencode/opencode.jsonc";
const CONFIG_JSON: &str = ".config/opencode/opencode.json";

/// Secrets built at run time, so no source line looks like a credential to a secret scanner.
fn secrets() -> [String; 4] {
    [
        ["env", "-value-", "Q7wZ"].concat(),
        ["header", "-value-", "R8xY"].concat(),
        ["arg", "-value-", "S9vX"].concat(),
        ["options", "-value-", "T1uW"].concat(),
    ]
}

/// A global `opencode.jsonc` with comments: servers and plugins of every kind, secrets in values.
fn opencode_home() -> Home {
    let [env_value, header_value, arg_value, option_value] = secrets();
    let home = Home::empty();

    home.write_text(
        CONFIG_JSONC,
        &format!(
            r#"{{
  // Global OpenCode settings
  "$schema": "https://opencode.ai/config.json",
  "plugin": [
    "opencode-wakatime",
    "@acme/opencode-plugin@1.2.3",
    "opencode-helper@latest",
    "./plugins/my-plugin.ts",
    "file:///home/someone/plugins/local-plugin.ts",
    "github:acme/git-plugin",
    "https://example.com/url-plugin.tgz",
    ["tuple-plugin", {{ "apiKey": "{option_value}" }}],
    "ranged-plugin@^2.0.0",
  ],
  "mcp": {{
    "github": {{
      "type": "local",
      "command": ["npx", "-y", "@modelcontextprotocol/server-github"],
      "environment": {{ "GITHUB_TOKEN": "{env_value}" }},
      "enabled": false,
    }},
    "docs": {{
      "type": "remote",
      "url": "https://docs.example.com/mcp",
      "headers": {{ "Authorization": "Bearer {header_value}" }},
      "oauth": {{ "clientId": "abc" }},
      "timeout": 5000,
    }},
    "leaky-args": {{ "type": "local", "command": ["server", "--token", "{arg_value}"] }},
    "env-url": {{ "type": "remote", "url": "https://x.example.com/{{env:API_KEY}}" }},
    "file-arg": {{ "type": "local", "command": ["server", "{{file:notes.txt}}"] }},
    "local-bin": {{ "type": "local", "command": ["/home/someone/bin/server"] }},
    "socket": {{ "type": "socket", "path": "/tmp/x" }},
    "harness-dashboard": {{ "type": "local", "command": ["harness-dashboard", "mcp"] }},
  }},
}}"#
        ),
    );

    home
}

fn bundle_of_everything(collection: &ExportCollection) -> Bundle {
    build_bundle(collection, &ids(&export_preview(collection)), None, None).unwrap()
}

#[test]
fn servers_and_npm_plugins_are_shared_by_reference_from_a_commented_config() {
    let home = opencode_home();
    let collection = home.collect(OPENCODE);
    let preview = export_preview(&collection);

    assert_eq!(
        ids(&preview),
        [
            "opencode:plugin:@acme/opencode-plugin",
            "opencode:plugin:opencode-helper",
            "opencode:plugin:opencode-wakatime",
            "opencode:mcp:docs",
            "opencode:mcp:github",
        ]
    );

    let items = &preview.sections[0].items;
    assert_eq!(items[0].version.as_deref(), Some("1.2.3"));
    assert!(items[0].notes.is_empty(), "the pin is installed as is");
    assert_eq!(items[2].version, None);

    let docs = &items[3];
    assert_eq!(docs.detail.as_deref(), Some("https://docs.example.com/mcp"));
    assert_eq!(docs.header_keys, ["Authorization"]);
    assert!(!docs.runs_program);
    assert_eq!(
        docs.notes,
        [
            SyncNote::HeaderValuesNotShared,
            SyncNote::AuthNotShared,
            SyncNote::OtherFieldsNotShared
        ]
    );

    let github = &items[4];
    assert_eq!(
        github.detail.as_deref(),
        Some("npx -y @modelcontextprotocol/server-github")
    );
    assert!(github.runs_program);
    assert_eq!(github.env_keys, ["GITHUB_TOKEN"]);
    assert_eq!(github.notes, [SyncNote::EnvValuesNotShared]);

    let bundle = bundle_of_everything(&collection);
    let section = &bundle.harnesses[&HarnessId::OpenCode];
    assert_eq!(
        section.plugins,
        [
            plugin("@acme/opencode-plugin", Some("1.2.3")),
            plugin("opencode-helper", Some("latest")),
            plugin("opencode-wakatime", None),
        ]
    );
    assert_eq!(
        section.mcp_servers,
        [
            McpServer {
                name: "docs".to_owned(),
                transport: Transport::Http,
                command: None,
                args: Vec::new(),
                url: Some("https://docs.example.com/mcp".to_owned()),
                env_keys: Vec::new(),
                header_keys: vec!["Authorization".to_owned()],
            },
            McpServer {
                name: "github".to_owned(),
                transport: Transport::Stdio,
                command: Some("npx".to_owned()),
                args: vec!["-y".to_owned(), "@modelcontextprotocol/server-github".to_owned()],
                url: None,
                env_keys: vec!["GITHUB_TOKEN".to_owned()],
                header_keys: Vec::new(),
            },
        ]
    );
}

fn plugin(id: &str, version: Option<&str>) -> Plugin {
    Plugin {
        id: id.to_owned(),
        marketplace: None,
        version: version.map(str::to_owned),
    }
}

#[test]
fn local_git_url_tuple_and_secret_entries_are_named_with_a_reason() {
    let preview = export_preview(&opencode_home().collect(OPENCODE));

    let excluded = |kind, label: &str, reason| SyncExcluded {
        harness: HarnessId::OpenCode,
        kind,
        label: label.to_owned(),
        reason,
    };
    use SyncExcludedReason::*;
    use SyncItemKind::{McpServer, Plugin};

    assert_eq!(
        preview.excluded,
        [
            excluded(Plugin, "my-plugin.ts", LocalPath),
            excluded(Plugin, "local-plugin.ts", LocalPath),
            excluded(Plugin, "git-plugin", Unsupported),
            excluded(Plugin, "url-plugin.tgz", Unsupported),
            excluded(Plugin, "tuple-plugin", Unsupported),
            excluded(Plugin, "ranged-plugin", Unsupported),
            excluded(McpServer, "leaky-args", ContainsSecret),
            excluded(McpServer, "env-url", Invalid),
            excluded(McpServer, "file-arg", Invalid),
            excluded(McpServer, "local-bin", LocalPath),
            excluded(McpServer, "socket", Unsupported),
        ]
    );
}

#[test]
fn no_value_reaches_the_preview_the_code_or_the_file_and_the_export_imports_again() {
    let home = opencode_home();
    let collection = home.collect(OPENCODE);
    let bundle = bundle_of_everything(&collection);
    let code = export_code(&bundle).unwrap().code;
    let file = to_json_pretty(&bundle).unwrap();
    let preview = serde_json::to_string(&export_preview(&collection)).unwrap();

    for text in [&preview, &file, &format!("{:?}", decode_input(&code).unwrap())] {
        for secret in secrets() {
            assert!(!text.contains(&secret), "leaks {secret:?}");
        }
        for local in [
            "/home/someone",
            "./plugins",
            "file:",
            "{env:",
            "{file:",
            "clientId",
            "5000",
        ] {
            assert!(!text.contains(local), "leaks {local:?}");
        }
    }

    for input in [code, file] {
        let decoded = decode_input(&input).unwrap();
        assert_eq!(decoded.invalid_count(), 0);
        assert!(decoded.unsupported.is_empty());
    }
}

#[test]
fn the_json_file_is_read_without_a_jsonc_one_and_only_global_files_count() {
    let home = Home::empty();
    home.write(
        CONFIG_JSON,
        &serde_json::json!({
            "plugin": ["global-plugin@0.3.0"],
            "mcp": { "remote": { "type": "remote", "url": "https://mcp.example.com/mcp" } },
        }),
    );
    home.write(
        ".opencode/opencode.json",
        &serde_json::json!({ "plugin": ["home-plugin"], "mcp": { "other": { "type": "remote", "url": "https://other.example.com" } } }),
    );

    assert_eq!(
        ids(&export_preview(&home.collect(OPENCODE))),
        ["opencode:plugin:global-plugin", "opencode:mcp:remote"]
    );
    assert!(
        export_preview(&home.collect(BOTH)).sections.is_empty(),
        "OpenCode is not installed"
    );
}

#[test]
fn an_unparsable_config_is_reported_by_its_path() {
    let home = Home::empty();
    home.write_text(CONFIG_JSON, "{ \"mcp\": ");

    let preview = export_preview(&home.collect(OPENCODE));

    assert!(preview.sections.is_empty());
    assert_eq!(
        preview.excluded,
        [SyncExcluded {
            harness: HarnessId::OpenCode,
            kind: SyncItemKind::McpServer,
            label: "~/.config/opencode/opencode.json".to_owned(),
            reason: SyncExcludedReason::Invalid,
        }]
    );
}
