//! The approval modes an agent may give a routine through `harness-desktop mcp` (see "Agent rules"
//! in `docs/routines-format.md`): the tool schema names Auto, a Claude Code routine may run in it,
//! and any other harness is refused.

mod routine_support;

use harness_business::hooks_mcp::Server;
use routine_support::{Root, mcp_call, root};
use serde_json::{Value, json};

/// A daily agent routine of `harness` in `approval_mode`.
fn agent(harness: &str, approval_mode: &str) -> Value {
    json!({
        "name": "Morning review",
        "scope": "project",
        "when": "daily",
        "at": "09:00",
        "action": "agent",
        "harness": harness,
        "prompt": "List what changed since yesterday.",
        "approvalMode": approval_mode,
    })
}

/// The MCP server working in the project of `root`.
fn server(root: &Root) -> Server {
    Server::new(None, Some(root.project.clone().into())).with_routines(Some(root.store.clone()))
}

#[test]
fn the_tool_schema_offers_auto() {
    let root = root();
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
    let listed = server(&root).handle(request).expect("answered")["result"].clone();

    let tools = listed["tools"].as_array().expect("tools");
    let add = tools
        .iter()
        .find(|tool| tool["name"] == "routine_add")
        .expect("routine_add");
    let modes = &add["inputSchema"]["properties"]["approvalMode"]["enum"];

    assert!(modes.as_array().expect("an enum").contains(&json!("auto")), "{modes}");
}

#[test]
fn only_a_claude_routine_may_run_in_auto() {
    let root = root();
    let mut server = server(&root);

    let (refused, added) = mcp_call(&mut server, "routine_add", agent("claude", "auto"));
    assert!(!refused, "{added}");
    let added: Value = serde_json::from_str(&added).expect("the reply is JSON");
    assert_eq!(added["added"]["action"]["approvalMode"], "auto");

    for harness in ["omp", "opencode"] {
        let (refused, text) = mcp_call(&mut server, "routine_add", agent(harness, "auto"));

        assert!(refused, "{harness}: {text}");
        assert!(
            text.contains(r#"action.approvalMode "auto" is for claude only"#),
            "{harness}: {text}"
        );
    }

    assert_eq!(
        root.store.list().routines.len(),
        1,
        "only the Claude Code routine was written"
    );
}
