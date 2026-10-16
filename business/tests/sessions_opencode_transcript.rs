//! OpenCode transcripts through its CLI, replayed (see `opencode_session_support`): the export
//! as chat messages, paging, redaction, edited files and the project check.

mod common;
mod opencode_session_support;
mod session_support;

use std::fs;
use std::time::Duration;

use common::CANARY;
use harness_business::harness::CliSessions;
use harness_business::harnesses::opencode::OpenCodeSessions;
use harness_business::model::{ChatBlock, IpcErrorCode, MessageRole, ModelTurns};
use opencode_session_support::{Fixture, MAIN, Replay, argv, error_code, ok, opencode};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn the_export_maps_onto_chat_messages() {
    let fixture = Fixture::new();
    let runner = Replay::answering([fixture.export_output()]);

    let page = OpenCodeSessions::new()
        .messages(&fixture.ctx(fixture.project(), &runner), &opencode(MAIN), None, 200)
        .unwrap();

    let roles: Vec<MessageRole> = page.messages.iter().map(|message| message.role).collect();
    assert_eq!(
        roles,
        [
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::ToolResult,
            MessageRole::ToolResult,
            MessageRole::ToolResult,
            MessageRole::ToolResult,
            MessageRole::User,
            MessageRole::Assistant,
        ]
    );
    assert_eq!(page.total, 8);
    assert_eq!(page.next_cursor, None);

    // The attachment mentioned in the prompt is not repeated.
    let prompt = &page.messages[0];
    assert_eq!(prompt.id.as_deref(), Some("msg_a1000000001aAAAAAAAAAAAAAA"));
    assert_eq!(prompt.timestamp, Some(1_760_600_000_000));
    assert_eq!(
        prompt.blocks,
        [ChatBlock::Text {
            text: "Make the greeting say hello to the world.".to_owned()
        }]
    );

    let reply = &page.messages[1];
    assert_eq!(reply.model.as_deref(), Some("claude-sonnet-4-5"));
    assert_eq!(reply.provider.as_deref(), Some("anthropic"));
    assert_eq!(reply.stop_reason.as_deref(), Some("tool-calls"));
    assert_eq!(reply.cost_usd, Some(0.0123));
    assert_eq!(
        reply.usage.map(|usage| (usage.input, usage.cache_read, usage.total)),
        Some((1200, 800, 2440))
    );
    assert_eq!(
        reply.blocks[..2],
        [
            ChatBlock::Thinking {
                text: "The greeting lives in src/main.rs.".to_owned(),
                redacted: false
            },
            ChatBlock::Text {
                text: "I will update the greeting.".to_owned()
            },
        ]
    );

    let tools: Vec<(&str, &str, Option<&str>)> = reply.blocks[2..]
        .iter()
        .map(|block| match block {
            ChatBlock::ToolCall { id, name, intent, .. } => (id.as_str(), name.as_str(), intent.as_deref()),
            other => panic!("not a tool call: {other:?}"),
        })
        .collect();
    assert_eq!(
        tools,
        [
            ("toolu_edit_1", "edit", Some("src/main.rs")),
            ("toolu_write_1", "write", Some("notes/todo.md")),
            ("toolu_edit_2", "edit", None),
            ("toolu_patch_1", "apply_patch", Some("2 files")),
        ]
    );
    let ChatBlock::ToolCall { args, .. } = &reply.blocks[3] else {
        unreachable!()
    };
    assert_eq!(args, &json!({ "filePath": "notes/todo.md", "content": "- ship it" }));

    let text = |text: &str| ChatBlock::Text { text: text.to_owned() };
    let results: Vec<(Option<&str>, Option<bool>, &ChatBlock)> = page.messages[3..6]
        .iter()
        .map(|result| (result.tool_call_id.as_deref(), result.is_error, &result.blocks[0]))
        .collect();
    assert_eq!(
        results,
        [
            (Some("toolu_write_1"), Some(false), &text("Wrote file successfully.")),
            (
                Some("toolu_edit_2"),
                Some(true),
                &text("Permission denied: outside the project")
            ),
            (Some("toolu_patch_1"), Some(false), &text("Success.")),
        ]
    );

    // The recorded output carries a secret assignment; it is masked.
    let edited = &page.messages[2];
    let ChatBlock::Text { text: output } = &edited.blocks[0] else {
        panic!("not text: {edited:?}")
    };
    assert_eq!(edited.tool_call_id.as_deref(), Some("toolu_edit_1"));
    assert_eq!(edited.is_error, Some(false));
    assert!(output.starts_with("Edit applied successfully. API_TOKEN="), "{output}");
    assert!(!output.contains(CANARY), "{output}");
    assert_eq!(page.messages[2].tool_name.as_deref(), Some("edit"));

    assert_eq!(page.messages[6].blocks, [text("@general Check the tests still pass")]);
    assert_eq!(
        page.messages[7].blocks,
        [text("Done: the greeting now says hello, world.")]
    );
    assert_eq!(page.messages[7].stop_reason.as_deref(), Some("stop"));

    let stats = page.stats.expect("the first page has totals");
    assert_eq!(stats.messages, 4);
    assert_eq!(stats.tool_calls, 4);
    assert_eq!(stats.context_tokens, Some(3580));
    assert!((stats.cost_usd.unwrap() - 0.0163).abs() < 1e-9, "{:?}", stats.cost_usd);
    assert_eq!(
        stats.models,
        [ModelTurns {
            model: "claude-sonnet-4-5".to_owned(),
            turns: 2
        }]
    );
    assert_eq!(stats.started_at, Some(1_760_600_000_000));
    assert_eq!(stats.ended_at, Some(1_760_600_070_000));

    let spec = &runner.specs()[0];
    assert_eq!(argv(spec), ["export", MAIN, "--pure"]);
    assert_eq!(spec.timeout, Duration::from_secs(30));
    assert_eq!(spec.max_output_bytes, 16 * 1024 * 1024);
}

