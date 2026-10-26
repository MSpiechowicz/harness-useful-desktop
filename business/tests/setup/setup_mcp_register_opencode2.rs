//! Registering the dashboard's, the memory server's and the app's MCP servers in OpenCode 2's
//! global config file: new entries go under `mcp.servers` as `{type: "local", command: [bin,
//! "mcp"]}` (no `enabled`), an existing entry is recognised in either shape (`mcp.servers.<name>`
//! or the legacy `mcp.<name>`) so nothing is doubled, `disabled: true` switches one off, and the
//! files are the ones OpenCode 2 reads (no `config.json`, `OPENCODE_CONFIG_DIR` replacing the
//! global directory).

use crate::mcp_support;

use std::path::{Path, PathBuf};

use harness_business::ecosystem::{
    DesktopRegistration, McpFile, MemoryRegistration, Shape, mcp_state, mcp_target, memory_mcp_state,
    register_dashboard_mcp, register_desktop_mcp, register_memory_mcp, share_mcp_target, unregister_memory_mcp,
};
use harness_business::env::Env;
use harness_business::harness::OpenCodeGeneration;
use harness_business::harnesses::OpenCode;
use harness_business::model::{McpRefusal, McpState};
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

const OPENCODE_JSON: &str = "~/.config/opencode/opencode.json";

/// Helpers of these tests on the shared [`Home`].
trait OpenCode2Setup {
    /// `name` in `~/.config/opencode`.
    fn opencode(&self, name: &str) -> PathBuf;

    fn write_opencode(&self, name: &str, text: &str) -> PathBuf;

    fn file(&self) -> McpFile;

    fn register(&self) -> McpState;

    fn state(&self) -> McpState;
}

impl OpenCode2Setup for Home {
    fn opencode(&self, name: &str) -> PathBuf {
        self.dir.path().join(".config/opencode").join(name)
    }

