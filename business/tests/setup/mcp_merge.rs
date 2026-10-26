//! Merging server entries into a user MCP file: several at once, never overwriting a name, keeping
//! everything else, refusing files it cannot change safely, and reading one entry back.

use crate::mcp_support;

use std::path::PathBuf;

use harness_business::ecosystem::{MergeOutcome, MergeResult, MergedServer, existing_server, merge_servers};
use harness_business::model::McpRefusal;
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

/// Helpers of these tests on the shared [`Home`].
trait Merging {
    fn file(&self) -> PathBuf;
    fn merge(&self, entries: &[(&str, Value)]) -> MergeOutcome;
}

impl Merging for Home {
    fn file(&self) -> PathBuf {
        self.agent("mcp.json")
    }

    fn merge(&self, entries: &[(&str, Value)]) -> MergeOutcome {
        let entries: Vec<(String, Value)> = entries
            .iter()
            .map(|(name, definition)| ((*name).to_owned(), definition.clone()))
            .collect();
        merge_servers(&self.env, &self.file(), &entries)
    }
}

fn merged(results: &[(&str, MergeResult)]) -> MergeOutcome {
    MergeOutcome::Merged(
        results
            .iter()
            .map(|(name, result)| MergedServer {
                name: (*name).to_owned(),
                result: *result,
            })
            .collect(),
    )
}

fn stdio() -> Value {
    json!({ "type": "stdio", "command": "uvx", "args": ["mcp-server-fetch"] })
}

fn remote() -> Value {
    json!({ "type": "http", "url": "https://mcp.example.com/mcp" })
}

#[test]
fn several_entries_are_merged_in_one_write_after_the_existing_servers() {
    let home = Home::new();
    let (original, path) = home.write_with_secrets(json!(["search"]));

    let outcome = home.merge(&[("fetch", stdio()), ("remote", remote())]);

    assert_eq!(
        outcome,
        merged(&[("fetch", MergeResult::Added), ("remote", MergeResult::Added)])
    );

    let written = read(&path);
    let mut expected = original.clone();
    expected["mcpServers"]["fetch"] = stdio();
    expected["mcpServers"]["remote"] = remote();

    assert_eq!(written, expected, "secrets and other keys are untouched");
    assert_eq!(keys(&written), keys(&original));
    assert_eq!(
        keys(&written["mcpServers"]),
        vec!["github", "search", "fetch", "remote"]
    );
    assert_eq!(home.files(), vec!["mcp.json"]);

    #[cfg(unix)]
    assert_eq!(mode(&path), 0o640, "the file keeps its permissions");
}

#[test]
fn a_missing_file_is_created_with_mode_0600() {
    let home = Home::new();

    assert_eq!(
        home.merge(&[("fetch", stdio())]),
        merged(&[("fetch", MergeResult::Added)])
    );
    assert_eq!(read(&home.file()), json!({ "mcpServers": { "fetch": stdio() } }));
    assert_eq!(home.files(), vec!["mcp.json"]);

    #[cfg(unix)]
    assert_eq!(mode(&home.file()), 0o600);
}

#[test]
fn an_existing_name_is_never_overwritten() {
    let home = Home::new();
    let text = r#"{"mcpServers":{"fetch":{"type":"stdio","command":"uvx","args":["mcp-server-fetch"]},"remote":{"url":"https://other.example.com/mcp"}}}"#;
    home.write("mcp.json", text);

    let outcome = home.merge(&[("fetch", stdio()), ("remote", remote())]);

    assert_eq!(
        outcome,
        merged(&[
            ("fetch", MergeResult::AlreadyPresent),
            ("remote", MergeResult::Conflict)
        ])
    );
    assert_eq!(
        std::fs::read_to_string(home.file()).unwrap(),
        text,
        "nothing is written"
    );
    assert_eq!(home.files(), vec!["mcp.json"]);
}

#[test]
fn a_conflict_does_not_stop_the_other_entries() {
    let home = Home::new();
    home.write(
        "mcp.json",
        r#"{"mcpServers":{"remote":{"url":"https://other.example.com/mcp"}}}"#,
    );

    let outcome = home.merge(&[("remote", remote()), ("fetch", stdio())]);

    assert_eq!(
        outcome,
        merged(&[("remote", MergeResult::Conflict), ("fetch", MergeResult::Added)])
    );
    assert_eq!(
        read(&home.file()),
        json!({ "mcpServers": {
            "remote": { "url": "https://other.example.com/mcp" },
            "fetch": stdio()
        } })
    );
}

