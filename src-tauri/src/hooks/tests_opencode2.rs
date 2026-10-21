//! Hooks in OpenCode 2 chats against the scripted OpenCode 2 `opencode serve` (`opencode2_hooks*.jsonl`,
//! whose prompt steps match the exact text the server receives): every hook event fires as on OpenCode 1,
//! `toolEnd` sees the OpenCode 2 tool ids (`edit`, `patch`, `shell`, an MCP tool) with the files they
//! touched, and a prompt refused before it was sent leaves the `sessionStart` context for the next one.
//!
//! An OpenCode 2 prompt goes out in the background, so a request the scenario did not expect does not
//! fail the send: the fake reports it on stderr (`fake_agent: …`), which every test checks.

use std::sync::Mutex;
use std::time::Duration;

use harness_business::model::{
    AgentAction, AgentStartRequest, HarnessId, HookAction, HookAuthor, HookEvent, ImageAttachment,
};
use serde_json::Value;
use tauri::async_runtime::block_on;

use super::tests::{Chats, context, events_of};
use crate::test_support::{agent_setup_opencode2, fake_agent, save_global_hook_matching, start_request, wait_until};

/// OpenCode 2 chats that play `scenario`; `None` (the test skips) when `fake_agent` is not built.
fn opencode2_chats(scenario: &str) -> Option<Chats> {
    let binary = fake_agent()?;

    Some(Chats {
        setup: agent_setup_opencode2(&binary, scenario),
    })
}

/// A request for a new OpenCode chat in the chats' project.
fn opencode_request(chats: &Chats) -> AgentStartRequest {
    AgentStartRequest {
        harness: HarnessId::OpenCode,
        ..start_request(&chats.setup.project)
    }
}

fn command(command: &str) -> HookAction {
    HookAction::Command {
        command: command.to_owned(),
        timeout_secs: None,
    }
}

/// Panics when the fake reported a request its scenario did not expect, or a hook failed.
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

/// The text of `file` in the project, once it is `expected` (hook commands run in the background).
fn wait_for_file(chats: &Chats, file: &str, expected: &str) {
    let path = chats.setup.project.join(file);
    let read = || std::fs::read_to_string(&path).unwrap_or_default();

    wait_until(file, || read() == expected);
}

#[cfg(unix)]
#[test]
fn an_opencode2_chat_fires_every_hook_event_with_the_opencode2_tool_ids() {
    let Some(chats) = opencode2_chats("opencode2_hooks.jsonl") else {
        return;
    };
    let desktop = HookAuthor::Desktop;
    let file_tools = Some("edit|write|apply_patch|patch");

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
        (HookEvent::ToolEnd, file_tools),
        command("printf '%s %s\\n' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\" >> tool-ended"),
        desktop,
        2,
    );
    save_global_hook_matching(
        chats.state(),
        "Changed files",
        (HookEvent::ToolEnd, file_tools),
        command("printf 'tool %s touched %s' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\""),
        desktop,
        3,
    );
    save_global_hook_matching(
        chats.state(),
        "Shell",
        (HookEvent::ToolEnd, Some("bash|shell")),
        command("printf 'ran %s' \"$HARNESS_HOOK_TOOL\""),
        desktop,
        4,
    );
    save_global_hook_matching(
        chats.state(),
        "Issues",
        (HookEvent::ToolEnd, Some("github_search_issues")),
        HookAction::FollowUp {
            text: "Link the issues.".to_owned(),
        },
        desktop,
        5,
    );
    chats.hook(
        "Turn done",
        HookEvent::TurnEnd,
        command("printf 'turn: %s\\n' \"$HARNESS_HOOK_TOUCHED\" >> turn-ended"),
        desktop,
        6,
    );
    chats.hook(
        "Wrap up",
        HookEvent::SessionEnd,
        command("touch session-ended"),
        desktop,
        7,
    );

    let (key, received) = chats.start(opencode_request(&chats));
    chats.prompt(&key, "Fix the parser");

    // The user's run, then the run of its one follow-up (the toolEnd output, in hook order).
    wait_until("both runs to settle", || events_of(&received, "settled").len() == 2);

    // The follow-up's own run fired no toolEnd: only the user run's file changes are logged, each
    // with the file it touched (an absolute `path`, a patch header).
    wait_for_file(&chats, "tool-ended", "edit src/lib.rs\npatch src/new.rs\n");

    // The next user run writes a file and is interrupted: its toolEnd fires, its follow-up does not.
    chats.prompt(&key, "Done");
    wait_until("the write to end", || events_of(&received, "toolEnd").len() == 6);
    chats.send(&key, AgentAction::Abort).expect("the run is interrupted");
    wait_until("the interrupted run to settle", || {
        events_of(&received, "settled").len() == 3
    });

    // A follow-up would reach the server before this prompt, and fail its match.
    std::thread::sleep(Duration::from_millis(300));
    chats.prompt(&key, "Last");
    wait_until("the last run to settle", || events_of(&received, "settled").len() == 4);
    assert_all_matched(&received);

    // turnEnd ran for every user run (the interrupted one too), never for the follow-up's run.
    wait_for_file(
        &chats,
        "turn-ended",
        "turn: src/lib.rs\nsrc/new.rs\nturn: src/done.rs\nturn: \n",
    );
    wait_for_file(
        &chats,
        "tool-ended",
        "edit src/lib.rs\npatch src/new.rs\nwrite src/done.rs\n",
    );

    let project = &chats.setup.project;
    block_on(chats.state().agents.stop_all());
    block_on(chats.state().hooks.finish_chats(Duration::from_secs(3)));
    assert!(project.join("session-ended").exists(), "sessionEnd ran");
}

#[test]
fn a_prompt_refused_before_it_reached_opencode2_keeps_the_session_start_context() {
    let Some(chats) = opencode2_chats("opencode2_hooks_refused.jsonl") else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );

    let (key, received) = chats.start(opencode_request(&chats));

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
