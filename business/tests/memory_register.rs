//! Registering the bundled memory server in a user MCP file: adding it by absolute path, keeping a
//! removed one out, following the stable copy's path, leaving the user's entries alone, and
//! refusing files it cannot change safely.

mod mcp_support;

use std::path::PathBuf;

use harness_business::ecosystem::{MemoryRegistration, register_memory_mcp, unregister_memory_mcp};
use harness_business::model::McpRefusal;
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

impl Home {
    /// Where the stable copy is installed in these tests.
    fn binary(&self) -> PathBuf {
        self.dir.path().join(".local/bin/harness-memory")
    }

    fn register(&self, recorded: bool) -> MemoryRegistration {
        register_memory_mcp(
            &self.agent("mcp.json"),
            &[self.agent(".mcp.json")],
            &self.binary(),
            recorded,
        )
    }

    fn text(&self) -> String {
        std::fs::read_to_string(self.agent("mcp.json")).unwrap()
    }
}

fn entry(command: &str) -> Value {
    json!({ "command": command, "args": ["mcp"] })
}

#[test]
fn a_missing_file_gets_the_server_by_absolute_path() {
    let home = Home::new();

    assert_eq!(home.register(false), MemoryRegistration::Added);

    let command = home.binary().to_string_lossy().into_owned();
    assert_eq!(
        read(&home.agent("mcp.json")),
        json!({ "mcpServers": { "harness-memory": entry(&command) } })
    );
    assert_eq!(home.files(), vec!["mcp.json"]);

    #[cfg(unix)]
    assert_eq!(mode(&home.agent("mcp.json")), 0o600);

    assert_eq!(
        home.register(false),
        MemoryRegistration::Present,
        "a second run changes nothing"
    );
}

#[test]
fn a_server_the_user_removed_stays_removed() {
    let home = Home::new();
    let text = r#"{"mcpServers":{"other":{"command":"x"}}}"#;
    home.write("mcp.json", text);

    assert_eq!(home.register(true), MemoryRegistration::SkippedRemoved);
    assert_eq!(home.text(), text);
}

#[test]
fn the_server_under_another_name_counts_as_present() {
    let home = Home::new();
    let text = r#"{"mcpServers":{"memory":{"command":"/opt/old/harness-memory","args":["mcp"]}}}"#;
    home.write("mcp.json", text);

    assert_eq!(home.register(false), MemoryRegistration::Present);
    assert_eq!(home.text(), text, "an entry the app does not own is never changed");
}

#[test]
fn an_owned_entry_with_an_old_path_only_changes_its_command() {
    let home = Home::new();
    let (mut original, path) = home.write_with_secrets(json!([]));
    original["mcpServers"]["harness-memory"] = json!({
        "command": "/old/place/harness-memory",
        "env": { "HM_LOG": "debug" },
        "args": ["mcp"],
        "enabled": false,
        "timeout": 30
    });
    std::fs::write(&path, serde_json::to_string(&original).unwrap()).unwrap();

    assert_eq!(home.register(false), MemoryRegistration::Updated);

    let updated = read(&path);
    let mut expected = original.clone();
    expected["mcpServers"]["harness-memory"]["command"] = json!(home.binary().to_string_lossy());

    assert_eq!(updated, expected);
    assert_eq!(keys(&updated), keys(&original));
    assert_eq!(
        keys(&updated["mcpServers"]["harness-memory"]),
        vec!["command", "env", "args", "enabled", "timeout"]
    );

    #[cfg(unix)]
    assert_eq!(mode(&path), 0o640, "the file keeps its permissions");

    assert_eq!(home.register(false), MemoryRegistration::Disabled, "still switched off");
}

#[test]
fn a_stale_entry_in_an_also_read_file_is_left_alone() {
    let home = Home::new();
    let text = r#"{"mcpServers":{"harness-memory":{"command":"/old/harness-memory","args":["mcp"]}}}"#;
    home.write(".mcp.json", text);

    assert_eq!(home.register(false), MemoryRegistration::Present);
    assert_eq!(std::fs::read_to_string(home.agent(".mcp.json")).unwrap(), text);
    assert!(!home.agent("mcp.json").exists());
}

#[test]
fn a_disabled_entry_is_reported_and_left_alone() {
    let home = Home::new();
    let command = home.binary().to_string_lossy().into_owned();
    let text = json!({
        "mcpServers": { "harness-memory": entry(&command) },
        "disabledServers": ["harness-memory"]
    })
    .to_string();
    home.write("mcp.json", &text);

    assert_eq!(home.register(false), MemoryRegistration::Disabled);
    assert_eq!(home.text(), text);
}

#[test]
fn files_that_cannot_be_changed_safely_are_refused_without_writing() {
    let home = Home::new();
    let text = r#"{ "mcpServers": { "a": { "command": "x" }, }, }"#;
    home.write("mcp.json", text);

    assert_eq!(
        home.register(false),
        MemoryRegistration::Refused(McpRefusal::Unparsable)
    );
    assert_eq!(
        unregister_memory_mcp(&home.agent("mcp.json")),
        Err(McpRefusal::Unparsable)
    );
    assert_eq!(home.text(), text);
    assert_eq!(home.files(), vec!["mcp.json"]);
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_refused_and_its_target_untouched() {
    let home = Home::new();
    let real = home.symlink_mcp_file();

    assert_eq!(home.register(false), MemoryRegistration::Refused(McpRefusal::Symlink));
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "{}");
}

#[test]
fn unregistering_removes_only_the_owned_entry() {
    let home = Home::new();
    let (mut original, path) = home.write_with_secrets(json!(["harness-memory"]));
    original["mcpServers"]["harness-memory"] = entry("/somewhere/harness-memory.exe");
    std::fs::write(&path, serde_json::to_string(&original).unwrap()).unwrap();

    assert_eq!(unregister_memory_mcp(&path), Ok(true));

    let removed = read(&path);
    assert_eq!(keys(&removed["mcpServers"]), vec!["github", "search"]);
    assert_eq!(
        removed["disabledServers"],
        json!(["harness-memory"]),
        "the lists are left alone"
    );
    assert_eq!(keys(&removed), keys(&original));

    assert_eq!(unregister_memory_mcp(&path), Ok(false));
}

#[test]
fn entries_the_app_does_not_own_are_never_unregistered() {
    let home = Home::new();

    for text in [
        r#"{"mcpServers":{"memory":{"command":"harness-memory","args":["mcp"]}}}"#,
        r#"{"mcpServers":{"harness-memory":{"command":"harness-memory","args":["mcp","--db","x"]}}}"#,
        r#"{"mcpServers":{"harness-memory":{"command":"other-memory","args":["mcp"]}}}"#,
    ] {
        home.write("mcp.json", text);

        assert_eq!(unregister_memory_mcp(&home.agent("mcp.json")), Ok(false));
        assert_eq!(home.text(), text);
    }

    assert_eq!(unregister_memory_mcp(&home.agent("missing.json")), Ok(false));
    assert!(!home.agent("missing.json").exists());
}
