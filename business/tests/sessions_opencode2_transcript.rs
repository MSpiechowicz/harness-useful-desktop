//! OpenCode 2 transcripts through its CLI, replayed (see `opencode_session_support`): real
//! `opencode session export --standalone` output of OpenCode 2.1.2 (see `sessions_opencode2.rs`)
//! as chat messages, edited files, redaction and the project check. `export-compaction.json` is
//! assembled from the recorded server responses of a compacted session (`GET /api/session/<id>`
//! and its message list, oldest first), as `session export` writes them.

mod common;
mod opencode_session_support;
mod session_support;

use std::fs;
use std::path::Path;

use common::CANARY;
use harness_business::harness::{CliSessions, OpenCodeGeneration};
use harness_business::harnesses::opencode::OpenCodeSessions;
use harness_business::model::{ChatBlock, ChatMessage, IpcErrorCode, MessageRole, ModelTurns, SessionMessagesPage};
use opencode_session_support::{Fixture, Replay, argv, error_code, ok, opencode};
use pretty_assertions::assert_eq;
use serde_json::json;

const EDIT: &str = "ses_ea283f985ffeX5BRVy9GzbD4gY";
const WRITE_REJECTED: &str = "ses_ea2837c64ffeCkDpkdXo15P9nS";
const READ_GLOB: &str = "ses_ea2841e42ffeZTCd1U8mJvf9FY";
const QUESTION: &str = "ses_ea27fb347ffe9TdQgsIApShsEN";
const INTERRUPTED: &str = "ses_ea27f21d1ffe637dkW2375t8zk";
const IMAGE: &str = "ses_ea27d7722ffe38bUxnEBuJHNmW";
const SHELL: &str = "ses_ea27cfa99ffe21eKppwXzXASSa";
const COMPACTED: &str = "ses_ea27dbca0ffeq7RgyVRicYAt6C";

/// A recorded OpenCode 2 fixture with `{{PROJECT}}` replaced by `project`.
fn recorded(name: &str, project: &Path) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sessions/opencode2")
        .join(name);
    let text = fs::read_to_string(path).expect("fixture");

    let quoted = serde_json::to_string(&project.to_string_lossy()).unwrap();
    text.replace("{{PROJECT}}", quoted.trim_matches('"'))
}

/// Every message of the export `stdout` of session `id`, read in the fixture's project.
fn transcript(fixture: &Fixture, id: &str, stdout: String) -> SessionMessagesPage {
    let runner = Replay::answering([ok(stdout)]);

    OpenCodeSessions::with_generation(OpenCodeGeneration::V2)
        .messages(&fixture.ctx(fixture.project(), &runner), &opencode(id), None, 200)
        .unwrap()
}

fn read(fixture: &Fixture, id: &str, name: &str) -> SessionMessagesPage {
    transcript(fixture, id, recorded(name, fixture.project()))
}

fn touched(fixture: &Fixture, id: &str, stdout: String) -> Vec<String> {
    let runner = Replay::answering([ok(stdout)]);

    OpenCodeSessions::with_generation(OpenCodeGeneration::V2)
        .files_touched(&fixture.ctx(fixture.project(), &runner), &opencode(id))
        .unwrap()
}

fn text(text: &str) -> ChatBlock {
    ChatBlock::Text { text: text.to_owned() }
}

