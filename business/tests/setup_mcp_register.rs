//! Registering the dashboard's MCP server in a user MCP file: creating, merging, recognising an
//! existing entry, and refusing files it cannot change safely.

use std::path::{Path, PathBuf};

use harness_business::ecosystem::{mcp_state, register_dashboard_mcp};
use harness_business::env::Env;
use harness_business::model::{McpRefusal, McpState};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Home {
    dir: TempDir,
    env: Env,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path());
        Self { dir, env }
    }

    fn agent(&self, name: &str) -> PathBuf {
        self.dir.path().join(".omp/agent").join(name)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.agent(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn register(&self) -> McpState {
        register_dashboard_mcp(&self.env, &self.agent("mcp.json"), &[self.agent(".mcp.json")])
    }

    fn state(&self) -> McpState {
        mcp_state(&self.env, &self.agent("mcp.json"), &[self.agent(".mcp.json")])
    }

    /// Every file name in the agent directory, sorted.
    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.agent(""))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
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

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
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
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.agent("mcp.json")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn other_servers_secrets_and_keys_are_kept_in_order() {
    let home = Home::new();
    let original = json!({
        "$schema": "https://example.com/mcp.schema.json",
        "mcpServers": {
            "github": {
                "command": "github-mcp",
                "env": { "GITHUB_TOKEN": "CANARY-mcp-token-7f3a9c" }
            },
            "search": {
                "url": "https://search.example.com/mcp",
                "headers": { "Authorization": "Bearer abc.def.ghi" }
            }
        },
        "disabledServers": [],
        "zLast": { "nested": [1, 2.5, "three"] }
    });
    let path = home.write("mcp.json", &serde_json::to_string(&original).unwrap());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    }

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
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o640, "the file keeps its permissions");
    }
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
    let real = home.dir.path().join("dotfiles/mcp.json");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, "{}").unwrap();
    std::fs::create_dir_all(home.agent("")).unwrap();
    std::os::unix::fs::symlink(&real, home.agent("mcp.json")).unwrap();

    assert_eq!(home.register(), refused(McpRefusal::Symlink));
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "{}");
    assert!(
        std::fs::symlink_metadata(home.agent("mcp.json"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
