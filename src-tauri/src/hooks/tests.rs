//! Hooks in chats against scripted agents (`omp_hooks_*.jsonl`, whose `prompt` steps match the
//! exact text the agent receives; a mismatch makes the agent exit, failing the send).

use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use harness_business::hooks::HookCapabilities;
use harness_business::model::{
    AgentAction, AgentReply, AgentStartRequest, HarnessId, HookAction, HookAuthor, HookDraft, HookEvent, HookScope,
    HookStatus, HookView, SessionRef,
};
use serde_json::Value;
use tauri::async_runtime::block_on;

use super::chat;
use crate::commands::{agent, hooks as hook_commands};
use crate::state::AppState;
use crate::test_support::{AgentSetup, agent_setup, collecting_channel, fake_agent, start_request, wait_until};

/// A chat app state whose agents play one scenario.
struct Chats {
    setup: AgentSetup,
}

/// `None` (the test skips) when `fake_agent` is not built.
fn chats(scenario: &str) -> Option<Chats> {
    let binary = fake_agent()?;

    Some(Chats {
        setup: agent_setup(&binary, scenario),
    })
}

impl Chats {
    fn state(&self) -> &AppState {
        &self.setup.state
    }

    /// Writes a global hook as `author` (the desktop's is approved, an agent's pending), created
    /// `secs` after a fixed time so the run order is known.
    fn hook(&self, name: &str, event: HookEvent, action: HookAction, author: HookAuthor, secs: u64) -> HookView {
        let draft = HookDraft {
            id: None,
            expected_sha256: None,
            name: name.to_owned(),
            description: None,
            scope: HookScope::Global,
            project: None,
            event,
            matcher: None,
            action,
            enabled: true,
        };
        let created = UNIX_EPOCH + Duration::from_secs(1_791_000_000 + secs);

        let store = self.state().hooks.store().expect("a hooks folder");
        let saved = store
            .save(&draft, author, HookCapabilities::default(), created)
            .expect("hook saved");
        self.state().hooks.invalidate();

        saved
    }

    fn start(&self, request: AgentStartRequest) -> (String, Arc<Mutex<Vec<Value>>>) {
        let (channel, received) = collecting_channel();
        let key = block_on(agent::start(self.state(), request, channel))
            .expect("the chat starts")
            .key;

        (key, received)
    }

    fn send(&self, key: &str, action: AgentAction) -> harness_business::Result<AgentReply> {
        block_on(chat::send(self.state(), key, action))
    }

    /// Sends `message`; fails when the agent did not accept it (a `match` failed).
    fn prompt(&self, key: &str, message: &str) {
        self.send(key, prompt(message)).expect("the agent accepts the prompt");
    }
}

impl Drop for Chats {
    fn drop(&mut self) {
        block_on(self.setup.state.agents.stop_all());
    }
}

fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

fn context(text: &str) -> HookAction {
    HookAction::AddContext { text: text.to_owned() }
}

/// The events of one type the chat's channel received.
fn events_of(received: &Mutex<Vec<Value>>, kind: &str) -> Vec<Value> {
    received
        .lock()
        .unwrap()
        .iter()
        .filter(|message| message["event"]["type"] == kind)
        .map(|message| message["event"].clone())
        .collect()
}

/// Waits for the third settle (the follow-up's run), then checks that no other follow-up reaches
/// the agent before "/done" and that no hook failed.
#[cfg(unix)]
fn ends_after_one_follow_up(chats: &Chats, key: &str, received: &Mutex<Vec<Value>>) {
    wait_until("the follow-up run to settle", || {
        events_of(received, "settled").len() == 3
    });
    std::thread::sleep(Duration::from_millis(300));
    chats.prompt(key, "/done");
    assert!(events_of(received, "notice").is_empty());
}

#[test]
fn the_first_prompt_of_a_new_chat_carries_the_session_start_context() {
    let Some(chats) = chats("omp_hooks_start.jsonl") else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );

    let (key, _) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Hello");
    chats.prompt(&key, "Again");

    chats.send(&key, AgentAction::NewSession).expect("a new session starts");
    chats.prompt(&key, "Fresh start");
}

