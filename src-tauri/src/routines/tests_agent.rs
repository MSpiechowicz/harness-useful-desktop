//! Agent routine runs against scripted agents (`fake_agent`, `omp_routine_*.jsonl`): the outcome,
//! excerpt and session tag of a run, its model, its time limit, hook follow-ups, a window attached
//! to its chat, a page reload, questions nobody answers or a window answers, a full session
//! manager, and the "Routine" tag in Sessions.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use harness_business::env::Env;
use harness_business::model::{
    AgentSessionInfo, HarnessId, HookAction, HookAuthor, HookEvent, NoticeLevel, RoutineAction, RoutineModel,
    RoutineRun, RoutineWhen, RunSession, RunStatus,
};
use tauri::async_runtime::block_on;

use super::agent_run::{AgentTiming, FULL_MESSAGE, RoutineAgents, StateHost};
use super::attention::UNANSWERED_MESSAGE;
use super::job::{AgentsUnavailable, STOPPED_MESSAGE};
use super::relay::RelaySink;
use super::tests::{draft, ended, runs_of, save};
use crate::commands::routines::chat::attach;
use crate::commands::{agent, sessions};
use crate::lifecycle::page_reloaded;
use crate::state::AppState;
use crate::test_support::{
    AgentSetup, agent_setup_for, agent_setup_opencode2, assert_envelopes_in_order, closable_channel,
    collecting_channel, event_types, fake_agent, save_global_hook, start_request, wait_until,
};

/// The OMP session every scenario reports (`omp_startup.jsonl`).
pub(super) const SESSION_ID: &str = "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";

/// Its fixture file in `business/tests/fixtures/sessions/omp`.
pub(super) const SESSION_FILE: &str = "2026-10-09T10-00-00-000Z_0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57.jsonl";

/// The last reply of `omp_routine_reply.jsonl`.
pub(super) const REPLY: &str = "Checked the build: all green.";

struct Host(Arc<AppState>);

impl StateHost for Host {
    fn app_state(&self) -> &AppState {
        &self.0
    }
}

/// Short waits and retries; minutes as written.
pub(super) fn timing() -> AgentTiming {
    AgentTiming {
        minute: Duration::from_secs(60),
        full_retry: Duration::from_millis(50),
        full_limit: Duration::from_secs(10),
        ui_wait: Duration::from_secs(30),
        idle_poll: Duration::from_millis(20),
    }
}

/// An app state whose agents play one scenario, with agent routines on.
pub(super) struct Fixture {
    pub(super) state: Arc<AppState>,
    pub(super) setup: AgentSetup,
}

impl Fixture {
    /// `None` (the test skips) when `fake_agent` is not built.
    pub(super) fn new(scenario: &str, timing: AgentTiming) -> Option<Self> {
        Self::of(HarnessId::Omp, scenario, timing)
    }

    /// Agents of `harness` that play `scenario`; `None` (the test skips) when `fake_agent` is not
    /// built.
    pub(super) fn of(harness: HarnessId, scenario: &str, timing: AgentTiming) -> Option<Self> {
        let binary = fake_agent()?;

        Some(Self::with_setup(agent_setup_for(&binary, harness, scenario), timing))
    }

    /// OpenCode 2 agents that play `scenario` on a scripted OpenCode 2 server; `None` (the test
    /// skips) when `fake_agent` is not built.
    pub(super) fn opencode2(scenario: &str, timing: AgentTiming) -> Option<Self> {
        let binary = fake_agent()?;

        Some(Self::with_setup(agent_setup_opencode2(&binary, scenario), timing))
    }

    fn with_setup(mut setup: AgentSetup, timing: AgentTiming) -> Self {
        let unused = AppState::new(setup.project.join("unused"), Env::for_tests(&setup.project));
        let state = Arc::new(std::mem::replace(&mut setup.state, unused));

        let fixture = Self { state, setup };
        fixture.install(timing);

        fixture
    }

    pub(super) fn install(&self, timing: AgentTiming) {
        let host = Arc::new(Host(self.state.clone()));

        self.state
            .routines
            .set_agent_runner(Arc::new(RoutineAgents::new(host, timing)));
    }

    /// Saves a global agent routine named "Nightly check" (approved) and runs it now; answers its
    /// id and the run's.
    pub(super) fn run(&self, action: RoutineAction) -> (String, String) {
        let view = save(
            &self.state,
            &draft("Nightly check", RoutineWhen::AppStart, action),
            HookAuthor::Desktop,
        );

        (view.id.clone(), self.run_again(&view.id))
    }

