//! Synthetic saved-session fixtures (`sessions/`): one OMP session with two branch points, its
//! transcript and touched files, plus short OMP and PI session lists.

use harness_business::model::{
    ChatBlock, ChatMessage, HarnessId, MessageRole, SessionMessagesPage, SessionNode, SessionNodeKind, SessionRef,
    SessionSummary, SessionTitleSource, SessionTree, TokenUsage,
};
use serde_json::{Value, json};

use super::to_json;
use crate::common::DEMO_PROJECT;

/// The exported OMP session (the one the chat fixtures run in).
const OMP_SESSION: &str = "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";

const MODEL: &str = "claude-sonnet-4-5";

fn session_ref(harness: HarnessId, id: &str) -> SessionRef {
    SessionRef {
        harness,
        id: id.to_owned(),
    }
}

fn summary(
    harness: HarnessId,
    id: &str,
    title: Option<(&str, SessionTitleSource)>,
    first_prompt: &str,
) -> SessionSummary {
    SessionSummary {
        session: session_ref(harness, id),
        title: title.map(|(text, _)| text.to_owned()),
        title_source: title.map(|(_, source)| source),
        cwd: DEMO_PROJECT.to_owned(),
        created_at: String::new(),
        updated_at: String::new(),
        size_bytes: 0,
        model: Some(MODEL.to_owned()),
        first_prompt: Some(first_prompt.to_owned()),
        parent_session: None,
    }
}

/// `summary` with its file times and size.
fn dated(summary: SessionSummary, created_at: &str, updated_at: &str, size_bytes: u64) -> SessionSummary {
    SessionSummary {
        created_at: created_at.to_owned(),
        updated_at: updated_at.to_owned(),
        size_bytes,
        ..summary
    }
}

/// Newest first: the current session, an untitled fork of it and an older user-named one.
fn omp_sessions() -> Vec<SessionSummary> {
    let current = summary(
        HarnessId::Omp,
        OMP_SESSION,
        Some(("Handle empty input in parse", SessionTitleSource::Auto)),
        "Make `parse` handle empty input and explain the change.",
    );

    let fork = SessionSummary {
        parent_session: Some(OMP_SESSION.to_owned()),
        ..summary(
            HarnessId::Omp,
            "0199c3a8-0b1e-7c42-8d5f-1a2b3c4d5e6f",
            None,
            "Why does `cargo test` hang on CI when the parser reads from stdin?",
        )
    };

    let release = SessionSummary {
        model: Some("gpt-5".to_owned()),
        ..summary(
            HarnessId::Omp,
            "01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3",
            Some(("Release checklist", SessionTitleSource::User)),
            "Draft the release checklist for 0.3.1.",
        )
    };

    vec![
        dated(fork, "2026-10-09T11:02:41Z", "2026-10-09T11:20:05Z", 12_874),
        dated(current, "2026-10-09T10:00:00Z", "2026-10-09T10:17:30Z", 48_213),
        dated(release, "2026-10-07T15:30:12Z", "2026-10-07T16:05:48Z", 203_557),
    ]
}

/// A user-named PI session and an untitled one (the first prompt is the fallback title).
fn pi_sessions() -> Vec<SessionSummary> {
    let named = summary(
        HarnessId::Pi,
        "8c2d4e6f-1a3b-4c5d-8e7f-9a0b1c2d3e4f",
        Some(("Streaming parser spike", SessionTitleSource::User)),
        "Sketch a streaming variant of `parse` that reads from any `BufRead`.",
    );
    let untitled = summary(
        HarnessId::Pi,
        "5b7c9d1e-3f4a-4b6c-8d0e-2f4a6b8c0d1e",
        None,
        "Which edition does the crate use?",
    );

    vec![
        dated(named, "2026-10-08T09:12:00Z", "2026-10-08T09:47:31Z", 31_090),
        dated(untitled, "2026-10-06T18:40:22Z", "2026-10-06T18:41:10Z", 4_512),
    ]
}

