//! Hooks in Claude Code chats against the scripted `claude` (`fake_agent --dialect claude`,
//! `claude_hooks*.jsonl`, whose `user` steps match the exact text the CLI reads): every hook event
//! fires as for the other harnesses, `toolEnd` sees Claude Code's own tool ids (`Write`, `Edit`,
//! `MultiEdit`, `NotebookEdit`, `Bash`) with the project files they touched, and a prompt refused
//! before it was sent leaves the `sessionStart` context for the next one (see `tests_cli`).

use harness_business::model::HarnessId;

use super::tests_cli::assert_a_refused_prompt_keeps_the_session_start_context;
#[cfg(unix)]
use super::tests_cli::{Expected, Matchers, assert_every_event_fires};

/// The matcher of a hook that follows every Claude Code file change.
#[cfg(unix)]
const FILE_TOOLS: &str = "Edit|MultiEdit|Write|NotebookEdit";

#[cfg(unix)]
#[test]
fn a_claude_chat_fires_every_hook_event_with_the_claude_tool_ids() {
    let matchers = Matchers {
        logged: FILE_TOOLS,
        changed: FILE_TOOLS,
        shell: "Bash",
    };

    // The user's run reads a file, writes hello.txt and has a Bash call declined (the Read matches
    // no hook), the follow-up's run edits src/lib.rs. The "Done" run edits three files.
    let expected = Expected {
        first_logged: "Write hello.txt\n",
        tool_ends: 7,
        turn_ended: "turn: hello.txt\nturn: src/lib.rs\nsrc/multi.rs\nsrc/nb.ipynb\nturn: \n",
        tool_ended: "Write hello.txt\nEdit src/lib.rs\nMultiEdit src/multi.rs\nNotebookEdit src/nb.ipynb\n",
    };

    assert_every_event_fires(HarnessId::Claude, "claude_hooks.jsonl", &matchers, &expected);
}

#[test]
fn a_prompt_refused_before_it_reached_claude_keeps_the_session_start_context() {
    assert_a_refused_prompt_keeps_the_session_start_context(HarnessId::Claude, "claude_hooks_refused.jsonl");
}