    /// Runs routine `id` now; answers the run's id.
    pub(super) fn run_again(&self, id: &str) -> String {
        let run = block_on(self.state.routines.run_now(id))
            .expect("the routine runs")
            .expect("it starts at once");

        run.record.id
    }

    /// The only run of routine `id`, once it ended.
    pub(super) fn ended(&self, id: &str) -> RoutineRun {
        ended(&self.state, id, 1).remove(0)
    }

    /// The relay of run `run_id`, once its chat started.
    pub(super) fn relay(&self, run_id: &str) -> Arc<RelaySink> {
        let relays = self.state.routines.relays();
        wait_until("the run's chat", || {
            relays.get(run_id).is_some_and(|relay| relay.key().is_some())
        });

        relays.get(run_id).expect("the relay")
    }

    /// Runs `action` now with a window attached to its chat, which stays open, and waits until the
    /// window shows the agent at work on the prompt (an end from then on is not a refused prompt,
    /// however slow the machine); answers the routine's id, the run's, the chat and the window's
    /// open flag.
    pub(super) fn run_shown(&self, action: RoutineAction) -> (String, String, AgentSessionInfo, Arc<AtomicBool>) {
        let (id, run_id) = self.run(action);
        self.relay(&run_id);

        let (window, open, heard) = closable_channel(true);
        let info = attach(&self.state, &run_id, window).expect("the chat opens");

        wait_until("the agent to take the prompt", || {
            event_types(&heard).iter().any(|kind| kind == "agentStart")
        });

        (id, run_id, info, open)
    }

    /// Fills the session manager with chats of the project; answers their keys.
    pub(super) fn fill_sessions(&self) -> Vec<String> {
        (0..harness_business::agent::MAX_SESSIONS)
            .map(|_| {
                let (channel, _) = collecting_channel();
                let started = block_on(agent::start(&self.state, start_request(&self.setup.project), channel));

                started.expect("a chat starts").key
            })
            .collect()
    }

    /// Stops run `run_id` of routine `id` as the user does; checks it ended stopped, its agent
    /// too.
    pub(super) fn stop_by_user(&self, id: &str, run_id: &str) {
        self.state.routines.stop_run(run_id).expect("stopped");

        let run = self.ended(id);
        assert_eq!(run.record.status, RunStatus::Stopped);
        assert_eq!(run.record.message.as_deref(), Some(STOPPED_MESSAGE));
        wait_until("the agent to stop", || self.chats().is_empty());
    }

    /// Runs `action` now and checks that its question went unanswered: the run needs attention,
    /// the view heard that it needed the user, and the agent stopped. Answers the run's relay.
    pub(super) fn run_unanswered(&self, action: RoutineAction) -> Arc<RelaySink> {
        let (listener, events) = collecting_channel();
        self.state.routines.listen(listener);

        let (id, run_id) = self.run(action);
        let relay = self.relay(&run_id);
        let run = self.ended(&id);

        assert_eq!(run.record.status, RunStatus::NeedsAttention);
        assert_eq!(run.record.message.as_deref(), Some(UNANSWERED_MESSAGE));
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|event| event["kind"] == "needsYou" && event["runId"] == run_id.as_str())
        );
        wait_until("the agent to stop", || self.chats().is_empty());

        relay
    }

    pub(super) fn chats(&self) -> Vec<AgentSessionInfo> {
        self.state.agents.manager().list()
    }

    pub(super) fn needs_you(&self, id: &str) -> bool {
        self.state.routines.with_needs_you(runs_of(&self.state, id))[0].needs_you
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The runner holds the state: let go of it.
        self.state.routines.set_agent_runner(Arc::new(AgentsUnavailable));
        block_on(self.state.agents.stop_all());
    }
}

/// An OMP agent action that sends `prompt`, with `model` and `max_minutes`.
pub(super) fn agent_with(prompt: &str, model: Option<(&str, &str)>, max_minutes: Option<u32>) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Omp,
        model: model.map(|(provider, id)| RoutineModel {
            provider: provider.to_owned(),
            id: id.to_owned(),
            thinking: None,
        }),
        prompt: prompt.to_owned(),
        approval_mode: None,
        trust_once: false,
        max_minutes,
    }
}