/// One session entry: `(id, parent, kind, text)`; the text is the message preview.
type Entry = (
    &'static str,
    Option<&'static str>,
    SessionNodeKind,
    Option<&'static str>,
);

/// Every entry of the OMP session in file order. `n05` and `n12` are branch points; the active
/// path ends at `n18`.
fn entries() -> Vec<Entry> {
    use SessionNodeKind::{Assistant, BranchSummary, ModelChange, ToolResult, User};

    vec![
        ("n01", None, ModelChange, None),
        (
            "n02",
            Some("n01"),
            User,
            Some("Make `parse` handle empty input and explain the change."),
        ),
        ("n03", Some("n02"), Assistant, Some("I'll read `src/lib.rs` first.")),
        ("n04", Some("n03"), ToolResult, None),
        (
            "n05",
            Some("n04"),
            Assistant,
            Some("`parse` now returns an empty list for empty input instead of an error."),
        ),
        ("n06", Some("n05"), User, Some("Also reject whitespace-only input.")),
        (
            "n07",
            Some("n06"),
            Assistant,
            Some("Done: whitespace-only input now returns `Error::Empty`."),
        ),
        ("n08", Some("n05"), BranchSummary, None),
        (
            "n09",
            Some("n08"),
            User,
            Some("Keep whitespace accepted, but add a test for empty input."),
        ),
        ("n10", Some("n09"), Assistant, Some("Adding the test.")),
        ("n11", Some("n10"), ToolResult, None),
        (
            "n12",
            Some("n11"),
            Assistant,
            Some("Added `parses_empty_input`; the error type moved to `src/error.rs`."),
        ),
        ("n13", Some("n12"), User, Some("Run the tests.")),
        ("n14", Some("n13"), Assistant, Some("All 14 tests pass.")),
        ("n15", Some("n12"), User, Some("Now update the README example.")),
        ("n16", Some("n15"), Assistant, Some("Updating the README.")),
        ("n17", Some("n16"), ToolResult, None),
        (
            "n18",
            Some("n17"),
            Assistant,
            Some("The README example now shows that empty input parses to no records."),
        ),
    ]
}

const LEAF: &str = "n18";

/// Entry times: one minute apart from 10:00.
fn entry_time(index: usize) -> String {
    format!("2026-10-09T10:{index:02}:00Z")
}

/// Ids from the root to `leaf`.
fn active_path(entries: &[Entry], leaf: &str) -> Vec<&'static str> {
    let mut path = Vec::new();
    let mut current = Some(leaf);

    while let Some(id) = current {
        let (id, parent, ..) = entries.iter().find(|entry| entry.0 == id).expect("known entry");
        path.push(*id);
        current = *parent;
    }

    path.reverse();
    path
}

fn omp_tree() -> SessionTree {
    let entries = entries();
    let active = active_path(&entries, LEAF);
    let child_count = |id: &str| entries.iter().filter(|entry| entry.1 == Some(id)).count() as u32;

    let nodes: Vec<SessionNode> = entries
        .iter()
        .enumerate()
        .map(|(index, &(id, parent, kind, preview))| SessionNode {
            id: id.to_owned(),
            parent_id: parent.map(str::to_owned),
            kind,
            preview: preview.map(str::to_owned),
            timestamp: Some(entry_time(index)),
            label: (id == "n12").then(|| "empty-input fix".to_owned()),
            on_active_path: active.contains(&id),
            child_count: child_count(id),
        })
        .collect();

    SessionTree {
        session: session_ref(HarnessId::Omp, OMP_SESSION),
        branch_count: nodes.iter().filter(|node| node.child_count > 1).count() as u32,
        nodes,
        leaf_id: Some(LEAF.to_owned()),
        truncated: false,
    }
}

/// A message of entry `id` at the entry's time.
fn message(id: &str, role: MessageRole, blocks: Vec<ChatBlock>) -> ChatMessage {
    let index = entries().iter().position(|entry| entry.0 == id).expect("known entry");

    ChatMessage {
        id: Some(id.to_owned()),
        role,
        blocks,
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: Some(1_791_540_000_000 + index as i64 * 60_000),
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    }
}

