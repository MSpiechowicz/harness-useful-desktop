//! The `harness-memory` executable driven by the app's own MCP client: the database location,
//! sharing between processes, and the command line.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

use harness_business::app_dirs::APP_IDENTIFIER;
use harness_business::ecosystem::{McpClient, McpOptions};
use harness_business::process::{ExecSpec, TokioSpawner};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};
use tempfile::TempDir;

const BINARY: &str = env!("CARGO_BIN_EXE_harness-memory");

struct Machine {
    _root: TempDir,
    home: PathBuf,
    data: PathBuf,
    repo: PathBuf,
    plain: PathBuf,
}

fn machine() -> Machine {
    let root = TempDir::new().expect("temp");
    let home = root.path().join("home");
    let data = root.path().join("data");
    let repo = root.path().join("work/widget");
    let plain = root.path().join("work/plain");

    for dir in [&home, &data, &repo, &plain] {
        std::fs::create_dir_all(dir).expect("mkdir");
    }

    for args in [
        &["init", "-q"][..],
        &["remote", "add", "origin", "https://github.com/acme/widget.git"][..],
    ] {
        let status = Command::new("git").args(args).current_dir(&repo).status().expect("git");
        assert!(status.success());
    }

    Machine {
        _root: root,
        home,
        data,
        repo,
        plain,
    }
}

async fn connect(machine: &Machine, cwd: &Path) -> McpClient {
    let spec = ExecSpec::new(BINARY)
        .args(["mcp"])
        .cwd(cwd)
        .env("HOME", &machine.home)
        .env("XDG_DATA_HOME", &machine.data);

    McpClient::connect(&TokioSpawner, &spec, McpOptions::default())
        .await
        .expect("the server starts and answers initialize")
}

fn arguments(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("object")
}

async fn call(client: &mut McpClient, name: &str, args: Value) -> String {
    let output = client.call_tool(name, arguments(args)).await.expect("tools/call");
    let text = output.text.expect("text");
    assert!(!output.is_error, "{name}: {text}");
    text
}

#[tokio::test]
async fn notes_persist_across_processes_in_the_app_data_folder() {
    let machine = machine();

    let mut first = connect(&machine, &machine.repo).await;
    assert_eq!(first.server_name(), Some("harness-memory"));
    assert_eq!(first.list_tools().await.expect("tools").len(), 7);

    let created = call(
        &mut first,
        "memory_remember",
        json!({ "title": "Uploader location", "body": "src/upload.rs holds the uploader", "kind": "location" }),
    )
    .await;
    assert_eq!(
        created,
        r#"created note 1 (repo "git:github.com/acme/widget", revision 1)"#
    );

    let found = call(&mut first, "memory_search", json!({ "query": "uploader" })).await;
    assert!(found.contains("Uploader location"), "{found}");
    first.shutdown().await;

    let database = machine.data.join(APP_IDENTIFIER).join("memory").join("memory.sqlite3");
    assert!(database.is_file(), "{}", database.display());
    assert!(!machine.repo.join("memory.sqlite3").exists());

    // A second server outside the repository finds the note when told which repository it is.
    let mut second = connect(&machine, &machine.plain).await;
    let unrelated = call(&mut second, "memory_search", json!({ "query": "uploader" })).await;
    assert_eq!(unrelated, "no matching notes");

    let repo_path = machine.repo.to_str().expect("utf-8");
    let shared = call(
        &mut second,
        "memory_search",
        json!({ "query": "uploader", "project_path": repo_path }),
    )
    .await;
    assert!(shared.contains("#1 [location, repo,"), "{shared}");
    second.shutdown().await;
}

#[test]
fn the_command_line_accepts_only_mcp_and_version() {
    let version = Command::new(BINARY).arg("--version").output().expect("run");
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("harness-memory {}", env!("CARGO_PKG_VERSION"))
    );

    let wrong = Command::new(BINARY).args(["--db", "/tmp/x"]).output().expect("run");
    assert_eq!(wrong.status.code(), Some(2));
    assert!(wrong.stdout.is_empty());
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("usage: harness-memory"));
}
