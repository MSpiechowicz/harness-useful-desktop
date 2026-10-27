//! Export of "Share setup": MCP arguments that hold a path of this machine. An `--option=/path` is
//! dropped alone; a path on its own goes with the `--option` before it only when that is a long
//! option named like a path (`--image-dir`, `--config-file`, ...); any other machine path (a script
//! a program runs, a positional path, a path after a flag that takes none or a short option) leaves
//! the whole server out, since dropping it would change what the server does.

use crate::sync_support;

use harness_business::model::{SyncExcludedReason, SyncNote};
use harness_business::sync::export_preview;
use pretty_assertions::assert_eq;
use serde_json::json;
use sync_support::{BOTH, Home, ids};

#[test]
fn machine_paths_in_mcp_args_are_dropped_and_in_urls_leave_the_server_out() {
    let home = Home::empty();
    home.write(
        ".omp/agent/mcp.json",
        &json!({ "mcpServers": {
            "root-arg": { "command": "server", "args": ["--root=/home/alice/proj"] },
            "split-arg": { "command": "npx", "args": ["-y", "figma-mcp", "--stdio", "--image-dir", "/Users/alice/Downloads"] },
            "equals-arg": { "command": "npx", "args": ["figma-mcp", "--stdio", "--image-dir=/Users/alice/Downloads"] },
            "path-url": { "type": "http", "url": "https://mcp.example.com/mcp?root=/home/alice" },
            "fine": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github", "--flag=value"] },
            "fine-url": { "type": "http", "url": "https://mcp.example.com/home/mcp" },
        }}),
    );

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(
        ids(&preview),
        [
            "omp:mcp:equals-arg",
            "omp:mcp:fine",
            "omp:mcp:fine-url",
            "omp:mcp:root-arg",
            "omp:mcp:split-arg"
        ]
    );

    let item = |id: &str| {
        preview
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .find(|item| item.id == id)
            .expect(id)
    };
    assert_eq!(
        item("omp:mcp:split-arg").detail.as_deref(),
        Some("npx -y figma-mcp --stdio")
    );
    assert_eq!(
        item("omp:mcp:equals-arg").detail.as_deref(),
        Some("npx figma-mcp --stdio"),
        "an option with its own value never takes the one before it along"
    );
    assert!(item("omp:mcp:split-arg").notes.contains(&SyncNote::PathArgsNotShared));
    assert!(item("omp:mcp:equals-arg").notes.contains(&SyncNote::PathArgsNotShared));
    assert!(!item("omp:mcp:fine").notes.contains(&SyncNote::PathArgsNotShared));

    assert_eq!(preview.excluded.len(), 1);
    assert_eq!(preview.excluded[0].label, "path-url");
}

#[test]
fn a_path_that_cannot_be_dropped_properly_leaves_the_server_out() {
    let home = Home::empty();
    home.write(
        ".omp/agent/mcp.json",
        &json!({ "mcpServers": {
            "boolean-flag": { "command": "server", "args": ["--read-only", "/home/alice/data"] },
            "script": { "command": "node", "args": ["/home/alice/server.js"] },
            "short-option": { "command": "server", "args": ["-d", "/home/alice/data"] },
            "positional": { "command": "server", "args": ["serve", "/home/alice/data"] },
            "key-value": { "command": "server", "args": ["DATA=/home/alice/data"] },
            "short-equals": { "command": "server", "args": ["-v=/home/alice/data"] },
            "kept": { "command": "server", "args": ["--stdio", "--config", "/home/alice/c.json", "--quiet"] },
        }}),
    );

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(ids(&preview), ["omp:mcp:kept"]);

    let mut left_out: Vec<(&str, SyncExcludedReason)> = preview
        .excluded
        .iter()
        .map(|excluded| (excluded.label.as_str(), excluded.reason))
        .collect();
    left_out.sort_by_key(|(label, _)| *label);

    assert_eq!(
        left_out,
        [
            ("boolean-flag", SyncExcludedReason::LocalPath),
            ("key-value", SyncExcludedReason::LocalPath),
            ("positional", SyncExcludedReason::LocalPath),
            ("script", SyncExcludedReason::LocalPath),
            ("short-equals", SyncExcludedReason::LocalPath),
            ("short-option", SyncExcludedReason::LocalPath),
        ]
    );
    assert_eq!(
        preview.sections[0].items[0].detail.as_deref(),
        Some("server --stdio --quiet")
    );
}

#[test]
fn opencode_and_codex_follow_the_same_rules() {
    let home = Home::empty();
    home.write(
        ".config/opencode/opencode.json",
        &json!({ "mcp": {
            "kept": { "type": "local", "command": ["server", "--stdio", "--image-dir=/home/alice/x"] },
            "script": { "type": "local", "command": ["node", "/home/alice/server.js"] },
        }}),
    );
    home.write_text(
        ".codex/config.toml",
        "[mcp_servers.kept]\ncommand = \"server\"\nargs = [\"--stdio\", \"--log-dir\", \"/home/alice/logs\"]\n\n\
         [mcp_servers.script]\ncommand = \"node\"\nargs = [\"/home/alice/server.js\"]\n",
    );

    let preview = export_preview(&home.collect(&[
        harness_business::model::HarnessId::OpenCode,
        harness_business::model::HarnessId::Codex,
    ]));

    assert_eq!(ids(&preview), ["opencode:mcp:kept", "codex:mcp:kept"]);
    assert_eq!(preview.excluded.len(), 2);
    assert!(
        preview
            .excluded
            .iter()
            .all(|excluded| excluded.label == "script" && excluded.reason == SyncExcludedReason::LocalPath)
    );
    for section in &preview.sections {
        assert_eq!(section.items[0].detail.as_deref(), Some("server --stdio"));
    }
}