fn text(text: &str) -> ChatBlock {
    ChatBlock::Text { text: text.to_owned() }
}

fn tool_call(id: &str, name: &str, args: Value) -> ChatBlock {
    ChatBlock::ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        args,
        intent: None,
    }
}

fn user(id: &str, prompt: &str) -> ChatMessage {
    message(id, MessageRole::User, vec![text(prompt)])
}

fn assistant(id: &str, blocks: Vec<ChatBlock>, stop_reason: &str, output: u64) -> ChatMessage {
    let usage = TokenUsage {
        input: 3_200,
        output,
        cache_read: 9_600,
        cache_write: 0,
        total: 12_800 + output,
    };

    ChatMessage {
        model: Some(MODEL.to_owned()),
        provider: Some("anthropic".to_owned()),
        stop_reason: Some(stop_reason.to_owned()),
        usage: Some(usage),
        cost_usd: Some(0.0125),
        ..message(id, MessageRole::Assistant, blocks)
    }
}

fn tool_result(id: &str, call: &str, name: &str, output: &str) -> ChatMessage {
    ChatMessage {
        tool_call_id: Some(call.to_owned()),
        tool_name: Some(name.to_owned()),
        is_error: Some(false),
        ..message(id, MessageRole::ToolResult, vec![text(output)])
    }
}

/// The whole active path of the OMP session as one page.
fn omp_messages() -> SessionMessagesPage {
    let read = tool_call("call-s1", "read", json!({ "path": "src/lib.rs" }));
    let edit = tool_call(
        "call-s2",
        "edit",
        json!({ "path": "src/lib.rs", "oldText": "-}", "newText": "}\n\n#[cfg(test)]\nmod tests {" }),
    );
    let readme = tool_call(
        "call-s3",
        "edit",
        json!({ "path": "README.md", "oldText": "parse(\"a,b\")", "newText": "parse(\"\")" }),
    );
    let summary = "The user explored rejecting whitespace-only input with `Error::Empty`, then went back.";

    let messages = vec![
        user("n02", "Make `parse` handle empty input and explain the change."),
        assistant("n03", vec![text("I'll read `src/lib.rs` first."), read], "toolUse", 32),
        tool_result(
            "n04",
            "call-s1",
            "read",
            "//! Line-oriented record parser.\n\npub fn parse(input: &str) …",
        ),
        assistant(
            "n05",
            vec![text(
                "`parse` now returns an empty list for empty input instead of an error.",
            )],
            "stop",
            120,
        ),
        message("n08", MessageRole::BranchSummary, vec![text(summary)]),
        user("n09", "Keep whitespace accepted, but add a test for empty input."),
        assistant("n10", vec![text("Adding the test."), edit], "toolUse", 210),
        tool_result("n11", "call-s2", "edit", "Applied 1 edit to src/lib.rs."),
        assistant(
            "n12",
            vec![text(
                "Added `parses_empty_input`; the error type moved to `src/error.rs`.",
            )],
            "stop",
            64,
        ),
        user("n15", "Now update the README example."),
        assistant("n16", vec![text("Updating the README."), readme], "toolUse", 48),
        tool_result("n17", "call-s3", "edit", "Applied 1 edit to README.md."),
        assistant(
            "n18",
            vec![text(
                "The README example now shows that empty input parses to no records.",
            )],
            "stop",
            40,
        ),
    ];

    SessionMessagesPage {
        total: messages.len() as u32,
        messages,
        next_cursor: None,
    }
}

/// The `sessions/` fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("sessions/sessions_list.omp.json", to_json(omp_sessions())),
        ("sessions/sessions_list.pi.json", to_json(pi_sessions())),
        ("sessions/session_tree.omp.json", to_json(omp_tree())),
        ("sessions/session_messages.omp.json", to_json(omp_messages())),
        (
            "sessions/session_files_touched.omp.json",
            json!(["README.md", "src/lib.rs"]),
        ),
    ]
}
