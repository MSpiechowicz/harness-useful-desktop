//! Registering the dashboard's MCP server in OpenCode's global config file: which file is
//! written, merging into it, recognising an existing entry in any config file OpenCode reads, and
//! offering the entry to paste instead of writing a file with comments.

mod mcp_support;

use std::path::PathBuf;

use harness_business::ecosystem::{McpFile, mcp_state, mcp_target, memory_mcp_state, register_dashboard_mcp};
use harness_business::harnesses::OpenCode;
use harness_business::model::McpState;
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

impl Home {
    /// `name` in OpenCode's global config directory, `~/.config/opencode`.
    fn opencode(&self, name: &str) -> PathBuf {
        self.dir.path().join(".config/opencode").join(name)
    }

    fn write_opencode(&self, name: &str, text: &str) -> PathBuf {
        let path = self.opencode(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    /// The file OpenCode's servers are registered in, as the setup step picks it.
    fn opencode_file(&self) -> McpFile {
        mcp_target(&self.env, &OpenCode).expect("OpenCode has an MCP file")
    }

    fn register_opencode(&self) -> McpState {
        register_dashboard_mcp(&self.env, &self.opencode_file())
    }

    fn opencode_state(&self) -> McpState {
        mcp_state(&self.env, &self.opencode_file())
    }

    /// Every file name in OpenCode's config directory, sorted.
    fn opencode_files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.opencode(""))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

const OPENCODE_JSON: &str = "~/.config/opencode/opencode.json";
const OPENCODE_JSONC: &str = "~/.config/opencode/opencode.jsonc";

/// What OpenCode writes into a config file it creates.
const OPENCODE_SEED: &str = "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n}";

fn opencode_dashboard_entry() -> Value {
    json!({ "type": "local", "command": ["harness-dashboard", "mcp"], "enabled": true })
}

/// The servers a manual-edit snippet holds, read as the content of an object.
fn pasted(snippet: &str) -> Value {
    serde_json::from_str(&format!("{{{snippet}}}")).expect("the snippet is an object member")
}

fn configured(file: &str, name: &str) -> McpState {
    McpState::Configured {
        file: file.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn opencode_without_a_config_file_gets_opencode_json() {
    let home = Home::new();

    assert_eq!(
        home.opencode_state(),
        McpState::Missing {
            file: OPENCODE_JSON.to_owned(),
            exists: false
        }
    );
    assert!(!home.opencode("").exists(), "reading the state writes nothing");

    assert_eq!(home.register_opencode(), configured(OPENCODE_JSON, "harness-dashboard"));

    let written = read(&home.opencode("opencode.json"));
    assert_eq!(
        written,
        json!({
            "$schema": "https://opencode.ai/config.json",
            "mcp": { "harness-dashboard": opencode_dashboard_entry() }
        })
    );
    assert_eq!(keys(&written), vec!["$schema", "mcp"]);
    assert_eq!(home.opencode_files(), vec!["opencode.json"]);

    #[cfg(unix)]
    assert_eq!(mode(&home.opencode("opencode.json")), 0o600);

    // A second run changes nothing.
    let first = std::fs::read(home.opencode("opencode.json")).unwrap();
    assert_eq!(home.register_opencode(), configured(OPENCODE_JSON, "harness-dashboard"));
    assert_eq!(std::fs::read(home.opencode("opencode.json")).unwrap(), first);
}

#[test]
fn opencode_jsonc_is_preferred_and_keeps_its_schema_and_other_keys() {
    let home = Home::new();
    home.write_opencode("opencode.json", r#"{"model":"anthropic/claude"}"#);
    let original = json!({
        "$schema": "https://opencode.ai/config.json",
        "theme": "opencode",
        "mcp": {
            "github": {
                "type": "local",
                "command": ["github-mcp"],
                "environment": { "GITHUB_TOKEN": "CANARY-mcp-token-7f3a9c" }
            }
        },
        "zLast": [1, 2.5, "three"]
    });
    let path = home.write_opencode("opencode.jsonc", &serde_json::to_string_pretty(&original).unwrap());

    assert_eq!(
        home.opencode_state(),
        McpState::Missing {
            file: OPENCODE_JSONC.to_owned(),
            exists: true
        }
    );
    assert_eq!(
        home.register_opencode(),
        configured(OPENCODE_JSONC, "harness-dashboard")
    );

    let merged = read(&path);
    let mut expected = original.clone();
    expected["mcp"]["harness-dashboard"] = opencode_dashboard_entry();

    assert_eq!(merged, expected);
    assert_eq!(keys(&merged), keys(&original));
    assert_eq!(keys(&merged["mcp"]), vec!["github", "harness-dashboard"]);
    assert_eq!(
        read(&home.opencode("opencode.json")),
        json!({ "model": "anthropic/claude" }),
        "the other config file is only read"
    );
}

#[test]
fn opencodes_own_seed_file_is_merged_into() {
    let home = Home::new();
    let path = home.write_opencode("opencode.jsonc", OPENCODE_SEED);

    assert_eq!(
        home.register_opencode(),
        configured(OPENCODE_JSONC, "harness-dashboard")
    );
    assert_eq!(
        read(&path),
        json!({
            "$schema": "https://opencode.ai/config.json",
            "mcp": { "harness-dashboard": opencode_dashboard_entry() }
        })
    );
    assert_eq!(home.opencode_files(), vec!["opencode.jsonc"]);
}

#[test]
fn an_existing_opencode_json_is_merged_into_when_there_is_no_jsonc() {
    let home = Home::new();
    let path = home.write_opencode("opencode.json", r#"{"mcp":{}}"#);

    assert_eq!(home.register_opencode(), configured(OPENCODE_JSON, "harness-dashboard"));
    assert_eq!(
        read(&path),
        json!({ "mcp": { "harness-dashboard": opencode_dashboard_entry() } })
    );
}

#[test]
fn a_user_opencode_entry_is_never_overwritten() {
    let home = Home::new();
    let cases = [
        (
            r#"{"mcp":{"harness-dashboard":{"type":"remote","url":"http://localhost:4000/mcp"}}}"#,
            "harness-dashboard",
        ),
        (
            r#"{"mcp":{"usage":{"type":"local","command":["/home/me/.local/bin/harness-dashboard","mcp"]}}}"#,
            "usage",
        ),
    ];

    for (text, name) in cases {
        home.write_opencode("opencode.json", text);

        assert_eq!(home.register_opencode(), configured(OPENCODE_JSON, name));
        assert_eq!(std::fs::read_to_string(home.opencode("opencode.json")).unwrap(), text);
    }

    // Another binary, or other arguments, is not the dashboard's server.
    let other = r#"{"mcp":{"x":{"type":"local","command":["harness-dashboard","serve"]}}}"#;
    home.write_opencode("opencode.json", other);
    assert_eq!(
        home.opencode_state(),
        McpState::Missing {
            file: OPENCODE_JSON.to_owned(),
            exists: true
        }
    );
}

#[test]
fn a_disabled_opencode_entry_is_reported_and_left_alone() {
    let home = Home::new();
    let text =
        r#"{"mcp":{"harness-dashboard":{"type":"local","command":["harness-dashboard","mcp"],"enabled":false}}}"#;
    home.write_opencode("opencode.json", text);

    let disabled = McpState::Disabled {
        file: OPENCODE_JSON.to_owned(),
        name: "harness-dashboard".to_owned(),
    };
    assert_eq!(home.opencode_state(), disabled);
    assert_eq!(home.register_opencode(), disabled);
    assert_eq!(std::fs::read_to_string(home.opencode("opencode.json")).unwrap(), text);
}

#[test]
fn an_opencode_file_with_comments_is_never_written() {
    let cases = [
        "{\n  // my servers\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"mcp\": {}\n}\n",
        "{ /* theme */ \"theme\": \"opencode\" }",
        "{ \"mcp\": { \"a\": { \"type\": \"remote\", \"url\": \"https://x\" }, }, } // unclosed /*",
    ];

    for text in cases {
        let home = Home::new();
        let path = home.write_opencode("opencode.jsonc", text);

        let state = home.opencode_state();
        let McpState::ManualEdit { file, snippet } = &state else {
            panic!("expected a manual edit, got {state:?}");
        };
        assert_eq!(file, OPENCODE_JSONC);
        assert_eq!(
            pasted(snippet),
            json!({ "harness-dashboard": opencode_dashboard_entry() })
        );

        assert_eq!(home.register_opencode(), state);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "unchanged byte for byte");
        assert_eq!(home.opencode_files(), vec!["opencode.jsonc"], "nothing else is created");
    }
}

#[test]
fn trailing_commas_alone_are_dropped_by_the_rewrite() {
    // OpenCode adds `$schema` to a config without one by inserting `"$schema": "...",` after the `{`,
    // which leaves `{}` with a trailing comma.
    let cases = [
        (
            "{\n  \"$schema\": \"https://opencode.ai/config.json\",}",
            json!({ "$schema": "https://opencode.ai/config.json" }),
        ),
        (
            "{ \"theme\": \"opencode\", \"mcp\": { \"a\": { \"type\": \"remote\", \"url\": \"https://x/*y*/\" }, }, }",
            json!({ "theme": "opencode", "mcp": { "a": { "type": "remote", "url": "https://x/*y*/" } } }),
        ),
    ];

    for (text, original) in cases {
        let home = Home::new();
        let path = home.write_opencode("opencode.jsonc", text);

        assert_eq!(
            home.opencode_state(),
            McpState::Missing {
                file: OPENCODE_JSONC.to_owned(),
                exists: true
            }
        );
        assert_eq!(
            home.register_opencode(),
            configured(OPENCODE_JSONC, "harness-dashboard")
        );

        let mut expected = original;
        expected["mcp"]["harness-dashboard"] = opencode_dashboard_entry();
        assert_eq!(read(&path), expected, "plain JSON now, every value kept");
    }
}

#[test]
fn an_entry_in_a_commented_opencode_file_is_found() {
    let home = Home::new();
    let text = r#"{
  // added by hand
  "mcp": {
    "harness-dashboard": { "type": "local", "command": ["harness-dashboard", "mcp"] }, // usage
  },
}"#;
    home.write_opencode("opencode.jsonc", text);

    assert_eq!(
        home.register_opencode(),
        configured(OPENCODE_JSONC, "harness-dashboard")
    );
    assert_eq!(std::fs::read_to_string(home.opencode("opencode.jsonc")).unwrap(), text);
}

#[test]
fn an_entry_in_another_opencode_config_file_counts_and_nothing_is_created() {
    let home = Home::new();
    let entry = r#"{"mcp":{"dash":{"type":"local","command":["harness-dashboard","mcp"]}}}"#;
    home.write_opencode("config.json", entry);

    assert_eq!(
        home.register_opencode(),
        configured("~/.config/opencode/config.json", "dash")
    );
    assert!(!home.opencode("opencode.json").exists());

    // `OPENCODE_CONFIG_DIR` adds a directory that is read, never written.
    let home = Home::new();
    let team = home.dir.path().join("team");
    std::fs::create_dir_all(&team).unwrap();
    std::fs::write(team.join("opencode.jsonc"), format!("// team\n{entry}")).unwrap();
    let env = home.env.clone().with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());
    let file = mcp_target(&env, &OpenCode).unwrap();

    assert_eq!(file.path, home.opencode("opencode.json"));
    assert_eq!(
        register_dashboard_mcp(&env, &file),
        configured("~/team/opencode.jsonc", "dash")
    );
    assert!(!home.opencode("opencode.json").exists());
}

#[test]
fn the_memory_servers_opencode_snippet_runs_the_server() {
    let home = Home::new();
    home.write_opencode("opencode.jsonc", "// mine\n{}");

    let state = memory_mcp_state(&home.env, &home.opencode_file());

    let McpState::ManualEdit { file, snippet } = &state else {
        panic!("expected a manual edit, got {state:?}");
    };
    assert_eq!(file, OPENCODE_JSONC);
    assert_eq!(
        pasted(snippet),
        json!({ "harness-memory": { "type": "local", "command": ["harness-memory", "mcp"], "enabled": true } }),
        "named, while the server is not installed"
    );
}