fn tool_calls(message: &ChatMessage) -> Vec<(&str, &str)> {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::ToolCall { id, name, .. } => Some((id.as_str(), name.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn the_export_maps_onto_chat_messages() {
    let fixture = Fixture::new();
    let runner = Replay::answering([ok(recorded("export-edit.json", fixture.project()))]);

    let page = OpenCodeSessions::with_generation(OpenCodeGeneration::V2)
        .messages(&fixture.ctx(fixture.project(), &runner), &opencode(EDIT), None, 200)
        .unwrap();

    use MessageRole::{Assistant, ToolResult, User};
    let roles: Vec<MessageRole> = page.messages.iter().map(|message| message.role).collect();
    assert_eq!(
        roles,
        [
            User, Assistant, ToolResult, Assistant, ToolResult, Assistant, ToolResult, Assistant, ToolResult, Assistant
        ]
    );
    assert_eq!(page.total, 10);

    let prompt = &page.messages[0];
    assert_eq!(prompt.id.as_deref(), Some("msg_15d7c06a6001mllm55mom3aBzP"));
    assert_eq!(prompt.timestamp, Some(1_792_569_771_927));
    assert_eq!(
        prompt.blocks,
        [text(
            "Append the line 'recorded' to notes.txt using the edit tool. Then run the shell command `ls` with the shell tool. Reply done."
        )]
    );

    let first = &page.messages[1];
    assert_eq!(first.model.as_deref(), Some("big-pickle"));
    assert_eq!(first.provider.as_deref(), Some("opencode"));
    assert_eq!(first.stop_reason.as_deref(), Some("tool-calls"));
    assert_eq!(first.cost_usd, Some(0.0));
    assert_eq!(
        first
            .usage
            .map(|usage| (usage.input, usage.output, usage.cache_read, usage.total)),
        Some((3517, 96, 1906, 5519))
    );
    assert_eq!(
        first.blocks[0],
        text("I'll check the current state of notes.txt first, then make the edit.")
    );
    let ChatBlock::ToolCall { args, intent, .. } = &first.blocks[1] else {
        panic!("not a tool call: {:?}", first.blocks[1])
    };
    assert_eq!(
        args,
        &json!({ "path": fixture.project().join("notes.txt").to_string_lossy() })
    );
    assert_eq!(intent, &None);

    let edit = &page.messages[3];
    assert!(
        matches!(&edit.blocks[0], ChatBlock::Thinking { text, redacted: false } if text.starts_with("Now I need to append"))
    );
    assert_eq!(tool_calls(edit), [("call_01a15d7c388975ddb41ea7f1", "edit")]);

    let result = |index: usize| {
        let result = &page.messages[index];
        (
            result.tool_call_id.as_deref(),
            result.tool_name.as_deref(),
            result.is_error,
            result.blocks.clone(),
        )
    };
    assert_eq!(
        result(4),
        (
            Some("call_01a15d7c388975ddb41ea7f1"),
            Some("edit"),
            Some(false),
            vec![text("Edited notes.txt (1 replacement)")]
        )
    );
    assert_eq!(
        result(6),
        (
            Some("call_01a15d7c540c72e7b51933da"),
            Some("shell"),
            Some(false),
            vec![text("dot.png\nnotes.txt\n")]
        )
    );
    assert_eq!(page.messages[4].timestamp, Some(1_792_569_784_887));

    let last = &page.messages[9];
    assert_eq!(
        last.blocks,
        [text(
            "done- Appended `recorded` to `notes.txt` (now: `first line` / `recorded`).\n- Ran `ls`: `dot.png`, `notes.txt`."
        )]
    );
    assert_eq!(last.stop_reason.as_deref(), Some("stop"));

    let stats = page.stats.expect("the first page has totals");
    assert_eq!(stats.messages, 6);
    assert_eq!(stats.tool_calls, 4);
    assert_eq!(
        stats.models,
        [ModelTurns {
            model: "big-pickle".to_owned(),
            turns: 5
        }]
    );
    assert_eq!(stats.started_at, Some(1_792_569_771_927));
    assert_eq!(stats.ended_at, Some(1_792_569_796_692));

    assert_eq!(argv(&runner.specs()[0]), ["session", "export", "--standalone", EDIT]);
}

#[test]
fn edited_files_are_project_relative_and_stay_inside() {
    let fixture = Fixture::new();

    // `edit` and `write` name their file in `path`; reads and shell calls touch nothing.
    assert_eq!(
        touched(&fixture, EDIT, recorded("export-edit.json", fixture.project())),
        ["notes.txt"]
    );
    assert!(
        touched(
            &fixture,
            READ_GLOB,
            recorded("export-read-glob.json", fixture.project())
        )
        .is_empty()
    );

    // As with OpenCode 1, a call counts whatever its outcome: this write was rejected.
    assert_eq!(
        touched(
            &fixture,
            WRITE_REJECTED,
            recorded("export-write-rejected.json", fixture.project())
        ),
        ["notes.txt"]
    );
}

#[test]
fn patch_headers_name_the_edited_files() {
    let fixture = Fixture::new();
    let patch = "*** Begin Patch\n*** Add File: src/new.rs\n+fn main() {}\n*** Update File: notes/todo.md\n*** Move to: notes/done.md\n@@\n-a\n+b\n*** Delete File: ../outside.txt\n*** End Patch";

    // The recorded edit call, rewritten as the `patch` call OpenCode 2 makes for the same change.
    let export = recorded("export-edit.json", fixture.project());
    let mut value: serde_json::Value = serde_json::from_str(&export).unwrap();
    let call = value
        .pointer_mut("/messages/2/content/1")
        .expect("the recorded edit call");
    call["name"] = json!("patch");
    call["state"]["input"] = json!({ "patchText": patch });

    let files = touched(&fixture, EDIT, value.to_string());

    // `../outside.txt` lies outside the project; a move names both files.
    assert_eq!(files, ["notes/done.md", "notes/todo.md", "src/new.rs"]);
}

#[test]
fn tools_questions_and_reads_keep_their_outputs() {
    let fixture = Fixture::new();

    let page = read(&fixture, QUESTION, "export-question.json");
    let ask = &page.messages[1];
    assert_eq!(tool_calls(ask), [("call_596aed8f43a64bc28ceac00f", "question")]);
    let answer = &page.messages[2];
    assert_eq!(answer.role, MessageRole::ToolResult);
    assert!(
        matches!(&answer.blocks[0], ChatBlock::Text { text } if text.contains("=\"blue\"")),
        "{answer:?}"
    );
    assert_eq!(page.messages[3].blocks, [text("You chose **blue**.")]);

    let page = read(&fixture, READ_GLOB, "export-read-glob.json");
    let reads = &page.messages[1];
    assert_eq!(
        tool_calls(reads),
        [
            ("call_function_ergyosjl9cx3_1", "read"),
            ("call_function_ergyosjl9cx3_2", "glob")
        ]
    );
    assert_eq!(
        page.messages[2].blocks,
        [text("Read file notes.txt, lines 1-1\n1: first line")]
    );
}

#[test]
fn rejected_and_interrupted_work_reads_as_errors() {
    let fixture = Fixture::new();

    let page = read(&fixture, WRITE_REJECTED, "export-write-rejected.json");
    let rejected = &page.messages[2];
    assert_eq!(rejected.tool_name.as_deref(), Some("write"));
    assert_eq!(rejected.is_error, Some(true));
    assert_eq!(rejected.blocks, [text("Do not change that file.")]);

    let page = read(&fixture, INTERRUPTED, "export-interrupted.json");
    let reply = &page.messages[1];
    assert_eq!(reply.stop_reason.as_deref(), Some("aborted"));
    assert_eq!(reply.error_message.as_deref(), Some("Step interrupted"));
    assert_eq!(reply.usage, None);
    let ChatBlock::ToolCall { args, .. } = &reply.blocks[1] else {
        panic!("not a tool call: {:?}", reply.blocks[1])
    };
    assert_eq!(args, &json!({}));
    let interrupted = &page.messages[2];
    assert_eq!(interrupted.is_error, Some(true));
    assert_eq!(interrupted.blocks, [text("Tool execution interrupted")]);
}

#[test]
fn images_commands_and_compactions_are_shown() {
    let fixture = Fixture::new();

    // The image is named by its type only; its data never reaches the transcript.
    let page = read(&fixture, IMAGE, "export-image.json");
    assert_eq!(
        page.messages[0].blocks,
        [
            text("What colour is this image? One word."),
            ChatBlock::Image {
                mime_type: "image/png".to_owned()
            }
        ]
    );
    let serialized = serde_json::to_string(&page.messages).unwrap();
    assert!(!serialized.contains("iVBORw0KGgo"), "the image data leaked");

    // A `!command` sent while the reply ran; the reply ends with reasoning and the interruption.
    let page = read(&fixture, SHELL, "export-shell.json");
    let command = page
        .messages
        .iter()
        .find(|message| message.role == MessageRole::Bash)
        .expect("the command");
    assert_eq!(command.blocks, [text("$ echo busy\nbusy\n")]);
    assert_eq!(command.timestamp, Some(1_792_570_350_420));
    let last = page.messages.last().unwrap();
    assert_eq!(last.role, MessageRole::Bash);
    let stopped = page
        .messages
        .iter()
        .rfind(|message| message.role == MessageRole::Assistant)
        .unwrap();
    assert_eq!(stopped.stop_reason.as_deref(), Some("aborted"));
    assert!(
        matches!(stopped.blocks[..], [ChatBlock::Thinking { .. }]),
        "{stopped:?}"
    );

    let page = read(&fixture, COMPACTED, "export-compaction.json");
    let roles: Vec<MessageRole> = page.messages.iter().map(|message| message.role).collect();
    use MessageRole::{Assistant, CompactionSummary, User};
    assert_eq!(roles, [User, Assistant, User, Assistant, CompactionSummary]);
    let summary = &page.messages[4];
    assert!(
        matches!(&summary.blocks[..], [ChatBlock::Text { text }] if text.starts_with("## Objective")),
        "{summary:?}"
    );
}

#[test]
fn tool_outputs_and_reply_errors_are_redacted() {
    let fixture = Fixture::new();
    let secret = format!("API_KEY={CANARY}-0123456789abcdef");
    let export = recorded("export-edit.json", fixture.project())
        .replace("dot.png\\nnotes.txt\\n", &format!("printed {secret}"))
        .replace(
            "\"finish\": \"stop\"",
            &format!("\"error\": {{\"type\": \"provider\", \"message\": \"rejected {secret}\"}}"),
        );

    let page = transcript(&fixture, EDIT, export);

    let output = serde_json::to_string(&page.messages[6]).unwrap();
    assert!(output.contains("printed API_KEY="), "{output}");
    assert!(!output.contains(CANARY), "{output}");

    let reply = &page.messages[9];
    let error = reply.error_message.as_deref().expect("the reply failed");
    assert!(error.starts_with("rejected API_KEY="), "{error}");
    assert!(!error.contains(CANARY), "{error}");
    assert_eq!(reply.stop_reason.as_deref(), Some("error"));
}

#[test]
fn a_session_of_another_project_is_not_found() {
    let fixture = Fixture::new();
    let elsewhere = fixture.home.home.join("Projects/elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();

    // The export finds the session wherever it was started.
    let runner = Replay::answering([
        ok(recorded("export-edit.json", &elsewhere)),
        ok(recorded("export-edit.json", &elsewhere)),
        ok(recorded("export-edit.json", fixture.project())),
        ok("not json"),
    ]);
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::with_generation(OpenCodeGeneration::V2);

    assert_eq!(
        error_code(sessions.messages(&ctx, &opencode(EDIT), None, 10)),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        error_code(sessions.files_touched(&ctx, &opencode(EDIT))),
        IpcErrorCode::NotFound
    );

    // An export of a different session than asked for is refused too.
    assert_eq!(
        error_code(sessions.messages(&ctx, &opencode(QUESTION), None, 10)),
        IpcErrorCode::NotFound
    );

    assert_eq!(
        error_code(sessions.messages(&ctx, &opencode(QUESTION), None, 10)),
        IpcErrorCode::Protocol
    );
}