#[test]
fn a_disabled_entry_is_written_as_given() {
    let home = Home::new();
    let disabled = json!({ "type": "stdio", "command": "needs-key", "args": [], "enabled": false });

    assert_eq!(
        home.merge(&[("keyed", disabled.clone())]),
        merged(&[("keyed", MergeResult::Added)])
    );
    assert_eq!(read(&home.file())["mcpServers"]["keyed"], disabled);
}

#[test]
fn a_second_identical_merge_writes_nothing() {
    let home = Home::new();
    home.write("mcp.json", r#"{ "mcpServers": { "other": { "command": "x" } } }"#);

    home.merge(&[("fetch", stdio()), ("remote", remote())]);
    let first = std::fs::read(home.file()).unwrap();
    let modified = std::fs::metadata(home.file()).unwrap().modified().unwrap();

    assert_eq!(
        home.merge(&[("fetch", stdio()), ("remote", remote())]),
        merged(&[
            ("fetch", MergeResult::AlreadyPresent),
            ("remote", MergeResult::AlreadyPresent)
        ])
    );
    assert_eq!(std::fs::read(home.file()).unwrap(), first);
    assert_eq!(std::fs::metadata(home.file()).unwrap().modified().unwrap(), modified);
}

#[test]
fn a_name_given_twice_is_compared_with_its_first_occurrence() {
    let home = Home::new();

    assert_eq!(
        home.merge(&[("fetch", stdio()), ("fetch", stdio()), ("fetch", remote())]),
        merged(&[
            ("fetch", MergeResult::Added),
            ("fetch", MergeResult::AlreadyPresent),
            ("fetch", MergeResult::Conflict)
        ])
    );
    assert_eq!(read(&home.file()), json!({ "mcpServers": { "fetch": stdio() } }));
}

#[test]
fn files_that_cannot_be_changed_safely_are_refused_without_writing() {
    let big = format!(r#"{{"pad":"{}"}}"#, "x".repeat(2 * 1024 * 1024));
    let cases = [
        (
            r#"{ "mcpServers": { "a": { "command": "x" }, }, }"#.to_owned(),
            McpRefusal::Unparsable,
        ),
        (r#"[{ "mcpServers": {} }]"#.to_owned(), McpRefusal::NotAnObject),
        (
            r#"{ "mcpServers": [{ "name": "a" }] }"#.to_owned(),
            McpRefusal::ServersNotAnObject,
        ),
        (big, McpRefusal::TooLarge),
    ];

    for (text, reason) in cases {
        let home = Home::new();
        home.write("mcp.json", &text);

        assert_eq!(home.merge(&[("fetch", stdio())]), MergeOutcome::Refused(reason));
        assert_eq!(existing_server(&home.env, &home.file(), "fetch"), Err(reason));
        assert_eq!(std::fs::read_to_string(home.file()).unwrap(), text);
        assert_eq!(home.files(), vec!["mcp.json"], "no temporary file is left behind");
    }
}

#[test]
fn a_directory_is_refused() {
    let home = Home::new();
    std::fs::create_dir_all(home.file()).unwrap();

    assert_eq!(
        home.merge(&[("fetch", stdio())]),
        MergeOutcome::Refused(McpRefusal::NotRegularFile)
    );
    assert!(home.file().is_dir());
    assert_eq!(home.files(), vec!["mcp.json"]);
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_refused_and_its_target_untouched() {
    let home = Home::new();
    let real = home.symlink_mcp_file();

    assert_eq!(
        home.merge(&[("fetch", stdio())]),
        MergeOutcome::Refused(McpRefusal::Symlink)
    );
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "{}");
    assert!(std::fs::symlink_metadata(home.file()).unwrap().file_type().is_symlink());
    assert_eq!(home.files(), vec!["mcp.json"]);
}

#[test]
fn existing_server_returns_the_named_definition_only() {
    let home = Home::new();

    assert_eq!(existing_server(&home.env, &home.file(), "fetch"), Ok(None), "no file");

    home.write(
        "mcp.json",
        r#"{"mcpServers":{"fetch":{"command":"uvx","env":{"KEY":"v"}}}}"#,
    );

    assert_eq!(
        existing_server(&home.env, &home.file(), "fetch"),
        Ok(Some(json!({ "command": "uvx", "env": { "KEY": "v" } })))
    );
    assert_eq!(existing_server(&home.env, &home.file(), "other"), Ok(None));

    home.write("mcp.json", r#"{"other":1}"#);
    assert_eq!(
        existing_server(&home.env, &home.file(), "fetch"),
        Ok(None),
        "no mcpServers"
    );
}
