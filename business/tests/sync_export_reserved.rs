//! Export of "Share setup" leaves out the entries the app and Recommended setup write themselves
//! (the dashboard's, the memory server's and the Harness Skills' OpenCode server), without a row,
//! in any case.

mod sync_support;

use harness_business::model::HarnessId;
use harness_business::sync::export_preview;
use serde_json::json;
use sync_support::{BOTH, Home, ids};

#[test]
fn the_dashboard_and_memory_entries_are_skipped_without_a_row() {
    let home = Home::empty();
    home.write(
        ".omp/agent/mcp.json",
        &json!({ "mcpServers": {
            "harness-memory": { "command": "/home/someone/.local/bin/harness-memory", "args": ["mcp"] },
            "Harness-Memory": { "command": "harness-memory", "args": ["mcp"] },
            "harness-dashboard": { "command": "harness-dashboard", "args": ["mcp"] },
            "docs": { "type": "http", "url": "https://docs.example.com/mcp" },
        }}),
    );

    let preview = export_preview(&home.collect(BOTH));

    assert_eq!(ids(&preview), ["omp:mcp:docs"]);
    assert!(preview.excluded.is_empty(), "{:?}", preview.excluded);
}

#[test]
fn the_skills_server_the_opencode_installer_writes_is_skipped_without_a_row() {
    let home = Home::empty();
    // As the installer writes it: `node` running the script by absolute path (a local path any
    // other server would be excluded for).
    let script = "/home/someone/.config/opencode/plugins/useful-skills/opencode/mcp-server.js";
    home.write_text(
        ".config/opencode/opencode.json",
        &format!(
            r#"{{ "mcp": {{
  "useful_skills": {{ "type": "local", "command": ["node", "{script}"], "enabled": true }},
  "Useful_Skills": {{ "type": "remote", "url": "https://skills.example.com/mcp" }},
  "docs": {{ "type": "remote", "url": "https://docs.example.com/mcp" }}
}} }}"#
        ),
    );

    let preview = export_preview(&home.collect(&[HarnessId::OpenCode]));

    assert_eq!(ids(&preview), ["opencode:mcp:docs"]);
    assert!(preview.excluded.is_empty(), "{:?}", preview.excluded);
}