#[test]
fn tool_errors_and_reply_errors_are_redacted() {
    let fixture = Fixture::new();
    let secret = format!("SECRET_KEY={CANARY}-0123456789abcdef");
    let failed_reply = json!({ "name": "APIError", "data": { "message": format!("rejected {secret}") } });
    let export = fixture
        .recorded("export-main.json", fixture.project())
        .replace(
            "Permission denied: outside the project",
            &format!("denied with {secret}"),
        )
        .replace("\"finish\": \"stop\"", &format!("\"error\": {failed_reply}"));
    let runner = Replay::answering([ok(export)]);

    let page = OpenCodeSessions::new()
        .messages(&fixture.ctx(fixture.project(), &runner), &opencode(MAIN), None, 200)
        .unwrap();

    let tool_error = serde_json::to_string(&page.messages[4]).unwrap();
    assert!(tool_error.contains("denied with SECRET_KEY="), "{tool_error}");
    assert!(!tool_error.contains(CANARY), "{tool_error}");

    let reply = &page.messages[7];
    let error = reply.error_message.as_deref().expect("the reply failed");
    assert!(error.starts_with("rejected SECRET_KEY="), "{error}");
    assert!(!error.contains(CANARY), "{error}");
    assert_eq!(reply.stop_reason.as_deref(), Some("error"));
}

#[test]
fn later_pages_reuse_the_export_of_the_first() {
    let fixture = Fixture::new();
    let runner = Replay::answering([fixture.export_output()]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    let first = sessions.messages(&ctx, &opencode(MAIN), None, 3).unwrap();
    assert_eq!(first.messages.len(), 3);
    assert_eq!(first.next_cursor.as_deref(), Some("3"));
    assert!(first.stats.is_some());

    let second = sessions.messages(&ctx, &opencode(MAIN), Some("3"), 200).unwrap();
    assert_eq!(second.messages.len(), 5);
    assert_eq!(second.next_cursor, None);
    assert_eq!(second.stats, None);
    assert_eq!(second.messages[0].tool_call_id.as_deref(), Some("toolu_write_1"));

    let unknown = sessions.messages(&ctx, &opencode(MAIN), Some("99"), 10);
    assert_eq!(error_code(unknown), IpcErrorCode::InvalidInput);

    assert_eq!(runner.specs().len(), 1, "one export: {:?}", runner.argv());

    // Opening the transcript again exports again.
    runner.queue(fixture.export_output());
    sessions.messages(&ctx, &opencode(MAIN), None, 3).unwrap();
    assert_eq!(runner.specs().len(), 2);
}

#[test]
fn touched_files_are_project_relative_and_stay_inside() {
    let fixture = Fixture::new();
    let runner = Replay::answering([fixture.export_output()]);

    let touched = OpenCodeSessions::new()
        .files_touched(&fixture.ctx(fixture.project(), &runner), &opencode(MAIN))
        .unwrap();

    // `/etc/hosts` and the patch's `../outside.txt` are dropped; the patch's move counts twice.
    assert_eq!(touched, ["notes/todo.md", "src/greet.rs", "src/lib.rs", "src/main.rs"]);
}

#[test]
fn a_session_of_another_project_is_not_found() {
    let fixture = Fixture::new();
    let elsewhere = fixture.home.home.join("Projects/elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();

    // `opencode export` finds the session wherever it was started.
    let runner = Replay::answering([
        ok(fixture.recorded("export-main.json", &elsewhere)),
        ok(fixture.recorded("export-main.json", &elsewhere)),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    assert_eq!(
        error_code(sessions.messages(&ctx, &opencode(MAIN), None, 10)),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        error_code(sessions.files_touched(&ctx, &opencode(MAIN))),
        IpcErrorCode::NotFound
    );

    // An export of a different session than asked for is refused too.
    runner.queue(fixture.export_output());
    let other = "ses_2b7f3c9a1ffeOtherSession0";
    assert_eq!(
        error_code(sessions.messages(&ctx, &opencode(other), None, 10)),
        IpcErrorCode::NotFound
    );
}
