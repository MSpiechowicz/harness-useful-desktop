//! What the OpenCode 2 chat engine reports from its session against the scripted OpenCode 2 server:
//! rename, the statistics (`opencode_server_v2::stats`), the last reply and the HTML export, escaped,
//! redacted and outside the project.

mod agent_support;
mod opencode2_engine_support;

use harness_business::agent::SessionOptions;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ContextUsage, SessionStats, TokenUsage};
use harness_business::opencode_server_v2::mapper::ContextNote;
use harness_business::opencode_server_v2::messages::SessionMessage;
use harness_business::opencode_server_v2::stats::{last_context, last_reply_text, session_stats};
use harness_business::opencode_server_v2::wire::ModelRef;
use opencode2_engine_support::{SESSION, start_with};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Options that export into `dir`.
fn exporting_into(dir: &std::path::Path) -> SessionOptions {
    SessionOptions {
        export_dir: Some(dir.to_path_buf()),
        ..agent_support::options()
    }
}

#[tokio::test]
async fn rename_stats_last_reply_and_an_escaped_export_outside_the_project() {
    let exports = tempfile::tempdir().expect("export folder");
    let export_dir = exports.path().join("exports");
    let chat = start_with("opencode2_export.jsonl", None, exporting_into(&export_dir)).await;
    let session = chat.session();

    let rename = AgentAction::SetSessionName {
        name: "Renamed chat".to_owned(),
    };
    assert_eq!(session.send(rename).await.expect("rename"), AgentReply::Ack);
    chat.wait(
        "renamed state",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Renamed chat")),
    )
    .await;

    let stats = match session.send(AgentAction::GetSessionStats).await {
        Ok(AgentReply::Stats { stats }) => stats,
        other => panic!("stats: {other:?}"),
    };
    assert_eq!(
        stats,
        SessionStats {
            // The /review run: its template prompt, 8 steps with 13 tool calls; then the prompt
            // and reply with markup. The `!command` counts in the total, its synthetic context not.
            user_messages: 2,
            assistant_messages: 9,
            tool_calls: 13,
            total_messages: 2 + 9 + 13 + 1,
            // Every step's tokens and cost, as on OpenCode 1.
            tokens: TokenUsage {
                input: 16_174,
                output: 1_799 + 3_671,
                cache_read: 67_709,
                cache_write: 0,
                total: 16_174 + 1_799 + 3_671 + 67_709,
            },
            cost_usd: Some(0.125),
            context_usage: Some(ContextUsage {
                tokens: 12,
                context_window: 200_000,
                percent: 0.006,
            }),
        }
    );

    let last = session
        .send(AgentAction::GetLastAssistantText)
        .await
        .expect("last reply");
    let AgentReply::Text { text: Some(text) } = last else {
        panic!("last reply: {last:?}");
    };
    assert!(text.starts_with("<script>alert(1)</script>"), "{text}");

    let path = match session.send(AgentAction::ExportHtml).await {
        Ok(AgentReply::Exported { path }) => std::path::PathBuf::from(path),
        other => panic!("export: {other:?}"),
    };
    assert_eq!(path, export_dir.join(format!("opencode-{SESSION}.html")));
    assert!(!path.starts_with(&chat.project), "never into the project");

    let html = std::fs::read_to_string(&path).expect("the export");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>Renamed chat</title>"));
    assert!(!html.to_lowercase().contains("<script"), "{html}");
    assert!(!html.contains("\"><img"), "{html}");
    assert!(!html.contains("abc123"), "secrets are redacted");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("<details class=\"thinking\">"));
    assert!(html.contains("<h3>Result</h3>"));
    assert!(
        html.contains("$ echo hi &amp;&amp; ls"),
        "the !command is in the transcript"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(&path).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "only the user reads the transcript");

        let folder = std::fs::metadata(&export_dir).expect("metadata").permissions().mode();
        assert_eq!(folder & 0o777, 0o700, "only the user lists the exports");
    }

    assert!(session.is_running(), "every request matched the scenario");
    session.stop().await;
}

