//! The alerts of chats against scripted agents: which ones reach [`ChatAlerts`] and which never do.
//! The engine's rules are tested in `hooks_alerts.rs` of the business crate, these check that the
//! shell raises what it decides, and only for the chats the user started and kept.

use std::sync::Arc;
use std::time::Duration;

use harness_business::agent::{AgentManager, EventSink, SessionOptions};
use harness_business::model::{AgentEnvelope, AgentEvent, ChatAlertKind, HarnessId, UiOutcome, UiResponse};
use tauri::async_runtime::block_on;

use super::tests::{Chats, chats_of};
use crate::agent_sink::ChatSink;
use crate::agents::Agents;
use crate::commands::agent::{self, AuthorizedFolder};
use crate::test_support::{start_request, wait_until};

const FAILED: ChatAlertKind = ChatAlertKind::Failed;

fn chats(scenario: &str) -> Option<Chats> {
    chats_of(HarnessId::Omp, scenario)
}

fn raised(chats: &Chats) -> Vec<(String, ChatAlertKind)> {
    chats.state().chat_alerts.raised()
}

fn kinds(chats: &Chats) -> Vec<ChatAlertKind> {
    raised(chats).into_iter().map(|(_, kind)| kind).collect()
}

/// Waits until every chat's hook task handled its agent's exit.
fn tasks_end(chats: &Chats) {
    let stopped = block_on(chats.state().hooks.finish_chats(Duration::from_secs(5)));

    assert_eq!(stopped, 0, "every hook task ends with its agent");
}

/// Opens a chat whose agent ignores the end of its input, so every way of ending it kills the
/// agent (an unclean exit), and ends it the way `end` does: nothing may be raised.
fn ended_by(end: impl FnOnce(&Chats, &str)) {
    let Some(mut chats) = chats("omp_hang.jsonl") else {
        return;
    };

    let options = SessionOptions {
        coalesce_window: Duration::ZERO,
        stop_grace: Duration::from_millis(100),
        ..SessionOptions::default()
    };
    let manager = AgentManager::with_options(options);
    chats.setup.state.agents = Arc::new(Agents::with(manager, chats.setup.spawner.clone()));

    let (key, _) = chats.start(start_request(&chats.setup.project));
    end(&chats, &key);

    block_on(chats.state().agents.stop_all());
    tasks_end(&chats);

    assert_eq!(raised(&chats), Vec::new());
    assert!(
        !chats.state().agents.manager().ended_by_user(&key),
        "the chat's task forgets it was ended"
    );
}

#[test]
fn a_crashed_agent_raises_failed_once() {
    let Some(chats) = chats("omp_crash.jsonl") else {
        return;
    };

    let (key, _) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Refactor the config loader.");

    wait_until("the crash alert", || !raised(&chats).is_empty());
    tasks_end(&chats);

    assert_eq!(raised(&chats), vec![(key, FAILED)]);
}

/// A chat the user watches, whose sink looks at the manager's list as soon as the agent's exit
/// arrives: the manager forgets an exited agent at such a look, before the hook task handles the
/// exit (the sink is handed every event first).
struct PruningSink {
    agents: Arc<Agents>,
}

impl EventSink for PruningSink {
    fn emit(&self, envelope: AgentEnvelope) {
        if matches!(envelope.event, AgentEvent::Exited { .. }) {
            self.agents.manager().list();
        }
    }
}

impl ChatSink for PruningSink {
    fn is_closed(&self) -> bool {
        false
    }

    fn raises_alerts(&self) -> bool {
        true
    }
}

#[test]
fn a_crash_raises_failed_although_the_manager_forgot_the_agent_before_the_exit_was_handled() {
    let Some(chats) = chats("omp_crash.jsonl") else {
        return;
    };

    let sink = Arc::new(PruningSink {
        agents: chats.state().agents.clone(),
    });
    let folder = AuthorizedFolder::of_routine(&chats.setup.project);
    let info = block_on(agent::start_with(
        chats.state(),
        start_request(&chats.setup.project),
        sink,
        folder,
    ))
    .expect("the chat starts");

    chats.prompt(&info.key, "Refactor the config loader.");
    tasks_end(&chats);

    assert!(
        chats.state().agents.manager().info(&info.key).is_err(),
        "the manager forgot the crashed agent"
    );
    assert_eq!(raised(&chats), vec![(info.key, FAILED)]);
}

