//! Hooks in Claude Code chats against the scripted `claude` (`fake_agent --dialect claude`,
//! `claude_hooks*.jsonl`, whose `user` steps match the exact text the CLI reads): every hook event
//! fires as for the other harnesses, `toolEnd` sees Claude Code's own tool ids (`Write`, `Edit`,
//! `MultiEdit`, `NotebookEdit`, `Bash`) with the project files they touched, and a prompt refused
//! before it was sent leaves the `sessionStart` context for the next one.
//!
//! A message the scenario did not expect makes the fake exit with a report on stderr
//! (`fake_agent: …`), which every test checks.

use std::sync::Mutex;
use std::time::Duration;

use harness_business::model::{
    AgentAction, AgentStartRequest, HarnessId, HookAction, HookAuthor, HookEvent, ImageAttachment,
};
use serde_json::Value;
use tauri::async_runtime::block_on;

use super::tests::{Chats, chats_of, context, events_of};
use crate::test_support::{save_global_hook_matching, start_request, wait_until};

/// The matcher of a hook that follows every Claude Code file change.
const FILE_TOOLS: &str = "Edit|MultiEdit|Write|NotebookEdit";

/// Claude Code chats that play `scenario`; `None` (the test skips) when `fake_agent` is not built.
fn claude_chats(scenario: &str) -> Option<Chats> {
    chats_of(HarnessId::Claude, scenario)
}

/// A request for a new Claude Code chat in the chats' project.
fn claude_request(chats: &Chats) -> AgentStartRequest {
    AgentStartRequest {
        harness: HarnessId::Claude,
        ..start_request(&chats.setup.project)
    }
}

fn command(command: &str) -> HookAction {
    HookAction::Command {
        command: command.to_owned(),
        timeout_secs: None,
    }
}

/// Panics when the fake reported a message its scenario did not expect, or a hook failed.
fn assert_all_matched(received: &Mutex<Vec<Value>>) {
    let mismatches: Vec<Value> = events_of(received, "stderr")
        .into_iter()
        .filter(|event| {
            event["line"]
                .as_str()
                .is_some_and(|line| line.starts_with("fake_agent:"))
        })
        .collect();

    assert!(mismatches.is_empty(), "{mismatches:#?}");
    assert!(events_of(received, "notice").is_empty(), "no hook failed");
}

/// Waits until `file` in the project reads `expected` (hook commands run in the background).
fn wait_for_file(chats: &Chats, file: &str, expected: &str) {
    let path = chats.setup.project.join(file);
    let read = || std::fs::read_to_string(&path).unwrap_or_default();

    wait_until(file, || read() == expected);
}

#[cfg(unix)]
#[test]
fn a_claude_chat_fires_every_hook_event_with_the_claude_tool_ids() {
    let Some(chats) = claude_chats("claude_hooks.jsonl") else {
        return;
    };
    let desktop = HookAuthor::Desktop;

    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        desktop,
        0,
    );
    chats.hook("Style", HookEvent::PromptSubmit, context("Be brief."), desktop, 1);
    save_global_hook_matching(
        chats.state(),
        "Tool log",
        (HookEvent::ToolEnd, Some(FILE_TOOLS)),
        command("printf '%s %s\\n' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\" >> tool-ended"),
        desktop,
        2,
    );
    save_global_hook_matching(
        chats.state(),
        "Changed files",
        (HookEvent::ToolEnd, Some(FILE_TOOLS)),
        command("printf 'tool %s touched %s' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\""),
        desktop,
        3,
    );
    save_global_hook_matching(
        chats.state(),
        "Shell",
        (HookEvent::ToolEnd, Some("Bash")),
        command("printf 'ran %s' \"$HARNESS_HOOK_TOOL\""),
        desktop,
        4,
    );
    chats.hook(
        "Turn done",
        HookEvent::TurnEnd,
        command("printf 'turn: %s\\n' \"$HARNESS_HOOK_TOUCHED\" >> turn-ended"),
        desktop,
        5,
    );
    chats.hook(
        "Wrap up",
        HookEvent::SessionEnd,
        command("touch session-ended"),
        desktop,
        6,
    );

    let (key, received) = chats.start(claude_request(&chats));
    chats.prompt(&key, "Fix the parser");

    // The user's run (Read, Write, a declined Bash call), then the run of its one follow-up (the
    // toolEnd output, in hook order: the Read matched no hook).
    wait_until("both runs to settle", || events_of(&received, "settled").len() == 2);

    // The follow-up's own run (an Edit) fired no toolEnd hook.
    wait_for_file(&chats, "tool-ended", "Write hello.txt\n");

    // The next user run edits three files and is interrupted: their toolEnd hooks fire, the
    // follow-up does not.
    chats.prompt(&key, "Done");
    wait_until("the edits to end", || events_of(&received, "toolEnd").len() == 7);
    chats.send(&key, AgentAction::Abort).expect("the run is interrupted");
    wait_until("the interrupted run to settle", || {
        events_of(&received, "settled").len() == 3
    });

    // A follow-up would reach the CLI before this prompt, and fail its match.
    std::thread::sleep(Duration::from_millis(300));
    chats.prompt(&key, "Last");
    wait_until("the last run to settle", || events_of(&received, "settled").len() == 4);
    assert_all_matched(&received);

    // turnEnd ran for every user run (the interrupted one too), never for the follow-up's run.
    wait_for_file(
        &chats,
        "turn-ended",
        "turn: hello.txt\nturn: src/lib.rs\nsrc/multi.rs\nsrc/nb.ipynb\nturn: \n",
    );
    wait_for_file(
        &chats,
        "tool-ended",
        "Write hello.txt\nEdit src/lib.rs\nMultiEdit src/multi.rs\nNotebookEdit src/nb.ipynb\n",
    );

    let project = &chats.setup.project;
    block_on(chats.state().agents.stop_all());
    block_on(chats.state().hooks.finish_chats(Duration::from_secs(3)));
    assert!(project.join("session-ended").exists(), "sessionEnd ran");
}

#[test]
fn a_prompt_refused_before_it_reached_claude_keeps_the_session_start_context() {
    let Some(chats) = claude_chats("claude_hooks_refused.jsonl") else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );

    let (key, received) = chats.start(claude_request(&chats));

    let refused = AgentAction::Prompt {
        message: "Look".to_owned(),
        images: vec![ImageAttachment {
            data: "AAAA".to_owned(),
            mime_type: "image/tiff".to_owned(),
        }],
        streaming_behavior: None,
    };
    chats.send(&key, refused).expect_err("the image type is refused");

    chats.prompt(&key, "Hello");
    wait_until("the run to settle", || events_of(&received, "settled").len() == 1);
    assert_all_matched(&received);
}
