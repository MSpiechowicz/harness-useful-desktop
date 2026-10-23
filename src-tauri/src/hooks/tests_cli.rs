//! What the hook tests of Claude Code and Codex chats share (`tests_claude`, `tests_codex`): the
//! hooks of every event, the runs their `*_hooks.jsonl` scenarios script, the check that the fake
//! matched every message, and a prompt refused before it was sent. Each test skips when
//! `fake_agent` is not built.
//!
//! The scripted runs: the prompt "Fix the parser" carries the sessionStart and promptSubmit
//! context, its run ends tools and earns one follow-up, whose own run earns none. The prompt
//! "Done" runs tools until it is interrupted (no follow-up), and the prompt "Last" is the next
//! message the agent reads. A message the scenario did not expect makes the fake exit with a
//! report on stderr (`fake_agent: …`).

use std::sync::Mutex;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use harness_business::model::HookAction;
use harness_business::model::{AgentAction, AgentStartRequest, HarnessId, HookAuthor, HookEvent, ImageAttachment};
use serde_json::Value;
#[cfg(unix)]
use tauri::async_runtime::block_on;

use super::tests::{Chats, chats_of, context, events_of};
#[cfg(unix)]
use crate::test_support::{WAIT, save_global_hook_matching};
use crate::test_support::{start_request, wait_until};

/// The `toolEnd` matchers of the hooks [`assert_every_event_fires`] saves.
#[cfg(unix)]
pub(super) struct Matchers<'a> {
    /// "Tool log" appends the tool and the files it touched to `tool-ended`.
    pub(super) logged: &'a str,
    /// "Changed files" prints the tool and the files it touched (a follow-up).
    pub(super) changed: &'a str,
    /// "Shell" prints the tool (a follow-up).
    pub(super) shell: &'a str,
}

/// What [`assert_every_event_fires`] expects of the scripted runs.
#[cfg(unix)]
pub(super) struct Expected<'a> {
    /// `tool-ended` once the first run and its follow-up's run settled.
    pub(super) first_logged: &'a str,
    /// The `toolEnd` events once the "Done" run ended its tools.
    pub(super) tool_ends: usize,
    /// `turn-ended` at the end: one line per user run, its files sorted.
    pub(super) turn_ended: &'a str,
    /// `tool-ended` at the end.
    pub(super) tool_ended: &'a str,
}

/// A request for a new chat of `harness` in the chats' project.
fn request(chats: &Chats, harness: HarnessId) -> AgentStartRequest {
    AgentStartRequest {
        harness,
        ..start_request(&chats.setup.project)
    }
}

#[cfg(unix)]
fn command(command: &str) -> HookAction {
    HookAction::Command {
        command: command.to_owned(),
        timeout_secs: None,
    }
}

/// Saves a hook for every event, in run order: "Project notes" (sessionStart context), "Style"
/// (promptSubmit context), the three `toolEnd` hooks of `matchers`, "Turn done" (appends the run's
/// files to `turn-ended`) and "Wrap up" (touches `session-ended`).
#[cfg(unix)]
fn save_every_event_hooks(chats: &Chats, matchers: &Matchers<'_>) {
    let desktop = HookAuthor::Desktop;
    let tool_end = |name: &str, matcher: &str, action: &str, secs: u64| {
        save_global_hook_matching(
            chats.state(),
            name,
            (HookEvent::ToolEnd, Some(matcher)),
            command(action),
            desktop,
            secs,
        );
    };

    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        desktop,
        0,
    );
    chats.hook("Style", HookEvent::PromptSubmit, context("Be brief."), desktop, 1);

    tool_end(
        "Tool log",
        matchers.logged,
        "printf '%s %s\\n' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\" >> tool-ended",
        2,
    );
    tool_end(
        "Changed files",
        matchers.changed,
        "printf 'tool %s touched %s' \"$HARNESS_HOOK_TOOL\" \"$HARNESS_HOOK_TOUCHED\"",
        3,
    );
    tool_end("Shell", matchers.shell, "printf 'ran %s' \"$HARNESS_HOOK_TOOL\"", 4);

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
}

/// Starts a chat of `harness` that plays `scenario` with a hook for every event (`toolEnd` ones
/// with `matchers`), plays the scripted runs (see the module docs) and checks what the hooks
/// wrote, then that `sessionEnd` ran when the chat stopped.
#[cfg(unix)]
pub(super) fn assert_every_event_fires(
    harness: HarnessId,
    scenario: &str,
    matchers: &Matchers<'_>,
    expected: &Expected<'_>,
) {
    let Some(chats) = chats_of(harness, scenario) else {
        return;
    };
    save_every_event_hooks(&chats, matchers);

    let (key, received) = chats.start(request(&chats, harness));
    chats.prompt(&key, "Fix the parser");

    // The user's run, then the run of its one follow-up (the toolEnd output, in hook order).
    wait_until("both runs to settle", || events_of(&received, "settled").len() == 2);

    // The follow-up's own run fired no toolEnd hook.
    wait_for_file(&chats, "tool-ended", expected.first_logged);

    // The next user run ends its tools and is interrupted: their toolEnd hooks fire, the follow-up
    // does not.
    chats.prompt(&key, "Done");
    wait_until("the tools to end", || {
        events_of(&received, "toolEnd").len() == expected.tool_ends
    });
    chats.send(&key, AgentAction::Abort).expect("the run is interrupted");
    wait_until("the interrupted run to settle", || {
        events_of(&received, "settled").len() == 3
    });

    // A follow-up would reach the agent before this prompt, and fail its match.
    std::thread::sleep(Duration::from_millis(300));
    chats.prompt(&key, "Last");
    wait_until("the last run to settle", || events_of(&received, "settled").len() == 4);
    assert_all_matched(&received);

    // turnEnd ran for every user run (the interrupted one too), never for the follow-up's run.
    wait_for_file(&chats, "turn-ended", expected.turn_ended);
    wait_for_file(&chats, "tool-ended", expected.tool_ended);

    let project = &chats.setup.project;
    block_on(chats.state().agents.stop_all());
    block_on(chats.state().hooks.finish_chats(Duration::from_secs(3)));
    assert!(project.join("session-ended").exists(), "sessionEnd ran");
}

/// Starts a chat of `harness` that plays `scenario` with a sessionStart hook, has a prompt refused
/// before it reached the agent (an image type no chat takes), then sends "Hello", which the
/// scenario expects with the context.
pub(super) fn assert_a_refused_prompt_keeps_the_session_start_context(harness: HarnessId, scenario: &str) {
    let Some(chats) = chats_of(harness, scenario) else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );

    let (key, received) = chats.start(request(&chats, harness));

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

    // A chat's own notices (a Codex thread that may write outside the project) are no hook failure.
    let failed: Vec<Value> = events_of(received, "notice")
        .into_iter()
        .filter(|event| {
            event["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("The hook "))
        })
        .collect();
    assert!(failed.is_empty(), "no hook failed: {failed:#?}");
}

/// Waits until `file` in the project reads `expected` (hook commands run in the background), and
/// shows what it reads when it never does.
#[cfg(unix)]
fn wait_for_file(chats: &Chats, file: &str, expected: &str) {
    let path = chats.setup.project.join(file);
    let read = || std::fs::read_to_string(&path).unwrap_or_default();
    let deadline = Instant::now() + WAIT;

    while read() != expected && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(read(), expected, "{file}");
}
