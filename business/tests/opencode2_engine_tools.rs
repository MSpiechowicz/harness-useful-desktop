//! Tool cards of the OpenCode 2 chat engine against the scripted OpenCode 2 server: a running
//! `shell` call's output streams into its card, read from the call's shell as OpenCode's own
//! clients read it, as OpenCode 1 streamed `bash` output.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use harness_business::model::AgentEvent;
use opencode2_engine_helpers::prompt;
use opencode2_engine_support::{start, until_marker};
use pretty_assertions::assert_eq;

/// The text of a tool result (`{content: [{type: text, text}]}`).
fn result_text(result: &serde_json::Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_owned()
}

#[tokio::test]
async fn a_running_shell_calls_output_streams_into_its_card() {
    let chat = start("opencode2_shell_output.jsonl").await;

    prompt(&chat, "Run make").await;
    let events = until_marker(&chat).await;

    let card: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolStart { name, .. } => Some(format!("start:{name}")),
            AgentEvent::ToolUpdate { partial, .. } => Some(format!("update:{}", result_text(partial))),
            AgentEvent::ToolEnd { result, .. } => Some(format!("end:{}", result_text(result))),
            _ => None,
        })
        .collect();
    assert_eq!(
        card,
        ["start:shell", "update:building\n", "end:building\ndone\n"],
        "the output read while the call ran shows once, before its result"
    );

    let reads = chat.requests_to("GET /api/shell/*/output");
    assert!(!reads.is_empty());
    assert!(
        reads
            .iter()
            .all(|read| read.path() == "/api/shell/sh_15d82a82d001Fd4r5Yj4H78EvM/output"),
        "only the call's shell is read"
    );
    assert!(
        chat.requests()
            .iter()
            .all(|request| !request.path().starts_with("/api/pty") && !request.path().contains("/experimental/")),
        "no PTY or experimental route"
    );
    assert!(chat.session().is_running(), "every request matched the scenario");

    chat.session().stop().await;
}