    fn write_opencode(&self, name: &str, text: &str) -> PathBuf {
        let path = self.opencode(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn file(&self) -> McpFile {
        target(&self.env)
    }

    fn register(&self) -> McpState {
        register_dashboard_mcp(&self.env, &self.file())
    }

    fn state(&self) -> McpState {
        mcp_state(&self.env, &self.file())
    }
}

/// The file OpenCode 2's servers are registered in, as the setup step picks it.
fn target(env: &Env) -> McpFile {
    mcp_target(env, &OpenCode::with_generation(OpenCodeGeneration::V2)).expect("OpenCode has an MCP file")
}

fn entry(program: &str) -> Value {
    json!({ "type": "local", "command": [program, "mcp"] })
}

fn configured(file: &str, name: &str) -> McpState {
    McpState::Configured {
        file: file.to_owned(),
        name: name.to_owned(),
    }
}

/// The servers a manual-edit snippet holds, read as the content of an object.
fn pasted(snippet: &str) -> Value {
    serde_json::from_str(&format!("{{{snippet}}}")).expect("the snippet is an object member")
}

#[test]
fn opencode_2_gets_a_nested_entry_without_enabled() {
    let home = Home::new();

    assert_eq!(home.file().shape, Shape::OpenCodeV2);
    assert_eq!(
        home.state(),
        McpState::Missing {
            file: OPENCODE_JSON.to_owned(),
            exists: false
        }
    );

    assert_eq!(home.register(), configured(OPENCODE_JSON, "harness-dashboard"));

    let written = read(&home.opencode("opencode.json"));
    assert_eq!(
        written,
        json!({
            "$schema": "https://opencode.ai/config.json",
            "mcp": { "servers": { "harness-dashboard": entry("harness-dashboard") } }
        })
    );

    #[cfg(unix)]
    assert_eq!(mode(&home.opencode("opencode.json")), 0o600);

    let first = std::fs::read(home.opencode("opencode.json")).unwrap();
    assert_eq!(home.register(), configured(OPENCODE_JSON, "harness-dashboard"));
    assert_eq!(
        std::fs::read(home.opencode("opencode.json")).unwrap(),
        first,
        "a second run changes nothing"
    );
}

#[test]
fn legacy_servers_and_other_keys_are_kept_beside_the_nested_entry() {
    let home = Home::new();
    let original = json!({
        "model": "anthropic/claude",
        "mcp": {
            "github": { "type": "local", "command": ["github-mcp"], "environment": { "TOKEN": "CANARY-7f3a" } },
            "timeout": { "startup": 5000 },
            "servers": { "docs": { "type": "remote", "url": "https://docs.example.com/mcp" } }
        },
        "zLast": true
    });
    let path = home.write_opencode("opencode.json", &serde_json::to_string_pretty(&original).unwrap());

    assert_eq!(home.register(), configured(OPENCODE_JSON, "harness-dashboard"));

    let mut expected = original.clone();
    expected["mcp"]["servers"]["harness-dashboard"] = entry("harness-dashboard");
    let merged = read(&path);
    assert_eq!(merged, expected);
    assert_eq!(keys(&merged), keys(&original));
    assert_eq!(keys(&merged["mcp"]), vec!["github", "timeout", "servers"]);
}

#[test]
fn an_existing_entry_in_either_shape_is_recognised_and_never_doubled() {
    let cases = [
        (
            r#"{"mcp":{"harness-dashboard":{"type":"local","command":["harness-dashboard","mcp"],"enabled":true}}}"#,
            "harness-dashboard",
        ),
        (
            r#"{"mcp":{"usage":{"type":"local","command":["/opt/bin/harness-dashboard","mcp"]}}}"#,
            "usage",
        ),
        (
            r#"{"mcp":{"servers":{"dash":{"type":"local","command":["harness-dashboard","mcp"]}}}}"#,
            "dash",
        ),
    ];

    for (text, name) in cases {
        let home = Home::new();
        home.write_opencode("opencode.json", text);

        assert_eq!(home.state(), configured(OPENCODE_JSON, name), "{text}");
        assert_eq!(home.register(), configured(OPENCODE_JSON, name), "{text}");
        assert_eq!(std::fs::read_to_string(home.opencode("opencode.json")).unwrap(), text);
    }
}

#[test]
fn disabled_and_legacy_switched_off_entries_are_reported_as_disabled() {
    for text in [
        r#"{"mcp":{"servers":{"harness-dashboard":{"type":"local","command":["harness-dashboard","mcp"],"disabled":true}}}}"#,
        r#"{"mcp":{"harness-dashboard":{"type":"local","command":["harness-dashboard","mcp"],"enabled":false}}}"#,
    ] {
        let home = Home::new();
        home.write_opencode("opencode.json", text);

        let disabled = McpState::Disabled {
            file: OPENCODE_JSON.to_owned(),
            name: "harness-dashboard".to_owned(),
        };
        assert_eq!(home.state(), disabled, "{text}");
        assert_eq!(home.register(), disabled);
        assert_eq!(std::fs::read_to_string(home.opencode("opencode.json")).unwrap(), text);
    }
}

#[test]
fn a_legacy_entry_that_only_switches_a_server_is_not_a_server() {
    let home = Home::new();
    let path = home.write_opencode("opencode.json", r#"{"mcp":{"harness-dashboard":{"enabled":false}}}"#);

    assert_eq!(home.register(), configured(OPENCODE_JSON, "harness-dashboard"));
    assert_eq!(
        read(&path),
        json!({ "mcp": {
            "harness-dashboard": { "enabled": false },
            "servers": { "harness-dashboard": entry("harness-dashboard") }
        }})
    );
}

#[test]
fn only_the_files_opencode_2_reads_count() {
    let home = Home::new();
    let entry_text = r#"{"mcp":{"dash":{"type":"local","command":["harness-dashboard","mcp"]}}}"#;
    home.write_opencode("config.json", entry_text);

    assert_eq!(
        home.state(),
        McpState::Missing {
            file: OPENCODE_JSON.to_owned(),
            exists: false
        },
        "OpenCode 2 reads no config.json"
    );

    // The other settings file of the global directory and the file `OPENCODE_CONFIG` names are
    // read, never written.
    let home = Home::new();
    home.write_opencode("opencode.jsonc", &format!("// mine\n{entry_text}"));
    assert_eq!(home.file().path, home.opencode("opencode.jsonc"));

    let home = Home::new();
    home.write_opencode("opencode.json", "{}");
    let explicit = home.dir.path().join("team.json");
    std::fs::write(&explicit, entry_text).unwrap();
    let env = home.env.clone().with_var("OPENCODE_CONFIG", explicit.to_string_lossy());
    assert_eq!(
        register_dashboard_mcp(&env, &target(&env)),
        configured("~/team.json", "dash")
    );
    assert_eq!(read(&home.opencode("opencode.json")), json!({}));
}

/// OpenCode 2 reads `opencode.json[c]` in the home directory and in `~/.opencode` only for
/// projects below the home directory, so a server found only there is registered globally too
/// (and the files are never written). Share setup still checks them.
#[test]
fn the_home_folder_configs_do_not_count_as_registered() {
    let nested = r#"{"mcp":{"servers":{"harness-dashboard":{"type":"local","command":["harness-dashboard","mcp"]},"harness-memory":{"type":"local","command":["harness-memory","mcp"]},"harness-desktop":{"type":"local","command":["/opt/Harness/harness-desktop","mcp"]}}}}"#;

    for home_file in ["opencode.jsonc", ".opencode/opencode.json"] {
        let home = Home::new();
        let walked = home.dir.path().join(home_file);
        std::fs::create_dir_all(walked.parent().unwrap()).unwrap();
        std::fs::write(&walked, nested).unwrap();

        let file = home.file();
        assert!(!file.also_read.contains(&walked), "{home_file}");
        let share = share_mcp_target(&home.env, &OpenCode::with_generation(OpenCodeGeneration::V2)).unwrap();
        assert!(share.also_read.contains(&walked), "{home_file}");

        assert_eq!(home.register(), configured(OPENCODE_JSON, "harness-dashboard"));
        assert_eq!(
            register_memory_mcp(&file, Path::new("/opt/bin/harness-memory"), false),
            MemoryRegistration::Added
        );
        assert_eq!(
            register_desktop_mcp(&file, Path::new("/opt/Harness/harness-desktop"), false, None),
            DesktopRegistration::Added
        );

        let servers = &read(&home.opencode("opencode.json"))["mcp"]["servers"];
        assert_eq!(servers["harness-dashboard"], entry("harness-dashboard"));
        assert_eq!(servers["harness-memory"], entry("/opt/bin/harness-memory"));
        assert_eq!(servers["harness-desktop"], entry("/opt/Harness/harness-desktop"));
        assert_eq!(
            std::fs::read_to_string(&walked).unwrap(),
            nested,
            "{home_file} is never written"
        );
    }
}

#[test]
fn the_custom_config_dir_replaces_the_global_one() {
    let home = Home::new();
    let team = home.dir.path().join("team");
    std::fs::create_dir_all(&team).unwrap();
    home.write_opencode(
        "opencode.json",
        r#"{"mcp":{"servers":{"dash":{"type":"local","command":["harness-dashboard","mcp"]}}}}"#,
    );
    let env = home.env.clone().with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());
    let file = target(&env);

    assert_eq!(file.path, team.join("opencode.json"));
    assert!(
        !file.also_read.contains(&home.opencode("opencode.json")),
        "not read on OpenCode 2"
    );
    assert_eq!(
        register_dashboard_mcp(&env, &file),
        configured("~/team/opencode.json", "harness-dashboard")
    );
    assert_eq!(
        read(&team.join("opencode.json"))["mcp"]["servers"]["harness-dashboard"],
        entry("harness-dashboard")
    );
}

#[test]
fn a_commented_file_offers_the_nested_entry_to_paste() {
    let home = Home::new();
    let text = "{\n  // mine\n  \"mcp\": {}\n}\n";
    let path = home.write_opencode("opencode.json", text);

    let state = home.register();
    let McpState::ManualEdit { file, snippet } = &state else {
        panic!("expected a manual edit, got {state:?}");
    };
    assert_eq!(file, OPENCODE_JSON);
    assert_eq!(
        pasted(snippet),
        json!({ "harness-dashboard": entry("harness-dashboard") })
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    let memory = memory_mcp_state(&home.env, &home.file());
    let McpState::ManualEdit { snippet, .. } = &memory else {
        panic!("expected a manual edit, got {memory:?}");
    };
    assert_eq!(pasted(snippet), json!({ "harness-memory": entry("harness-memory") }));
}

#[test]
fn servers_that_are_not_an_object_are_refused() {
    for text in [
        r#"{"mcp":{"servers":[]}}"#,
        r#"{"mcp":{"servers":{"type":"local","command":["x"]}}}"#,
    ] {
        let home = Home::new();
        home.write_opencode("opencode.json", text);

        assert_eq!(
            home.register(),
            McpState::Refused {
                file: OPENCODE_JSON.to_owned(),
                reason: McpRefusal::ServersNotAnObject
            },
            "{text}"
        );
        assert_eq!(std::fs::read_to_string(home.opencode("opencode.json")).unwrap(), text);
    }
}

#[test]
fn the_memory_server_is_added_nested_repointed_in_place_and_removed() {
    let home = Home::new();
    let file = home.file();
    let path = home.opencode("opencode.json");

    assert_eq!(
        register_memory_mcp(&file, Path::new("/opt/bin/harness-memory"), false),
        MemoryRegistration::Added
    );
    assert_eq!(
        read(&path)["mcp"]["servers"]["harness-memory"],
        entry("/opt/bin/harness-memory")
    );

    // An owned legacy entry follows the server where it is, keeping its other fields.
    home.write_opencode(
        "opencode.json",
        r#"{"mcp":{"harness-memory":{"type":"local","command":["/old/harness-memory","mcp"],"environment":{"A":"1"}}}}"#,
    );
    assert_eq!(
        register_memory_mcp(&file, Path::new("/new/harness-memory"), true),
        MemoryRegistration::Updated
    );
    assert_eq!(
        read(&path),
        json!({ "mcp": { "harness-memory":
            { "type": "local", "command": ["/new/harness-memory", "mcp"], "environment": { "A": "1" } } } })
    );

    assert_eq!(unregister_memory_mcp(&file), Ok(true));
    assert_eq!(read(&path), json!({ "mcp": {} }));

    home.write_opencode(
        "opencode.json",
        r#"{"mcp":{"servers":{"harness-memory":{"type":"local","command":["harness-memory","mcp"]}}}}"#,
    );
    assert_eq!(unregister_memory_mcp(&file), Ok(true));
    assert_eq!(read(&path), json!({ "mcp": { "servers": {} } }));
}

/// Turning project memory off removes the owned nested entry, but never a legacy server of the
/// same name the app does not own (a hand-written remote one with its token stays as it was).
#[test]
fn removing_the_memory_server_keeps_a_legacy_server_the_app_does_not_own() {
    let home = Home::new();
    let file = home.file();
    let token = format!("Bearer {}", "FAKE-not-a-token");
    let legacy = json!({ "type": "remote", "url": "https://team.example/mcp", "headers": { "Authorization": token } });
    let original = json!({ "mcp": {
        "harness-memory": legacy,
        "servers": { "harness-memory": { "type": "local", "command": ["/home/u/.local/bin/harness-memory", "mcp"] } },
    }});
    let path = home.write_opencode("opencode.json", &original.to_string());

    assert_eq!(unregister_memory_mcp(&file), Ok(true));

    let left = read(&path);
    assert_eq!(left["mcp"]["servers"], json!({}), "the owned nested entry is removed");
    assert_eq!(
        serde_json::to_string(&left["mcp"]["harness-memory"]).unwrap(),
        serde_json::to_string(&legacy).unwrap(),
        "the legacy server is kept byte for byte"
    );
    assert_eq!(
        memory_mcp_state(&home.env, &file),
        configured(OPENCODE_JSON, "harness-memory"),
        "OpenCode still loads it"
    );

    assert_eq!(unregister_memory_mcp(&file), Ok(false), "nothing owned is left");
    assert_eq!(read(&path), left);
}

#[test]
fn the_apps_server_is_added_nested_and_a_legacy_owned_entry_follows_the_app() {
    let home = Home::new();
    let file = home.file();
    let path = home.opencode("opencode.json");
    let app = Path::new("/opt/Harness/harness-desktop");

    assert_eq!(
        register_desktop_mcp(&file, app, false, None),
        DesktopRegistration::Added
    );
    assert_eq!(
        read(&path)["mcp"]["servers"]["harness-desktop"],
        entry("/opt/Harness/harness-desktop")
    );

    home.write_opencode(
        "opencode.json",
        r#"{"mcp":{"harness-desktop":{"type":"local","command":["/old/harness-desktop","mcp"]}}}"#,
    );
    assert_eq!(
        register_desktop_mcp(&file, app, true, None),
        DesktopRegistration::Updated
    );
    assert_eq!(
        read(&path),
        json!({ "mcp": { "harness-desktop": entry("/opt/Harness/harness-desktop") } }),
        "repointed in place, no nested copy"
    );
}
