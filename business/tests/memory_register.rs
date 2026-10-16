//! Registering the installed memory server in a user MCP file: adding it by absolute path, keeping
//! a removed one out, following the resolved server's path, leaving the user's entries alone, and
//! refusing files it cannot change safely. The same in OpenCode's config file (`command` arrays),
//! where a file with comments is never written.

mod mcp_support;

use std::path::PathBuf;

use harness_business::ecosystem::{
    Blocked, McpFile, MemoryRegistration, Shape, register_memory_mcp, unregister_memory_mcp,
};
use harness_business::model::McpRefusal;
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

impl Home {
    /// Where the server is installed in these tests.
    fn binary(&self) -> PathBuf {
        self.dir.path().join(".local/bin/harness-memory")
    }

    /// OMP's `mcp.json`, which also reads `.mcp.json`.
    fn file(&self) -> McpFile {
        McpFile {
            also_read: vec![self.agent(".mcp.json")],
            ..mcp_json(self.agent("mcp.json"))
        }
    }

    fn register(&self, recorded: bool) -> MemoryRegistration {
        register_memory_mcp(&self.file(), &self.binary(), recorded)
    }

    fn text(&self) -> String {
        std::fs::read_to_string(self.agent("mcp.json")).unwrap()
    }
}

/// `path` as a `mcp.json` with nothing else to read.
fn mcp_json(path: PathBuf) -> McpFile {
    McpFile {
        path,
        also_read: Vec::new(),
        shape: Shape::McpServers,
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
        unregister_memory_mcp(&home.file()),
        Err(Blocked::Refused(McpRefusal::Unparsable))
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

    assert_eq!(unregister_memory_mcp(&mcp_json(path.clone())), Ok(true));

    let removed = read(&path);
    assert_eq!(keys(&removed["mcpServers"]), vec!["github", "search"]);
    assert_eq!(
        removed["disabledServers"],
        json!(["harness-memory"]),
        "the lists are left alone"
    );
    assert_eq!(keys(&removed), keys(&original));

    assert_eq!(unregister_memory_mcp(&mcp_json(path.clone())), Ok(false));
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

        assert_eq!(unregister_memory_mcp(&home.file()), Ok(false));
        assert_eq!(home.text(), text);
    }

    assert_eq!(unregister_memory_mcp(&mcp_json(home.agent("missing.json"))), Ok(false));
    assert!(!home.agent("missing.json").exists());
}

impl Home {
    /// OpenCode's `opencode.json` in `~/.config/opencode`.
    fn opencode_file(&self) -> McpFile {
        McpFile {
            path: self.dir.path().join(".config/opencode/opencode.json"),
            also_read: Vec::new(),
            shape: Shape::OpenCode,
        }
    }