#[tokio::test]
async fn replies_never_show_the_server_password() {
    let exports = tempfile::tempdir().expect("export folder");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(exports.path(), std::fs::Permissions::from_mode(0o755)).expect("open the folder");
    }
    let chat = start_with("opencode2_export_password.jsonl", None, exporting_into(exports.path())).await;
    let session = chat.session();
    let password = chat.password();

    let last = session
        .send(AgentAction::GetLastAssistantText)
        .await
        .expect("last reply");
    let AgentReply::Text { text: Some(text) } = last else {
        panic!("last reply: {last:?}");
    };
    assert!(!text.contains(&password), "{text}");
    assert!(text.contains("••••"), "{text}");

    let path = match session.send(AgentAction::ExportHtml).await {
        Ok(AgentReply::Exported { path }) => std::path::PathBuf::from(path),
        other => panic!("export: {other:?}"),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let folder = std::fs::metadata(exports.path())
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(folder & 0o777, 0o700, "an existing exports folder is narrowed");
    }

    let html = std::fs::read_to_string(&path).expect("the export");
    assert!(!html.contains(&password), "the export holds the password");

    let output = session.bash("env").await.expect("ran");
    assert!(!output.output.contains(&password), "{output:?}");
    assert!(output.output.contains("••••"), "{output:?}");

    assert!(session.is_running(), "every request matched the scenario");
    session.stop().await;
}

fn assistant(id: &str, output: u64, text: &str) -> Value {
    json!({ "type": "assistant", "id": id, "time": { "created": 1 },
        "model": { "id": "big-pickle", "providerID": "opencode", "variant": "high" },
        "content": [{ "type": "text", "text": text }], "cost": 0.25,
        "tokens": { "input": 100, "output": output, "reasoning": 5, "cache": { "read": 7, "write": 3 } } })
}

fn parse(values: &[Value]) -> Vec<SessionMessage> {
    values.iter().map(SessionMessage::parse).collect()
}

#[test]
fn the_context_is_the_newest_reply_with_output_and_unknown_after_a_compaction() {
    let replies = parse(&[assistant("msg_1", 10, "first"), assistant("msg_2", 0, "")]);

    assert_eq!(
        last_context(&replies),
        Some(ContextNote::Tokens {
            tokens: 100 + 10 + 5 + 7 + 3,
            model: Some(ModelRef {
                id: "big-pickle".to_owned(),
                provider_id: "opencode".to_owned(),
                variant: None,
            }),
        }),
        "a step without output is skipped"
    );

    let compacted = parse(&[
        assistant("msg_1", 10, "first"),
        json!({ "type": "compaction", "id": "msg_2", "status": "completed", "reason": "manual", "summary": "Summary" }),
    ]);
    assert_eq!(last_context(&compacted), Some(ContextNote::Compacted));
    assert_eq!(last_context(&[]), None);
}

#[test]
fn totals_add_up_every_step_and_compaction_and_the_last_reply_skips_steps_without_text() {
    let messages = parse(&[
        json!({ "type": "user", "id": "msg_0", "text": "Hi" }),
        assistant("msg_1", 10, "Hello"),
        json!({ "type": "assistant", "id": "msg_2", "content": [{ "type": "reasoning", "text": "hmm" }] }),
        json!({ "type": "compaction", "id": "msg_3", "status": "completed", "reason": "manual",
            "summary": "Summary", "cost": 0.5,
            "tokens": { "input": 200, "output": 20, "reasoning": 0, "cache": { "read": 0, "write": 0 } } }),
    ]);

    let stats = session_stats(&messages, &|_| None);

    assert_eq!(
        stats.tokens,
        TokenUsage {
            input: 100 + 200,
            output: 10 + 5 + 20,
            cache_read: 7,
            cache_write: 3,
            total: 300 + 35 + 7 + 3,
        }
    );
    assert_eq!(stats.cost_usd, Some(0.75));
    assert_eq!(stats.context_usage, None, "the context is unknown after a compaction");
    assert_eq!((stats.user_messages, stats.assistant_messages), (1, 2));
    assert_eq!(last_reply_text(&messages).as_deref(), Some("Hello"));
    assert_eq!(last_reply_text(&[]), None);
}
