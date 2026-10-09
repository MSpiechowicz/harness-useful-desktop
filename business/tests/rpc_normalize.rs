//! Normalization of harness payloads into the chat DTOs, and touched-path extraction.

use harness_business::model::{ChatBlock, MessageRole, ModelOption};
use harness_business::pi_family::normalize::{chat_message, model_option, session_state, session_stats, tool_result};
use harness_business::pi_family::touched::touched_paths;
use pretty_assertions::assert_eq;
use serde_json::json;

const SECRET: &str = "CANARY-7f3a9c";

#[test]
fn models_keep_display_fields_only() {
    let raw = json!({
        "id": "gpt-5", "name": "GPT-5", "api": "openai-responses", "provider": "openai",
        "baseUrl": format!("https://user:{SECRET}@proxy.example"), "reasoning": true, "input": ["text"],
        "cost": { "input": 1 }, "contextWindow": 400000, "maxTokens": null,
        "headers": { "Authorization": SECRET }, "compat": { "key": SECRET }, "thinking": { "mode": "x" }
    });

    let model = model_option(&raw).unwrap();

    assert_eq!(
        model,
        ModelOption {
            provider: "openai".to_owned(),
            id: "gpt-5".to_owned(),
            name: Some("GPT-5".to_owned()),
            reasoning: true,
            context_window: Some(400_000),
            max_tokens: None,
            input: vec!["text".to_owned()],
        }
    );
    assert!(model_option(&json!({ "id": "x" })).is_none());
}

#[test]
fn state_and_stats_map_both_harness_spellings() {
    let omp = session_state(&json!({
        "sessionId": "s1", "sessionFile": "/secret/path.jsonl", "isStreaming": true, "messageCount": 3,
        "queuedMessageCount": 1, "contextUsage": { "tokens": 100, "contextWindow": 1000, "percent": 10 }
    }));
    assert_eq!(
        (omp.session_id.as_str(), omp.is_streaming, omp.queued_count),
        ("s1", true, 1)
    );
    assert_eq!(omp.context_usage.as_ref().unwrap().percent, 10.0);
    assert!(!serde_json::to_string(&omp).unwrap().contains("/secret/path"));

    let pi = session_state(&json!({ "sessionId": "p1", "pendingMessageCount": 4 }));
    assert_eq!(pi.queued_count, 4);

    let stats = session_stats(&json!({
        "userMessages": 2, "assistantMessages": 2, "toolCalls": 1, "totalMessages": 5,
        "tokens": { "input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4, "total": 10 }, "cost": 0.5,
        "contextUsage": { "tokens": null, "contextWindow": 1000, "percent": null }
    }));
    assert_eq!((stats.tokens.total, stats.cost_usd), (10, Some(0.5)));
    assert_eq!(stats.context_usage, None, "unknown usage right after compaction");
}

#[test]
fn messages_normalize_per_role_without_payloads() {
    let assistant = chat_message(&json!({
        "role": "assistant", "provider": "anthropic", "model": "m", "stopReason": "error", "errorMessage": "429",
        "content": [
            { "type": "thinking", "thinking": "hmm", "thinkingSignature": SECRET },
            { "type": "redactedThinking", "data": SECRET },
            { "type": "toolCall", "id": "c1", "name": "read", "arguments": { "path": "a" }, "intent": "Read a" },
            { "type": "fallback", "from": {}, "to": {} }
        ],
        "usage": { "input": 1, "output": 2, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 3, "cost": { "total": 0.25 } },
        "providerPayload": { "secret": SECRET }, "timestamp": 1_700_000_000_000_i64
    }));
    assert_eq!(assistant.blocks.len(), 4);
    assert_eq!(
        assistant.blocks[1],
        ChatBlock::Thinking {
            text: String::new(),
            redacted: true
        }
    );
    assert_eq!(
        assistant.blocks[3],
        ChatBlock::Other {
            kind: "fallback".to_owned()
        }
    );
    assert_eq!(
        (assistant.cost_usd, assistant.error_message.as_deref()),
        (Some(0.25), Some("429"))
    );
    assert!(!serde_json::to_string(&assistant).unwrap().contains(SECRET));

    let tool = chat_message(&json!({
        "role": "toolResult", "toolCallId": "c1", "toolName": "read", "isError": true,
        "content": [{ "type": "text", "text": "nope" }, { "type": "image", "data": SECRET, "mimeType": "image/jpeg" }],
        "details": { "diff": SECRET }
    }));
    assert_eq!((tool.role, tool.is_error), (MessageRole::ToolResult, Some(true)));
    assert_eq!(
        tool.blocks[1],
        ChatBlock::Image {
            mime_type: "image/jpeg".to_owned()
        }
    );
    assert!(!serde_json::to_string(&tool).unwrap().contains(SECRET));

    let bash = chat_message(&json!({ "role": "bashExecution", "command": "ls", "output": "a\n", "cancelled": false }));
    assert_eq!(
        bash.blocks,
        [ChatBlock::Text {
            text: "$ ls\na\n".to_owned()
        }]
    );

    let summary = chat_message(&json!({ "role": "compactionSummary", "summary": "earlier work", "tokensBefore": 9 }));
    assert_eq!(summary.role, MessageRole::CompactionSummary);

    let odd = chat_message(&json!({ "role": "pythonExecution", "code": "print(1)" }));
    assert_eq!(odd.role, MessageRole::Other);
}

#[test]
fn tool_results_keep_content_blocks_and_drop_details() {
    let result = tool_result(Some(&json!({
        "content": [{ "type": "text", "text": "ok" }, { "type": "image", "data": SECRET, "mimeType": "image/png" }],
        "details": { "secret": SECRET }, "isError": false
    })));

    assert_eq!(
        result,
        json!({ "content": [{ "type": "text", "text": "ok" }, { "type": "image", "mimeType": "image/png" }] })
    );
    assert_eq!(
        tool_result(Some(&json!("plain"))),
        json!({ "content": [{ "type": "text", "text": "plain" }] })
    );
    assert_eq!(tool_result(None), json!({ "content": [] }));
}

#[test]
fn touched_paths_are_project_relative_and_contained() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "").unwrap();
    let absolute = root.join("src/deep/new.rs").to_string_lossy().into_owned();

    let edit =
        json!({ "path": "src/lib.rs", "moveTo": "./src/../src/deep/moved.rs", "edits": [{ "path": "ignored.rs" }] });
    assert_eq!(touched_paths(root, "edit", &edit), ["src/lib.rs", "src/deep/moved.rs"]);
    assert_eq!(
        touched_paths(root, "Write", &json!({ "file_path": absolute })),
        ["src/deep/new.rs"]
    );

    let outside = [
        json!({ "path": "../escape.rs" }),
        json!({ "path": "/etc/passwd" }),
        json!({ "path": "missing-dir/new.rs" }),
        json!({ "path": "" }),
        json!({ "path": 42 }),
    ];
    for args in outside {
        assert!(touched_paths(root, "write", &args).is_empty(), "{args}");
    }

    assert!(touched_paths(root, "read", &json!({ "path": "src/lib.rs" })).is_empty());
    assert_eq!(
        touched_paths(root, "notebook_edit", &json!({ "path": "src/lib.rs" })),
        ["src/lib.rs"]
    );
}

#[cfg(unix)]
#[test]
fn touched_paths_do_not_follow_symlinks_out_of_the_project() {
    let project = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join("link")).unwrap();

    assert!(touched_paths(project.path(), "write", &json!({ "path": "link/new.rs" })).is_empty());
}