#[test]
fn a_resumed_chat_gets_no_session_start_context() {
    let Some(chats) = chats("omp_hooks_plain.jsonl") else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );

    let request = AgentStartRequest {
        resume: Some(SessionRef {
            harness: HarnessId::Omp,
            id: "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57".to_owned(),
        }),
        ..start_request(&chats.setup.project)
    };
    let (key, _) = chats.start(request);

    chats.prompt(&key, "Hello");
}

#[test]
fn one_follow_up_goes_out_after_a_user_run_and_none_after_its_own_run() {
    let Some(chats) = chats("omp_hooks_follow_up.jsonl") else {
        return;
    };
    chats.hook(
        "Save notes",
        HookEvent::TurnEnd,
        HookAction::FollowUp {
            text: "Save what you learned.".to_owned(),
        },
        HookAuthor::Desktop,
        0,
    );

    let (key, received) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Fix the parser");

    wait_until("both runs to settle", || events_of(&received, "settled").len() == 2);

    // A second follow-up would reach the agent before this prompt, and fail its match.
    std::thread::sleep(Duration::from_millis(300));
    chats.prompt(&key, "/done");
    assert!(events_of(&received, "notice").is_empty());
}

#[test]
fn utility_agents_run_no_hooks() {
    let Some(chats) = chats("omp_hooks_plain.jsonl") else {
        return;
    };
    chats.hook(
        "Project notes",
        HookEvent::SessionStart,
        context("Read NOTES.md first."),
        HookAuthor::Desktop,
        0,
    );
    chats.hook(
        "Style",
        HookEvent::PromptSubmit,
        context("Be brief."),
        HookAuthor::Desktop,
        1,
    );

    let request = AgentStartRequest {
        utility: true,
        ..start_request(&chats.setup.project)
    };
    let (key, _) = chats.start(request);

    assert!(chats.state().hooks.chat(&key).is_none());
    chats.prompt(&key, "Hello");
}

#[test]
fn a_pending_hook_runs_only_once_approved() {
    let Some(chats) = chats("omp_hooks_pending.jsonl") else {
        return;
    };
    let pending = chats.hook(
        "Agent rule",
        HookEvent::PromptSubmit,
        context("Use tabs."),
        HookAuthor::Agent,
        0,
    );
    assert_eq!(pending.status, HookStatus::Pending);

    let (key, _) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Hello");

    let approved = block_on(hook_commands::approve(chats.state(), pending.id, pending.sha256)).expect("approved");
    assert_eq!(approved.status, HookStatus::Active);
    chats.prompt(&key, "Again");
}

#[cfg(unix)]
#[test]
fn a_command_hooks_output_reaches_the_prompt() {
    let Some(chats) = chats("omp_hooks_command.jsonl") else {
        return;
    };
    let command = |command: &str| HookAction::Command {
        command: command.to_owned(),
        timeout_secs: None,
    };
    chats.hook(
        "Event name",
        HookEvent::PromptSubmit,
        command("printf 'event=%s' \"$HARNESS_HOOK_EVENT\""),
        HookAuthor::Desktop,
        0,
    );
    chats.hook(
        "Broken",
        HookEvent::PromptSubmit,
        command("echo secret-output; exit 3"),
        HookAuthor::Desktop,
        1,
    );

    let (key, received) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Hello");

    wait_until("the notice", || !events_of(&received, "notice").is_empty());
    let notices = events_of(&received, "notice");
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0]["level"], "warning");
    assert_eq!(notices[0]["message"], "The hook \"Broken\" failed with exit code 3.");
}

#[test]
fn a_stopped_chat_is_forgotten() {
    let Some(chats) = chats("omp_hooks_plain.jsonl") else {
        return;
    };

    let (key, _) = chats.start(start_request(&chats.setup.project));
    assert!(chats.state().hooks.chat(&key).is_some());

    block_on(agent::stop(chats.state(), &key)).expect("stopped");
    assert!(chats.state().hooks.chat(&key).is_none());

    // A page reload forgets every chat.
    let (other, _) = chats.start(start_request(&chats.setup.project));
    chats.state().hooks.clear();
    assert!(chats.state().hooks.chat(&other).is_none());
}

