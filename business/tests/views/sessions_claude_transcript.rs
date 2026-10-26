//! Claude Code saved sessions: branch trees, transcripts and touched files over the fixtures in
//! `tests/fixtures/sessions/claude` (see `claude_session_support`).

use crate::claude_session_support;
use crate::common;

use claude_session_support::{CLONE, COMPACTED, EDITS, SHELL, TOOLS, append_line, claude, claude_home, replace_in};
use common::CANARY;
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{
    ChatBlock, ChatMessage, IpcErrorCode, MessageRole, ModelTurns, SessionMessagesPage, SessionNodeKind,
};
use pretty_assertions::assert_eq;

/// A line id of the synthetic session.
fn line(number: u32) -> String {
    format!("e0000000-0000-4000-8000-{number:012}")
}

fn page(home: &claude_session_support::ClaudeHome, id: &str) -> SessionMessagesPage {
    Claude
        .session_messages(&home.home.env, &home.home.project, &claude(id), None, 200)
        .unwrap()
}

fn roles(messages: &[ChatMessage]) -> Vec<MessageRole> {
    messages.iter().map(|message| message.role).collect()
}

fn texts(message: &ChatMessage) -> Vec<&str> {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn tool_calls(message: &ChatMessage) -> Vec<&str> {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::ToolCall { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_lines_of_one_reply_read_as_one_message() {
    use MessageRole::{Assistant, Bash, System, ToolResult, User};

    let home = claude_home();
    let page = page(&home, EDITS);
    let messages = &page.messages;

    assert_eq!(
        roles(messages),
        [
            User, Assistant, ToolResult, ToolResult, Assistant, ToolResult, ToolResult, Assistant, User, System, Bash,
            User, Assistant
        ]
    );

    // Thinking and two tool calls, three lines of one reply, named after the last line.
    let first = &messages[1];
    assert_eq!(first.id, Some(line(5)));
    assert_eq!(
        first.blocks[0],
        ChatBlock::Thinking {
            text: "Planning the edits".to_owned(),
            redacted: false
        }
    );
    assert_eq!(tool_calls(first), ["Edit", "NotebookEdit"]);
    assert_eq!(first.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(first.stop_reason.as_deref(), Some("tool_use"));
    assert_eq!(first.usage.map(|usage| usage.total), Some(1130));

    let edit = &messages[2];
    assert_eq!(edit.tool_name.as_deref(), Some("Edit"));
    assert_eq!(edit.tool_call_id.as_deref(), Some("toolu_demo_edit"));
    assert_eq!(edit.is_error, Some(false));

    // A reply line after a tool result of the same reply still joins it.
    assert_eq!(tool_calls(&messages[4]), ["MultiEdit", "Write"]);
    assert_eq!(messages[4].id, Some(line(10)));
    assert_eq!(messages[6].tool_name.as_deref(), Some("Write"));
    assert_eq!(messages[6].is_error, Some(true));

    assert_eq!(texts(&messages[7]), ["Done."]);
    assert_eq!(texts(&messages[8]), ["/model sonnet"]);
    assert_eq!(
        texts(&messages[9]),
        ["Set model to Sonnet"],
        "terminal escapes are dropped"
    );
    assert_eq!(
        texts(&messages[10]),
        ["$ git status\nOn branch main"],
        "the output line joins its run"
    );

    let image = &messages[11];
    assert_eq!(texts(image), ["What is in this screenshot?"]);
    assert_eq!(
        image.blocks[1],
        ChatBlock::Image {
            mime_type: "image/png".to_owned()
        }
    );
    assert_eq!(messages[12].model.as_deref(), Some("claude-sonnet-5"));
    assert_eq!(messages[12].timestamp, Some(1_792_487_520_000));

    let stats = page.stats.expect("the first page has the totals");
    assert_eq!(stats.messages, 7);
    assert_eq!(stats.tool_calls, 4);
    assert_eq!(stats.context_tokens, Some(1130));
    assert_eq!(stats.cost_usd, None);
    assert_eq!(
        stats.models,
        [
            ModelTurns {
                model: "claude-opus-5-5".to_owned(),
                turns: 3
            },
            ModelTurns {
                model: "claude-sonnet-5".to_owned(),
                turns: 1
            }
        ]
    );

    let shown = format!("{messages:?}");
    assert!(
        !shown.contains(CANARY),
        "attachment, request, subagent or hidden lines leaked"
    );
    assert!(!shown.contains("iVBORw0KGgo"), "image data leaked");
}

#[test]
fn the_tree_follows_parent_uuids_and_marks_the_active_path() {
    let home = claude_home();
    let tree = Claude
        .session_tree(&home.home.env, &home.home.project, &claude(EDITS))
        .unwrap();

    assert_eq!(
        tree.nodes.len(),
        17,
        "subagent, attachment and hidden lines are not nodes"
    );
    assert_eq!(tree.leaf_id, Some(line(24)));
    assert_eq!(tree.branch_count, 1);
    assert!(!tree.truncated);

    let node = |id: String| tree.nodes.iter().find(|node| node.id == id).expect("node");

    // The three lines of the first reply are one node.
    let reply = node(line(5));
    assert_eq!(reply.kind, SessionNodeKind::Assistant);
    assert_eq!(reply.parent_id, Some(line(2)));
    assert!(tree.nodes.iter().all(|node| node.id != line(3) && node.id != line(4)));

    // `Done.` has two children: the abandoned branch and the active one.
    let done = node(line(12));
    assert_eq!(done.child_count, 2);
    assert_eq!(done.preview.as_deref(), Some("Done."));

    let abandoned = node(line(15));
    assert_eq!(abandoned.preview.as_deref(), Some("Actually, stop here"));
    assert!(!abandoned.on_active_path);
    assert!(!node(line(16)).on_active_path);

    assert_eq!(node(line(17)).preview.as_deref(), Some("/model sonnet"));
    assert_eq!(node(line(18)).kind, SessionNodeKind::Other);
    assert_eq!(node(line(19)).kind, SessionNodeKind::Bash);

    // The prompt after the hidden caveat line hangs on the shell output before it.
    let prompt = node(line(22));
    assert_eq!(prompt.parent_id, Some(line(20)));
    assert!(prompt.on_active_path);

    let on_path = tree.nodes.iter().filter(|node| node.on_active_path).count();
    assert_eq!(on_path, 15);
    assert!(!format!("{tree:?}").contains(CANARY));
}

#[test]
fn compaction_shows_its_summary_where_it_happened() {
    use MessageRole::{Assistant, CompactionSummary, System, User};

    let home = claude_home();
    let messages = page(&home, COMPACTED).messages;

    assert_eq!(
        roles(&messages),
        [
            User,
            Assistant,
            User,
            Assistant,
            CompactionSummary,
            User,
            System,
            User,
            Assistant
        ]
    );
    assert_eq!(texts(&messages[4]), ["pong\nping"], "Claude Code's framing is dropped");
    assert_eq!(texts(&messages[5]), ["/compact keep only the words pong and ping"]);
    assert_eq!(texts(&messages[6]), ["Compacted"]);
    assert_eq!(texts(&messages[8]), ["after-compact"]);

    // The boundary links the summary to the last message before it.
    let tree = Claude
        .session_tree(&home.home.env, &home.home.project, &claude(COMPACTED))
        .unwrap();
    let summary = tree
        .nodes
        .iter()
        .find(|node| node.kind == SessionNodeKind::Compaction)
        .unwrap();
    assert_eq!(summary.id, "efc19932-b68e-440c-8271-dd637b24f639");
    assert_eq!(
        summary.parent_id.as_deref(),
        Some("adab2b4c-7bbb-426b-bc64-680b7a1803e5")
    );
    assert!(tree.nodes.iter().all(|node| node.on_active_path));
}

#[test]
fn a_clone_of_a_compacted_session_starts_at_its_summary() {
    let home = claude_home();
    let messages = page(&home, CLONE).messages;

    assert_eq!(messages[0].role, MessageRole::CompactionSummary);
    assert_eq!(texts(&messages[1]), ["ping"], "the kept reply follows the summary");
    assert_eq!(texts(messages.last().unwrap()), ["cloned"]);

    let tree = Claude
        .session_tree(&home.home.env, &home.home.project, &claude(CLONE))
        .unwrap();
    assert_eq!(tree.nodes[0].kind, SessionNodeKind::Compaction);
    assert_eq!(
        tree.nodes[0].parent_id, None,
        "a parent later in the file is not followed"
    );
}

#[test]
fn an_injected_shell_run_reads_as_a_bash_message() {
    let home = claude_home();
    let messages = page(&home, SHELL).messages;

    let shell: Vec<&ChatMessage> = messages
        .iter()
        .filter(|message| message.role == MessageRole::Bash)
        .collect();
    assert_eq!(shell.len(), 1);
    assert_eq!(texts(shell[0]), ["$ echo injected\ninjected-output-42"]);
    assert_eq!(texts(&messages[0]), ["!echo typed-bang"], "a typed `!` is a prompt");
}

#[test]
fn declined_and_failed_tool_calls_are_errors() {
    let home = claude_home();
    let messages = page(&home, TOOLS).messages;

    let results: Vec<(&str, bool)> = messages
        .iter()
        .filter(|message| message.role == MessageRole::ToolResult)
        .map(|message| (message.tool_name.as_deref().unwrap(), message.is_error.unwrap()))
        .collect();
    assert_eq!(results, [("Read", false), ("Write", false), ("Bash", true)]);
}

#[test]
fn files_touched_are_the_edit_tools_paths_inside_the_project() {
    let home = claude_home();
    std::fs::create_dir_all(home.home.project.join("notes")).unwrap();

    let touched = |id: &str| {
        Claude
            .session_files_touched(&home.home.env, &home.home.project, &claude(id))
            .unwrap()
    };

    // Edit, NotebookEdit and a relative MultiEdit; the Write outside the project and the
    // subagent's Write are left out.
    assert_eq!(touched(EDITS), ["notes/analysis.ipynb", "src/lib.rs", "src/parse.rs"]);
    assert_eq!(touched(TOOLS), ["hello.txt"]);
    assert!(touched(SHELL).is_empty());
}

#[test]
fn pages_continue_where_the_previous_one_ended() {
    let home = claude_home();
    let env = &home.home.env;
    let project = &home.home.project;
    let session = claude(EDITS);

    let first = Claude.session_messages(env, project, &session, None, 5).unwrap();
    assert_eq!(first.messages.len(), 5);
    assert_eq!(first.total, 13);
    assert_eq!(first.next_cursor.as_deref(), Some("5"));
    assert!(first.stats.is_some());

    let rest = Claude.session_messages(env, project, &session, Some("5"), 200).unwrap();
    assert_eq!(rest.messages.len(), 8);
    assert_eq!(rest.next_cursor, None);
    assert!(rest.stats.is_none());
    assert_eq!(rest.messages[0].id, Some(line(9)));

    let unknown = Claude.session_messages(env, project, &session, Some("99"), 5);
    assert_eq!(unknown.expect_err("unknown cursor").code(), IpcErrorCode::InvalidInput);
}

/// Paging through a session parses its file once while it is unchanged: a later page comes from
/// that parse, and a file that changed size is read again.
#[test]
fn later_pages_reuse_the_parse_until_the_file_changes() {
    let home = claude_home();
    let env = &home.home.env;
    let project = &home.home.project;
    let session = claude(EDITS);
    let reply = |page: &SessionMessagesPage| {
        page.messages
            .iter()
            .any(|message| texts(message).contains(&"A terminal."))
    };

    Claude.session_messages(env, project, &session, None, 5).unwrap();
    let rest = Claude.session_messages(env, project, &session, Some("5"), 200).unwrap();
    assert!(reply(&rest));

    // Same size and modification time: the parse is kept.
    replace_in(&home.file(EDITS), "A terminal.", "A computer.");
    let cached = Claude.session_messages(env, project, &session, Some("5"), 200).unwrap();
    assert_eq!(cached, rest);

    // A new line changes the size: the file is read again.
    append_line(
        &home.file(EDITS),
        &serde_json::json!({ "type": "summary", "summary": "x" }),
    );
    let fresh = Claude.session_messages(env, project, &session, Some("5"), 200).unwrap();
    assert!(!reply(&fresh));
    assert!(
        fresh
            .messages
            .iter()
            .any(|message| texts(message).contains(&"A computer."))
    );
}