#[test]
fn stopping_the_chat_raises_nothing() {
    ended_by(|chats, key| {
        block_on(agent::stop(chats.state(), key)).expect("the chat stops");
    });
}

#[test]
fn a_page_reload_raises_nothing() {
    ended_by(|chats, _| crate::lifecycle::page_reloaded(chats.state()));
}

#[test]
fn the_app_quitting_raises_nothing() {
    ended_by(|chats, _| block_on(chats.state().agents.stop_all()));
}

/// A chat like a routine run's: nothing shows it, and it does not tell the user anything.
struct QuietSink;

impl EventSink for QuietSink {
    fn emit(&self, _: AgentEnvelope) {}
}

impl ChatSink for QuietSink {
    fn is_closed(&self) -> bool {
        false
    }
}

#[test]
fn a_chat_that_raises_no_alerts_stays_silent_when_it_crashes() {
    let Some(chats) = chats("omp_crash.jsonl") else {
        return;
    };

    let request = start_request(&chats.setup.project);
    let folder = AuthorizedFolder::of_routine(&chats.setup.project);
    let info =
        block_on(agent::start_with(chats.state(), request, Arc::new(QuietSink), folder)).expect("the chat starts");

    chats.prompt(&info.key, "Refactor the config loader.");
    tasks_end(&chats);

    assert_eq!(raised(&chats), Vec::new());
}

#[test]
fn a_run_that_ends_raises_finished_and_a_failed_run_raises_failed() {
    let Some(plain) = chats("omp_hooks_follow_up.jsonl") else {
        return;
    };
    let (key, _) = plain.start(start_request(&plain.setup.project));
    plain.prompt(&key, "Fix the parser");

    wait_until("the alert", || !raised(&plain).is_empty());
    assert_eq!(raised(&plain), vec![(key, ChatAlertKind::Finished)]);

    let Some(failing) = chats("omp_error.jsonl") else {
        return;
    };
    let (key, _) = failing.start(start_request(&failing.setup.project));
    failing.prompt(&key, "Summarize the open TODOs.");

    wait_until("the alert", || !raised(&failing).is_empty());
    assert_eq!(raised(&failing), vec![(key, FAILED)]);
}

#[test]
fn a_run_with_a_follow_up_raises_one_finished_for_the_whole_chain() {
    let Some(chats) = chats("omp_hooks_follow_up.jsonl") else {
        return;
    };
    let (key, received) = chats.start_after_follow_up_hook();

    wait_until("both runs to settle", || {
        super::tests::events_of(&received, "settled").len() == 2
    });
    wait_until("the alert", || !raised(&chats).is_empty());

    // A second alert would come with the events still to be handled.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(raised(&chats), vec![(key, ChatAlertKind::Finished)]);
}

#[test]
fn a_dialog_raises_needs_you_and_the_finished_run_after_it_raises_finished() {
    let Some(chats) = chats("omp_approval.jsonl") else {
        return;
    };

    let (key, _) = chats.start(start_request(&chats.setup.project));
    chats.prompt(&key, "Run the tests.");

    wait_until("the dialog alert", || !raised(&chats).is_empty());
    assert_eq!(kinds(&chats), vec![ChatAlertKind::NeedsYou]);

    let answer = UiResponse {
        id: "approval-1".to_owned(),
        outcome: UiOutcome::Value {
            value: "Approve".to_owned(),
        },
    };
    block_on(agent::respond(chats.state(), &key, answer)).expect("the answer reaches the agent");

    wait_until("the run to end", || raised(&chats).len() == 2);
    assert_eq!(kinds(&chats), vec![ChatAlertKind::NeedsYou, ChatAlertKind::Finished]);
}