/// An OMP agent action that sends `prompt`.
pub(super) fn agent(prompt: &str) -> RoutineAction {
    agent_with(prompt, None, None)
}

#[test]
fn a_run_ends_when_its_agent_settled_with_the_last_reply_and_tags_its_session() {
    let Some(fixture) = Fixture::new("omp_basic.jsonl", timing()) else {
        return;
    };

    let (id, run_id) = fixture.run(agent("Make `parse` handle empty input."));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(
        run.record.excerpt.as_deref(),
        Some("Done: `parse` now returns `None` for blank input.")
    );
    assert_eq!(
        run.record.session,
        Some(RunSession {
            harness: HarnessId::Omp,
            id: SESSION_ID.to_owned(),
        })
    );
    wait_until("the agent to stop", || fixture.chats().is_empty());

    let tags = fixture.state.routines.session_tags();
    let tag = tags.get(HarnessId::Omp, SESSION_ID).expect("the session is tagged");
    assert_eq!(
        (tag.routine_id.as_str(), tag.run_id.as_str()),
        (id.as_str(), run_id.as_str())
    );
    assert_eq!(tag.name, "Nightly check");

    // Sessions lists the workspace of global routines, with the tag.
    let workspace = fixture.state.routines.workspace().expect("the workspace exists");
    let omp = fixture.state.registry.get(HarnessId::Omp).expect("OMP");
    let folder = omp.sessions_dir(&fixture.state.env, &workspace);
    std::fs::create_dir_all(&folder).expect("create the session folder");
    let fixture_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../business/tests/fixtures/sessions/omp")
        .join(SESSION_FILE);
    std::fs::copy(fixture_file, folder.join(SESSION_FILE)).expect("copy the session");

    let listed = block_on(sessions::list(
        &fixture.state,
        HarnessId::Omp,
        workspace.to_string_lossy().into_owned(),
    ))
    .expect("the workspace is listed");
    let session = listed
        .iter()
        .find(|summary| summary.session.id == SESSION_ID)
        .expect("the session");
    assert_eq!(session.routine.as_ref(), Some(tag));
}

#[test]
fn a_run_switches_to_its_model_and_fails_when_the_agent_has_not_got_it() {
    let Some(fixture) = Fixture::new("omp_routine_model.jsonl", timing()) else {
        return;
    };

    let (id, _) = fixture.run(agent_with("Check the build", Some(("openai", "gpt-5")), None));
    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some(REPLY));

    let Some(missing) = Fixture::new("omp_routine_model_missing.jsonl", timing()) else {
        return;
    };

    let (id, _) = missing.run(agent_with("Check the build", Some(("openai", "gpt-0")), None));
    let run = missing.ended(&id);
    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(
        run.record.message.as_deref(),
        Some("The model openai/gpt-0 is not available")
    );
    wait_until("the agent to stop", || missing.chats().is_empty());
}

#[test]
fn a_run_past_its_time_limit_is_stopped_and_fails() {
    let short = AgentTiming {
        minute: Duration::from_millis(400),
        ..timing()
    };
    let Some(fixture) = Fixture::new("omp_routine_hang.jsonl", short) else {
        return;
    };

    let (id, _) = fixture.run(agent_with("Check the build", None, Some(1)));

    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(run.record.message.as_deref(), Some("Stopped after 1 minute"));
    wait_until("the agent to stop", || fixture.chats().is_empty());
}

#[test]
fn a_run_waits_for_the_follow_up_of_its_hooks() {
    let Some(fixture) = Fixture::new("omp_routine_follow_up.jsonl", timing()) else {
        return;
    };
    save_global_hook(
        &fixture.state,
        "Save notes",
        HookEvent::TurnEnd,
        HookAction::FollowUp {
            text: "Save what you learned.".to_owned(),
        },
        HookAuthor::Desktop,
        0,
    );

    let (id, _) = fixture.run(agent("Fix the parser"));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some("Notes saved."));
}

