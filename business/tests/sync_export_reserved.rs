//! Export of "Share setup" leaves out the entries the app and Recommended setup write themselves
//! (the dashboard's and the memory server's), without a row, in any case.

mod sync_support;

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
