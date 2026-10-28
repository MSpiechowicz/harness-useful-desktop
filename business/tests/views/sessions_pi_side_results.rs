//! OMP transcripts keep the result of a tool call on the active path when the result entry sits on
//! a side branch (a replayed tool batch or a foreign writer's entries), without duplicates.

use crate::session_support;

use std::fs;

use harness_business::harness::Harness;
use harness_business::harnesses::Omp;
use harness_business::model::{ChatBlock, ChatMessage, HarnessId, MessageRole, SessionMessagesPage};
use pretty_assertions::assert_eq;
use session_support::{session, session_home};

const SESSION: &str = "0199f000-0000-7000-8000-000000000042";

fn user(id: &str, parent: Option<&str>) -> String {
    let parent = parent.map_or_else(|| "null".to_owned(), |parent| format!("\"{parent}\""));
    format!(
        r#"{{"type":"message","id":"{id}","parentId":{parent},"timestamp":"2026-10-10T09:00:00.000Z","message":{{"role":"user","content":"Read /home/demo/src/lib.rs","timestamp":1791622800000}}}}"#
    )
}

fn assistant(id: &str, parent: &str, calls: &[&str]) -> String {
    let blocks: Vec<String> = calls
        .iter()
        .map(|call| {
            format!(
                r#"{{"type":"toolCall","id":"{call}","name":"read","arguments":{{"path":"/home/demo/src/lib.rs"}}}}"#
            )
        })
        .collect();
    let blocks: String = blocks.iter().map(|block| format!(",{block}")).collect();
    format!(
        r#"{{"type":"message","id":"{id}","parentId":"{parent}","timestamp":"2026-10-10T09:01:00.000Z","message":{{"role":"assistant","content":[{{"type":"text","text":"Reading."}}{blocks}],"stopReason":"toolUse","timestamp":1791622860000}}}}"#
    )
}

fn result(id: &str, parent: &str, call: &str) -> String {
    format!(
        r#"{{"type":"message","id":"{id}","parentId":"{parent}","timestamp":"2026-10-10T09:02:00.000Z","message":{{"role":"toolResult","content":[{{"type":"text","text":"output of {id}"}}],"toolCallId":"{call}","toolName":"read","isError":false,"timestamp":1791622920000}}}}"#
    )
}

/// Reads a session made of `entries` (the last one is the leaf).
fn read(entries: &[String], limit: u32, cursor: Option<&str>) -> SessionMessagesPage {
    let home = session_home();
    let dir = Omp.sessions_dir(&home.env, &home.project);
    fs::create_dir_all(&dir).unwrap();

    let header = format!(
        r#"{{"type":"session","version":3,"id":"{SESSION}","timestamp":"2026-10-10T09:00:00.000Z","cwd":"/home/demo"}}"#
    );
    let text = format!("{header}\n{}\n", entries.join("\n"));
    fs::write(dir.join(format!("2026-10-10T09-00-00-000Z_{SESSION}.jsonl")), text).unwrap();

    Omp.session_messages(
        &home.env,
        &home.project,
        &session(HarnessId::Omp, SESSION),
        cursor,
        limit,
    )
    .unwrap()
}

fn ids(page: &SessionMessagesPage) -> Vec<&str> {
    page.messages
        .iter()
        .filter_map(|message| message.id.as_deref())
        .collect()
}

fn calls(messages: &[ChatMessage]) -> Vec<&str> {
    messages
        .iter()
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            ChatBlock::ToolCall { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect()
}

fn answers<'a>(messages: &'a [ChatMessage], call: &str) -> Vec<&'a str> {
    messages
        .iter()
        .filter(|message| message.role == MessageRole::ToolResult && message.tool_call_id.as_deref() == Some(call))
        .filter_map(|message| message.id.as_deref())
        .collect()
}

/// u1 -> a1 (calls c1, c2) -> r1 -> a2 (call c3) -> r3 -> a3, with the result of c2 on a side
/// branch of a1, a second result of c3 on a side branch of a2, and a side branch with its own
/// call c4 and result.
fn tree() -> Vec<String> {
    vec![
        user("u1", None),
        assistant("s1", "u1", &["c4"]),
        result("r4", "s1", "c4"),
        assistant("a1", "u1", &["c1", "c2"]),
        result("r1", "a1", "c1"),
        result("r2", "a1", "c2"),
        assistant("a2", "r1", &["c3"]),
        result("r3dup", "a2", "c3"),
        result("r3", "a2", "c3"),
        assistant("a3", "r3", &[]),
    ]
}

#[test]
fn a_result_on_a_side_branch_still_answers_its_call_on_the_path() {
    let page = read(&tree(), 200, None);

    assert_eq!(ids(&page), ["u1", "a1", "r2", "r1", "a2", "r3", "a3"]);
    assert_eq!(page.total, 7);

    for call in calls(&page.messages) {
        assert_eq!(answers(&page.messages, call).len(), 1, "one result for {call}");
    }
    assert_eq!(answers(&page.messages, "c2"), ["r2"], "from the side branch");
    assert_eq!(answers(&page.messages, "c3"), ["r3"], "the path's result wins");
    assert!(
        answers(&page.messages, "c4").is_empty(),
        "a call off the path stays off"
    );

    let stats = page.stats.expect("the first page carries stats");
    assert_eq!(stats.tool_calls, 3);
    assert_eq!(stats.messages, 4, "the totals cover the path only");
}

#[test]
fn pages_cut_through_attached_results_without_losing_or_repeating_one() {
    let entries = tree();
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let page = read(&entries, 2, cursor.as_deref());
        seen.extend(ids(&page).into_iter().map(str::to_owned));

        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    assert_eq!(seen, ["u1", "a1", "r2", "r1", "a2", "r3", "a3"]);
}