#[cfg(unix)]
#[test]
fn a_message_sent_while_a_slow_tool_command_runs_belongs_to_the_next_run() {
    let Some(chats) = chats("omp_hooks_race.jsonl") else {
        return;
    };
    chats.hook(
        "Slow check",
        HookEvent::ToolEnd,
        HookAction::Command {
            command: "sleep 1; printf 'tool %s' \"$HARNESS_HOOK_TOOL\"".to_owned(),
            timeout_secs: None,
        },
        HookAuthor::Desktop,
        0,
    );

    let (key, received) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Fix the parser");

    // The run settled while its toolEnd command still sleeps; the user moves on at once.
    wait_until("the first settle", || events_of(&received, "settled").len() == 1);
    chats.prompt(&key, "Next");

    // A follow-up for the first run would reach the agent instead of the second run's, and fail
    // its match; the second run's own follow-up is what the agent expects.
    ends_after_one_follow_up(&chats, &key, &received);
}

#[cfg(unix)]
#[test]
fn a_settle_while_a_slow_prompt_submit_command_runs_ends_the_run_before_the_prompt() {
    let Some(chats) = chats("omp_hooks_slow_submit.jsonl") else {
        return;
    };
    let command = |command: &str| HookAction::Command {
        command: command.to_owned(),
        timeout_secs: None,
    };
    // Slow from the second prompt on, without output.
    chats.hook(
        "Slow submit",
        HookEvent::PromptSubmit,
        command("if [ -e .second ]; then sleep 1.5; fi; touch .second"),
        HookAuthor::Desktop,
        0,
    );
    chats.hook(
        "Tool name",
        HookEvent::ToolEnd,
        command("printf 'tool %s' \"$HARNESS_HOOK_TOOL\""),
        HookAuthor::Desktop,
        1,
    );

    let (key, received) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Fix the parser");

    // The first run settles while this prompt's command sleeps.
    chats.prompt(&key, "Next");
    assert_eq!(events_of(&received, "settled").len(), 1);

    // A follow-up for the first run ("tool read") would fail the agent's match.
    ends_after_one_follow_up(&chats, &key, &received);
}

#[test]
fn a_moved_chat_gets_the_hooks_of_its_new_project() {
    let Some(chats) = chats("omp_hooks_moved.jsonl") else {
        return;
    };
    let other = chats.setup.project.parent().unwrap().join("other");
    std::fs::create_dir_all(&other).unwrap();

    let draft = HookDraft {
        id: None,
        expected_sha256: None,
        name: "Other rules".to_owned(),
        description: None,
        scope: HookScope::Project,
        project: Some(other.to_string_lossy().into_owned()),
        event: HookEvent::PromptSubmit,
        matcher: None,
        action: context("Use the other rules."),
        enabled: true,
    };
    let store = chats.state().hooks.store().expect("a hooks folder");
    store
        .save(&draft, HookAuthor::Desktop, HookCapabilities::default(), UNIX_EPOCH)
        .expect("hook saved");
    chats.state().hooks.invalidate();

    let (key, _) = chats.start(start_request(&chats.setup.project));
    block_on(chat::moved(chats.state(), &key, other));

    chats.prompt(&key, "Hello");
}

#[cfg(unix)]
#[test]
fn exit_waits_for_session_end_commands_within_its_budget() {
    let Some(chats) = chats("omp_hooks_plain.jsonl") else {
        return;
    };
    let command = |command: &str| HookAction::Command {
        command: command.to_owned(),
        timeout_secs: None,
    };
    chats.hook(
        "Wrap up",
        HookEvent::SessionEnd,
        command("sleep 0.3; touch wrapped-up"),
        HookAuthor::Desktop,
        0,
    );

    chats.start(start_request(&chats.setup.project));
    block_on(chats.state().agents.stop_all());

    let stopped = block_on(chats.state().hooks.finish_chats(Duration::from_secs(3)));
    assert_eq!(stopped, 0);
    assert!(chats.setup.project.join("wrapped-up").exists());

    // A command that outlasts the budget is stopped.
    chats.hook(
        "Too slow",
        HookEvent::SessionEnd,
        command("sleep 5; touch too-late"),
        HookAuthor::Desktop,
        1,
    );
    chats.start(start_request(&chats.setup.project));
    block_on(chats.state().agents.stop_all());

    let stopped = block_on(chats.state().hooks.finish_chats(Duration::from_millis(500)));
    assert_eq!(stopped, 1);
}
