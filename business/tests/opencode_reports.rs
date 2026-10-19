//! What the OpenCode chat reports from a session's messages: the statistics, the context meter and
//! the last reply (`opencode_server::stats`, against the recorded `opencode serve` messages), and
//! the HTML export (`opencode_server::export_html`), escaped and redacted.

mod opencode_mapper_support;

use harness_business::model::{ChatBlock, ChatMessage, ContextUsage, MessageRole, SessionStats, TokenUsage};
use harness_business::opencode_server::export_html::{ExportPage, escape, render};
use harness_business::opencode_server::stats::{
    ContextMark, context_mark, last_context, last_reply_text, session_stats, usage,
};
use harness_business::opencode_server::wire::ModelRef;
use opencode_mapper_support::recorded_body;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The recorded `GET /session/{id}/message` records.
fn recorded_messages() -> Vec<Value> {
    recorded_body("messages.json")
        .as_array()
        .cloned()
        .expect("a message list")
}

/// The context window of the recorded mock model.
fn mock_window(model: &ModelRef) -> Option<u64> {
    (model.provider == "mock" && model.id == "m1").then_some(100_000)
}

#[test]
fn stats_count_the_shown_messages_and_add_up_tokens_and_cost() {
    let stats = session_stats(&recorded_messages(), &mock_window);

    assert_eq!(
        stats,
        SessionStats {
            // The `!command`'s user message is OpenCode's own context.
            user_messages: 8,
            assistant_messages: 11,
            tool_calls: 4,
            total_messages: 8 + 11 + 4,
            tokens: TokenUsage {
                input: 300,
                output: 52,
                cache_read: 0,
                cache_write: 0,
                total: 352,
            },
            cost_usd: Some(0.0),
            context_usage: Some(ContextUsage {
                tokens: 25,
                context_window: 100_000,
                percent: 0.025,
            }),
        }
    );
}

#[test]
fn reasoning_counts_as_output_and_costs_add_up() {
    let reply = |input: u64, output: u64, reasoning: u64, cost: f64| {
        json!({ "info": { "id": "msg_1", "role": "assistant", "providerID": "mock", "modelID": "m1",
            "cost": cost, "tokens": { "input": input, "output": output, "reasoning": reasoning,
            "cache": { "read": 7, "write": 3 } } }, "parts": [] })
    };

    let stats = session_stats(&[reply(100, 10, 5, 0.25), reply(200, 20, 0, 0.5)], &|_| None);

    assert_eq!(
        stats.tokens,
        TokenUsage {
            input: 300,
            output: 35,
            cache_read: 14,
            cache_write: 6,
            total: 355,
        }
    );
    assert_eq!(stats.cost_usd, Some(0.75));
    assert_eq!(stats.context_usage, None, "an unknown model has no window");
    assert_eq!(stats.assistant_messages, 0, "replies without content are not messages");
}

#[test]
fn the_context_is_the_newest_reply_with_output_and_unknown_after_a_compaction() {
    let records = recorded_messages();

    assert_eq!(
        last_context(&records),
        Some(ContextMark::Tokens {
            tokens: 25,
            model: Some(ModelRef {
                provider: "mock".to_owned(),
                id: "m1".to_owned()
            }),
        }),
        "the `!command`'s reply has no output and is skipped"
    );

    let summary = json!({ "role": "assistant", "summary": true, "tokens": { "input": 900, "output": 40 } });
    assert_eq!(context_mark(&summary), Some(ContextMark::Compacted));
    assert_eq!(context_mark(&json!({ "role": "user" })), None);

    assert_eq!(usage(50_000, Some(200_000)).map(|usage| usage.percent), Some(25.0));
    assert_eq!(usage(10, None), None);
}

#[test]
fn the_last_reply_is_the_newest_assistant_text() {
    assert_eq!(
        last_reply_text(&recorded_messages()).as_deref(),
        Some("Hello from the mock model."),
        "the `!command`'s reply holds only its tool call"
    );
    assert_eq!(last_reply_text(&[]), None);
}

fn message(role: MessageRole, blocks: Vec<ChatBlock>) -> ChatMessage {
    ChatMessage {
        id: Some("msg_1".to_owned()),
        role,
        blocks,
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: Some(1_792_438_561_716),
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    }
}

fn text(text: &str) -> ChatBlock {
    ChatBlock::Text { text: text.to_owned() }
}

#[test]
fn the_export_escapes_and_redacts_everything_from_the_session() {
    let hostile = "<script>alert(1)</script>\"><img src=x onerror=alert(2)>' onmouseover='x";
    let secret = ["hf", "abcdefghijklmnopqrstuvwxyz"].join("_");

    let mut reply = message(
        MessageRole::Assistant,
        vec![
            ChatBlock::Thinking {
                text: format!("thinking {hostile}"),
                redacted: false,
            },
            text(&format!("reply {hostile} with {secret}")),
            ChatBlock::ToolCall {
                id: "call_1".to_owned(),
                name: format!("bash{hostile}"),
                args: json!({ "command": format!("echo {hostile}"), "token": secret }),
                intent: Some(hostile.to_owned()),
            },
        ],
    );
    reply.model = Some(format!("m1{hostile}"));
    reply.error_message = Some(hostile.to_owned());

    let mut result = message(MessageRole::ToolResult, vec![text(&format!("out {hostile}"))]);
    result.tool_call_id = Some("call_1".to_owned());
    result.is_error = Some(true);

    let messages = [message(MessageRole::User, vec![text(hostile)]), reply, result];
    let page = ExportPage {
        title: hostile,
        harness: "OpenCode",
        session_id: "ses_1",
        exported_at: "2026-10-19T20:00:00Z",
    };
    let html = render(&page, &messages);
    let lower = html.to_lowercase();

    assert!(!lower.contains("<script"), "{html}");
    assert!(!lower.contains("<img"), "{html}");
    assert!(!html.contains("\"><img"), "no attribute breakout");
    assert!(!html.contains("' onmouseover='"), "no single-quote breakout");
    assert!(!html.contains(&secret), "secrets are redacted");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;&quot;&gt;&lt;img src=x onerror=alert(2)&gt;&#39;"));
    assert!(
        html.contains("default-src 'none'; style-src 'unsafe-inline'"),
        "the page may load and run nothing"
    );
    assert_eq!(
        html.matches("<details class=\"tool failed\">").count(),
        1,
        "the result sits with its call"
    );
    assert_eq!(
        html.matches("<section").count(),
        2,
        "the paired result has no section of its own"
    );
    assert!(html.contains("<time>2026-10-19T19:36:01Z</time>"));
    assert!(html.contains("<details class=\"thinking\"><summary>Thinking</summary>"));
}

#[test]
fn escape_covers_text_and_attribute_characters() {
    assert_eq!(escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&#39;f");
}
