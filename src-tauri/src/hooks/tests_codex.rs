//! Hooks in Codex chats against the scripted `codex app-server` (`fake_agent --dialect codex`,
//! `codex_hooks*.jsonl`, whose `turn/start` steps match the exact text Codex receives): every hook
//! event fires as for the other harnesses, `toolEnd` sees the app's Codex tool ids (`shell`,
//! `apply_patch`, `mcp__<server>__<tool>`) with the project files a change touched (both paths of a
//! move, nothing outside the project), and a prompt refused before it was sent leaves the
//! `sessionStart` context for the next one (see `tests_cli`).

use harness_business::model::HarnessId;

use super::tests_cli::assert_a_refused_prompt_keeps_the_session_start_context;
#[cfg(unix)]
use super::tests_cli::{Expected, Matchers, assert_every_event_fires};

#[cfg(unix)]
#[test]
fn a_codex_chat_fires_every_hook_event_with_the_codex_tool_ids() {
    let matchers = Matchers {
        logged: "apply_patch|mcp__harness-memory__note_save",
        changed: "apply_patch",
        shell: "shell",
    };

    // The user's run has a command fail, adds hello.txt and lists the files (the "Shell" hook
    // prints once), the follow-up's run edits src/lib.rs. The "Done" run edits src/lib.rs, moves
    // src/old.rs to src/new.rs and deletes src/gone.rs, adds a file outside the project and saves a
    // note through Harness Memory's MCP server.
    let expected = Expected {
        first_logged: "apply_patch hello.txt\n",
        tool_ends: 8,
        turn_ended: "turn: hello.txt\nturn: src/gone.rs\nsrc/lib.rs\nsrc/new.rs\nsrc/old.rs\nturn: \n",
        tool_ended: "apply_patch hello.txt\napply_patch src/lib.rs\napply_patch src/old.rs\nsrc/new.rs\nsrc/gone.rs\n\
                     apply_patch \nmcp__harness-memory__note_save \n",
    };

    assert_every_event_fires(HarnessId::Codex, "codex_hooks.jsonl", &matchers, &expected);
}

#[test]
fn a_prompt_refused_before_it_reached_codex_keeps_the_session_start_context() {
    assert_a_refused_prompt_keeps_the_session_start_context(HarnessId::Codex, "codex_hooks_refused.jsonl");
}
