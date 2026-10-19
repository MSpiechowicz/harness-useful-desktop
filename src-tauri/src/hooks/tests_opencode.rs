//! Hooks in OpenCode chats against the scripted `opencode serve` (`opencode_hooks*.jsonl`, whose
//! `prompt_async` steps match the exact text the server receives; a mismatch fails the send):
//! every hook event fires as in an OMP chat, and a prompt refused before it was sent leaves the
//! `sessionStart` context for the next one.

use std::time::Duration;

use harness_business::model::{
    AgentAction, AgentStartRequest, HarnessId, HookAction, HookAuthor, HookEvent, ImageAttachment,
};
use tauri::async_runtime::block_on;

use super::tests::{Chats, chats_of, context, events_of};
use crate::test_support::{save_global_hook_matching, start_request, wait_until};

/// OpenCode chats that play `scenario`; `None` (the test skips) when `fake_agent` is not built.
fn opencode_chats(scenario: &str) -> Option<Chats> {
    chats_of(HarnessId::OpenCode, scenario)
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

#[cfg(unix)]
#[test]
fn an_opencode_chat_fires_every_hook_event() {
    let Some(chats) = opencode_chats("opencode_hooks.jsonl") else {
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
        "Changed files",
        (HookEvent::ToolEnd, Some("edit|write")),
        command("printf 'tool %s touched %s' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\""),
        desktop,
        2,
    );
    chats.hook(
        "Turn done",
        HookEvent::TurnEnd,
        command("printf '%s\\n' \"$HARNESS_HOOK_TOUCHED\" >> turn-ended"),
        desktop,
        3,
    );
    chats.hook(
        "Wrap up",
        HookEvent::SessionEnd,
        command("touch session-ended"),
        desktop,
        4,
    );

    let (key, received) = chats.start(opencode_request(&chats));
    chats.prompt(&key, "Fix the parser");

    // The user's run, then the run of its follow-up (the toolEnd output).
    wait_until("both runs to settle", || events_of(&received, "settled").len() == 2);

    // A second follow-up would reach the server before this prompt, and fail its match.
    std::thread::sleep(Duration::from_millis(300));
    chats.prompt(&key, "Done");
    assert!(events_of(&received, "notice").is_empty(), "no hook failed");

    // turnEnd ran once, for the user's run only, with the files that run touched.
    let project = &chats.setup.project;
    let turn_ended = std::fs::read_to_string(project.join("turn-ended")).expect("turnEnd ran");
    assert_eq!(turn_ended, "src/new.rs\n");

    block_on(chats.state().agents.stop_all());
    block_on(chats.state().hooks.finish_chats(Duration::from_secs(3)));
    assert!(project.join("session-ended").exists(), "sessionEnd ran");
}

#[test]
fn a_prompt_refused_before_it_reached_opencode_keeps_the_session_start_context() {
    let Some(chats) = opencode_chats("opencode_hooks_refused.jsonl") else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );

    let (key, _) = chats.start(opencode_request(&chats));

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
}