#[test]
fn an_attached_window_replays_the_chat_in_order_and_keeps_it_until_it_leaves() {
    let Some(fixture) = Fixture::new("omp_routine_slow.jsonl", timing()) else {
        return;
    };

    let (id, run_id) = fixture.run(agent("Check the build"));
    let relay = fixture.relay(&run_id);

    let (window, open, received) = closable_channel(true);
    let info = attach(&fixture.state, &run_id, window).expect("the chat opens");
    assert_eq!(Some(info.key.clone()), relay.key());

    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);

    let seen = event_types(&received);
    assert_eq!(seen[..3], ["ready", "state", "commandsUpdated"]);
    assert!(seen.contains(&"settled".to_owned()), "{seen:?}");
    assert_envelopes_in_order(&received, &info.key);

    // The window keeps the chat of the ended run.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(fixture.chats().len(), 1);

    // Once the window is gone (seen at the next event) the agent stops.
    open.store(false, Ordering::Release);
    fixture
        .state
        .agents
        .manager()
        .notice(&info.key, NoticeLevel::Info, "ping".to_owned())
        .expect("the chat runs");
    wait_until("the agent to stop", || fixture.chats().is_empty());
}

#[test]
fn a_window_that_leaves_a_running_chat_leaves_its_agent_running() {
    let Some(fixture) = Fixture::new("omp_routine_hang.jsonl", timing()) else {
        return;
    };

    let (id, run_id, info, open) = fixture.run_shown(agent("Check the build"));

    open.store(false, Ordering::Release);
    let manager = fixture.state.agents.manager();
    manager
        .notice(&info.key, NoticeLevel::Info, "ping".to_owned())
        .expect("the chat runs");
    wait_until("the window to leave", || !fixture.relay(&run_id).is_attached());

    assert!(manager.info(&info.key).is_ok(), "the agent keeps running");
    assert!(fixture.state.routines.running().contains(&id));

    fixture.stop_by_user(&id, &run_id);
}

#[test]
fn a_page_reload_stops_the_chats_but_those_of_routine_runs() {
    let Some(fixture) = Fixture::new("omp_routine_hang.jsonl", timing()) else {
        return;
    };

    let (id, run_id) = fixture.run(agent("Check the build"));
    let routine_chat = fixture.relay(&run_id).key().expect("the chat's key");

    let (channel, _) = collecting_channel();
    block_on(agent::start(
        &fixture.state,
        start_request(&fixture.setup.project),
        channel,
    ))
    .expect("a chat starts");
    assert_eq!(fixture.chats().len(), 2);

    page_reloaded(&fixture.state);

    wait_until("the page's chat to stop", || fixture.chats().len() == 1);
    assert_eq!(fixture.chats()[0].key, routine_chat);
    assert!(fixture.state.hooks.chat(&routine_chat).is_some(), "its hooks go on");
    assert!(fixture.state.routines.running().contains(&id));

    fixture.state.routines.stop_run(&run_id).expect("stopped");
    assert_eq!(fixture.ended(&id).record.status, RunStatus::Stopped);
}

#[test]
fn a_question_nobody_answers_is_cancelled_and_the_run_needs_attention() {
    let quick = AgentTiming {
        ui_wait: Duration::from_millis(300),
        ..timing()
    };
    let Some(fixture) = Fixture::new("omp_routine_question.jsonl", quick) else {
        return;
    };
    let relay = fixture.run_unanswered(agent("Run the tests"));

    // The agent got "cancelled" for its question (its scenario says so in a notice) and exited.
    wait_until("the agent's last events", || {
        let (window, received) = collecting_channel();
        relay.attach(window).expect("the chat's events");
        let seen = event_types(&received);

        seen.contains(&"notice".to_owned()) && seen.last().map(String::as_str) == Some("exited")
    });
}

#[test]
fn a_run_waits_for_a_free_session_and_gives_up_after_its_limit() {
    let give_up_soon = AgentTiming {
        full_limit: Duration::from_millis(300),
        ..timing()
    };
    let Some(fixture) = Fixture::new("omp_basic.jsonl", give_up_soon) else {
        return;
    };

    let keys = fixture.fill_sessions();

    let (id, _) = fixture.run(agent("Make `parse` handle empty input."));
    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(run.record.message.as_deref(), Some(FULL_MESSAGE));

    fixture.install(timing());
    fixture.run_again(&id);
    std::thread::sleep(Duration::from_millis(300));
    assert!(fixture.state.routines.running().contains(&id), "it waits");
    assert_eq!(fixture.setup.spawner.spawned(), harness_business::agent::MAX_SESSIONS);

    block_on(agent::stop(&fixture.state, &keys[0])).expect("a chat closes");

    let runs = ended(&fixture.state, &id, 2);
    assert_eq!(runs[0].record.status, RunStatus::Ok, "{:?}", runs[0].record.message);
}