    fn write_opencode(&self, text: &str) -> PathBuf {
        let path = self.opencode_file().path;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn register_opencode(&self, recorded: bool) -> MemoryRegistration {
        register_memory_mcp(&self.opencode_file(), &self.binary(), recorded)
    }
}

fn opencode_entry(command: &str) -> Value {
    json!({ "type": "local", "command": [command, "mcp"], "enabled": true })
}

#[test]
fn opencode_gets_a_local_server_with_a_command_array() {
    let home = Home::new();
    let path = home.write_opencode(
        r#"{"$schema":"https://opencode.ai/config.json","mcp":{"other":{"type":"remote","url":"https://x"}}}"#,
    );

    assert_eq!(home.register_opencode(false), MemoryRegistration::Added);

    let command = home.binary().to_string_lossy().into_owned();
    let written = read(&path);
    assert_eq!(keys(&written), vec!["$schema", "mcp"]);
    assert_eq!(keys(&written["mcp"]), vec!["other", "harness-memory"]);
    assert_eq!(written["mcp"]["harness-memory"], opencode_entry(&command));

    assert_eq!(home.register_opencode(false), MemoryRegistration::Present);
}

#[test]
fn an_owned_opencode_entry_with_an_old_path_only_changes_its_program() {
    let home = Home::new();
    let original = json!({
        "mcp": {
            "harness-memory": {
                "type": "local",
                "command": ["/old/place/harness-memory", "mcp"],
                "environment": { "HM_LOG": "debug" },
                "enabled": true
            }
        }
    });
    let path = home.write_opencode(&original.to_string());

    assert_eq!(home.register_opencode(false), MemoryRegistration::Updated);

    let mut expected = original.clone();
    expected["mcp"]["harness-memory"]["command"][0] = json!(home.binary().to_string_lossy());
    let updated = read(&path);

    assert_eq!(updated, expected);
    assert_eq!(
        keys(&updated["mcp"]["harness-memory"]),
        vec!["type", "command", "environment", "enabled"]
    );

    // Turning project memory off removes it.
    assert_eq!(unregister_memory_mcp(&home.opencode_file()), Ok(true));
    assert_eq!(read(&path), json!({ "mcp": {} }));
}

#[test]
fn opencode_entries_the_app_does_not_own_are_never_changed() {
    let home = Home::new();

    for text in [
        r#"{"mcp":{"memory":{"type":"local","command":["/opt/harness-memory","mcp"]}}}"#,
        r#"{"mcp":{"harness-memory":{"type":"local","command":["/opt/harness-memory","mcp","--db","x"]}}}"#,
        r#"{"mcp":{"harness-memory":{"type":"remote","url":"http://localhost:7000/mcp"}}}"#,
    ] {
        home.write_opencode(text);

        assert_eq!(home.register_opencode(false), MemoryRegistration::Present);
        assert_eq!(unregister_memory_mcp(&home.opencode_file()), Ok(false));
        assert_eq!(std::fs::read_to_string(home.opencode_file().path).unwrap(), text);
    }

    let disabled = r#"{"mcp":{"harness-memory":{"enabled":false}}}"#;
    home.write_opencode(disabled);
    assert_eq!(home.register_opencode(false), MemoryRegistration::Disabled);
    assert_eq!(std::fs::read_to_string(home.opencode_file().path).unwrap(), disabled);
}

#[test]
fn a_removed_opencode_entry_stays_removed() {
    let home = Home::new();
    let text = r#"{"mcp":{}}"#;
    let path = home.write_opencode(text);

    assert_eq!(home.register_opencode(true), MemoryRegistration::SkippedRemoved);
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}

#[test]
fn an_opencode_file_with_only_trailing_commas_is_rewritten() {
    let home = Home::new();
    let path = home.write_opencode(r#"{ "$schema": "https://opencode.ai/config.json", }"#);

    assert_eq!(home.register_opencode(false), MemoryRegistration::Added);

    let command = home.binary().to_string_lossy().into_owned();
    assert_eq!(
        read(&path),
        json!({ "$schema": "https://opencode.ai/config.json", "mcp": { "harness-memory": opencode_entry(&command) } })
    );
}

#[test]
fn an_opencode_file_with_comments_is_never_written() {
    let home = Home::new();
    let command = home.binary().to_string_lossy().into_owned();

    let cases = [
        "{\n  // mine\n  \"mcp\": {}\n}".to_owned(),
        // An owned entry with an old path is not re-pointed either.
        r#"{ "mcp": { "harness-memory": { "type": "local", "command": ["/old/harness-memory", "mcp"] } } } // app"#
            .to_owned(),
    ];

    for text in cases {
        let path = home.write_opencode(&text);

        assert_eq!(home.register_opencode(false), MemoryRegistration::ManualEdit, "{text}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "unchanged byte for byte");
    }

    // An up-to-date entry in such a file counts, and cannot be removed.
    let text = format!(
        "{{ /* app */ \"mcp\": {{ \"harness-memory\": {} }} }}",
        opencode_entry(&command)
    );
    let path = home.write_opencode(&text);

    assert_eq!(home.register_opencode(false), MemoryRegistration::Present);
    assert_eq!(unregister_memory_mcp(&home.opencode_file()), Err(Blocked::ManualEdit));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}
