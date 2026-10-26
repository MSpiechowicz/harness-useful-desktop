//! Registering the dashboard's MCP server in a user MCP file: creating, merging, recognising an
//! existing entry, and refusing files it cannot change safely; and reading the memory server's
//! state in the same file by the same rules. OpenCode's config file is in
//! `setup_mcp_register_opencode.rs`.

use crate::mcp_support;

use harness_business::ecosystem::{McpFile, Shape, mcp_state, memory_mcp_state, register_dashboard_mcp};
use harness_business::model::{McpRefusal, McpState};
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

/// Helpers of these tests on the shared [`Home`].
trait OmpSetup {
    /// OMP's `mcp.json`, which also reads `.mcp.json`.
    fn file(&self) -> McpFile;

    fn register(&self) -> McpState;

    fn state(&self) -> McpState;

    fn memory_state(&self) -> McpState;
}

impl OmpSetup for Home {
    fn file(&self) -> McpFile {
        McpFile {
            path: self.agent("mcp.json"),
            also_read: vec![self.agent(".mcp.json")],
            shape: Shape::McpServers,
        }
    }

    fn register(&self) -> McpState {
        register_dashboard_mcp(&self.env, &self.file())
    }

    fn state(&self) -> McpState {
        mcp_state(&self.env, &self.file())
    }

    fn memory_state(&self) -> McpState {
        memory_mcp_state(&self.env, &self.file())
    }
}

const FILE: &str = "~/.omp/agent/mcp.json";

fn configured(file: &str, name: &str) -> McpState {
    McpState::Configured {
        file: file.to_owned(),
        name: name.to_owned(),
    }
}

fn refused(reason: McpRefusal) -> McpState {
    McpState::Refused {
        file: FILE.to_owned(),
        reason,
    }
}

fn dashboard_entry() -> Value {
    json!({ "command": "harness-dashboard", "args": ["mcp"] })
}

#[test]
fn a_missing_file_is_created_with_only_the_dashboard_server() {
    let home = Home::new();

    assert_eq!(
        home.state(),
        McpState::Missing {
            file: FILE.to_owned(),
            exists: false
        }
    );
    assert!(!home.agent("mcp.json").exists(), "reading the state writes nothing");

    assert_eq!(home.register(), configured(FILE, "harness-dashboard"));
    assert_eq!(
        read(&home.agent("mcp.json")),
        json!({ "mcpServers": { "harness-dashboard": dashboard_entry() } })
    );
    assert_eq!(home.files(), vec!["mcp.json"]);

    #[cfg(unix)]
    assert_eq!(mode(&home.agent("mcp.json")), 0o600);
}

#[test]
fn other_servers_secrets_and_keys_are_kept_in_order() {
    let home = Home::new();
    let (original, path) = home.write_with_secrets(json!([]));

    assert_eq!(
        home.state(),
        McpState::Missing {
            file: FILE.to_owned(),
            exists: true
        }
    );
    assert_eq!(home.register(), configured(FILE, "harness-dashboard"));

    let merged = read(&path);
    let mut expected = original.clone();
    expected["mcpServers"]["harness-dashboard"] = dashboard_entry();

    assert_eq!(merged, expected);
    assert_eq!(keys(&merged), keys(&original));
    assert_eq!(
        keys(&merged["mcpServers"]),
        vec!["github", "search", "harness-dashboard"]
    );
    assert_eq!(home.files(), vec!["mcp.json"]);

    #[cfg(unix)]
    assert_eq!(mode(&path), 0o640, "the file keeps its permissions");
}

#[test]
fn a_second_registration_leaves_the_file_unchanged() {
    let home = Home::new();
    home.write("mcp.json", r#"{ "mcpServers": { "other": { "command": "x" } } }"#);

    home.register();
    let first = std::fs::read(home.agent("mcp.json")).unwrap();

    assert_eq!(home.register(), configured(FILE, "harness-dashboard"));
    assert_eq!(std::fs::read(home.agent("mcp.json")).unwrap(), first);
}

#[test]
fn an_entry_with_the_name_is_kept_whatever_its_arguments() {
    let home = Home::new();
    let text = r#"{"mcpServers":{"harness-dashboard":{"command":"/opt/hd","args":["mcp","--db","/tmp/u.db"]}}}"#;
    home.write("mcp.json", text);

    assert_eq!(home.register(), configured(FILE, "harness-dashboard"));
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), text);
}

#[test]
fn the_dashboard_command_under_another_name_counts_as_configured() {
    let home = Home::new();
    let text = r#"{"mcpServers":{"usage":{"command":"/home/me/.local/bin/harness-dashboard","args":["mcp"]}}}"#;
    home.write("mcp.json", text);

    assert_eq!(home.register(), configured(FILE, "usage"));
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), text);
}

#[test]
fn an_entry_in_an_also_read_file_counts_and_nothing_is_created() {
    let home = Home::new();
    home.write(
        ".mcp.json",
        r#"{"mcpServers":{"dash":{"command":"harness-dashboard.exe","args":["mcp"]}}}"#,
    );

    assert_eq!(home.register(), configured("~/.omp/agent/.mcp.json", "dash"));
    assert!(!home.agent("mcp.json").exists());
}

#[test]
fn a_disabled_entry_is_reported_and_left_alone() {
    let home = Home::new();
    let listed = r#"{"mcpServers":{"harness-dashboard":{"command":"harness-dashboard","args":["mcp"]}},"disabledServers":["harness-dashboard"]}"#;
    home.write("mcp.json", listed);

    let disabled = McpState::Disabled {
        file: FILE.to_owned(),
        name: "harness-dashboard".to_owned(),
    };
    assert_eq!(home.register(), disabled);
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), listed);

    let switched_off =
        r#"{"mcpServers":{"harness-dashboard":{"command":"harness-dashboard","args":["mcp"],"enabled":false}}}"#;
    home.write("mcp.json", switched_off);

    assert_eq!(home.state(), disabled);
}

#[test]
fn a_blank_file_counts_as_an_empty_object() {
    let home = Home::new();
    home.write("mcp.json", "  \n\t\n");

    assert_eq!(home.register(), configured(FILE, "harness-dashboard"));
    assert_eq!(
        read(&home.agent("mcp.json")),
        json!({ "mcpServers": { "harness-dashboard": dashboard_entry() } })
    );
}

#[test]
fn a_file_without_servers_gets_them_after_its_other_keys() {
    let home = Home::new();
    home.write("mcp.json", r#"{"b":1,"a":{"x":true}}"#);

    assert_eq!(home.register(), configured(FILE, "harness-dashboard"));

    let merged = read(&home.agent("mcp.json"));
    assert_eq!(keys(&merged), vec!["b", "a", "mcpServers"]);
    assert_eq!(merged["mcpServers"], json!({ "harness-dashboard": dashboard_entry() }));
}

#[test]
fn files_that_cannot_be_changed_safely_are_refused_without_writing() {
    let cases = [
        (
            r#"{ "mcpServers": { "a": { "command": "x" }, }, }"#,
            McpRefusal::Unparsable,
        ),
        (r#"[{ "mcpServers": {} }]"#, McpRefusal::NotAnObject),
        (r#"{ "mcpServers": [{ "name": "a" }] }"#, McpRefusal::ServersNotAnObject),
    ];

    for (text, reason) in cases {
        let home = Home::new();
        home.write("mcp.json", text);

        assert_eq!(home.state(), refused(reason));
        assert_eq!(home.register(), refused(reason));
        assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), text);
        assert_eq!(home.files(), vec!["mcp.json"], "no temporary file is left behind");
    }
}

#[test]
fn a_directory_or_an_oversized_file_is_refused() {
    let home = Home::new();
    std::fs::create_dir_all(home.agent("mcp.json")).unwrap();
    assert_eq!(home.register(), refused(McpRefusal::NotRegularFile));

    let home = Home::new();
    let big = format!(r#"{{"pad":"{}"}}"#, "x".repeat(2 * 1024 * 1024));
    home.write("mcp.json", &big);
    assert_eq!(home.register(), refused(McpRefusal::TooLarge));
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), big);
}

#[cfg(unix)]
#[test]
fn a_failed_write_is_refused_as_such_and_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let home = Home::new();
    let path = home.write("mcp.json", "{}");
    let directory = home.agent("");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();

    // A user who may write anyway (root) cannot see the failure.
    let probe = directory.join("probe");
    if std::fs::write(&probe, "").is_ok() {
        std::fs::remove_file(&probe).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let outcome = home.register();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(outcome, refused(McpRefusal::WriteFailed));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    assert_eq!(home.files(), vec!["mcp.json"]);
    assert_eq!(
        home.state(),
        McpState::Missing {
            file: FILE.to_owned(),
            exists: true
        },
        "a failed write is not a read failure"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_refused_and_its_target_untouched() {
    let home = Home::new();
    let real = home.symlink_mcp_file();

    assert_eq!(home.register(), refused(McpRefusal::Symlink));
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "{}");
    assert!(
        std::fs::symlink_metadata(home.agent("mcp.json"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn the_memory_server_is_read_by_the_same_rules_and_apart_from_the_dashboard() {
    let home = Home::new();
    let missing = McpState::Missing {
        file: FILE.to_owned(),
        exists: true,
    };

    // Only the dashboard's server: the memory server is missing, and the other way round.
    home.write(
        "mcp.json",
        r#"{"mcpServers":{"harness-dashboard":{"command":"harness-dashboard","args":["mcp"]}}}"#,
    );
    assert_eq!(home.state(), configured(FILE, "harness-dashboard"));
    assert_eq!(home.memory_state(), missing);

    home.write(
        "mcp.json",
        r#"{"mcpServers":{"notes":{"command":"/home/me/.local/bin/harness-memory","args":["mcp"]}}}"#,
    );
    assert_eq!(home.memory_state(), configured(FILE, "notes"), "any path, any name");
    assert_eq!(home.state(), missing);

    home.write(
        "mcp.json",
        r#"{"mcpServers":{"harness-memory":{"command":"C:\\Tools\\harness-memory.exe","args":["mcp"]}},"disabledServers":["harness-memory"]}"#,
    );
    assert_eq!(
        home.memory_state(),
        McpState::Disabled {
            file: FILE.to_owned(),
            name: "harness-memory".to_owned()
        }
    );

    home.write(
        "mcp.json",
        r#"{"mcpServers":{"harness-memory":{"command":"harness-memory","args":["mcp"],"enabled":false}}}"#,
    );
    assert!(matches!(home.memory_state(), McpState::Disabled { .. }));

    // Another binary's `mcp` does not count.
    home.write(
        "mcp.json",
        r#"{"mcpServers":{"other":{"command":"/opt/harness-memory-old","args":["mcp"]}}}"#,
    );
    assert_eq!(home.memory_state(), missing);
}

#[test]
fn the_memory_server_counts_in_an_also_read_file_and_a_bad_file_is_refused() {
    let home = Home::new();
    home.write(
        ".mcp.json",
        r#"{"mcpServers":{"mem":{"command":"/usr/local/bin/harness-memory","args":["mcp"]}}}"#,
    );

    assert_eq!(home.memory_state(), configured("~/.omp/agent/.mcp.json", "mem"));
    assert_eq!(
        home.state(),
        McpState::Missing {
            file: FILE.to_owned(),
            exists: false
        }
    );

    home.write("mcp.json", "{ not json");
    assert_eq!(home.memory_state(), refused(McpRefusal::Unparsable));
    assert_eq!(home.state(), refused(McpRefusal::Unparsable));
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), "{ not json");
}
