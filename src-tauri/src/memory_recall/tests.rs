//! Recall in chats against scripted agents (`omp_recall_*.jsonl`, whose `prompt` steps match the
//! exact text the agent receives; a mismatch makes the agent exit, failing the send) and scripted
//! `harness-memory` servers (`mcp_memory_hits.jsonl`, `mcp_timeout.jsonl`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use harness_business::ecosystem::MemoryRecall;
use harness_business::model::{AgentStartRequest, HarnessId, ImageAttachment, SessionRef, StreamingBehavior};
use tauri::async_runtime::block_on;

use super::*;
use crate::commands::agent;
use crate::state::MemoryServer;
use crate::test_support::{
    AgentSetup, ScenarioSpawner, agent_setup, collecting_channel, fake_agent, start_request, wait_until,
};

/// Finds the notes of `mcp_memory_hits.jsonl` (query `uploader code`).
const QUESTION: &str = "Where is the uploader code?";

/// A chat app state whose recalls run the `memory` scenario.
struct Chats {
    setup: AgentSetup,
    memory: Arc<ScenarioSpawner>,
}

/// `None` (the test skips) when `fake_agent` is not built.
fn chats(agent_scenario: &str, memory_scenario: &str) -> Option<Chats> {
    chats_with(agent_scenario, memory_scenario, |_| {})
}

/// [`chats`] with the state changed by `change` before any chat starts.
fn chats_with(agent_scenario: &str, memory_scenario: &str, change: impl FnOnce(&mut AppState)) -> Option<Chats> {
    let binary = fake_agent()?;
    let mut setup = agent_setup(&binary, agent_scenario);
    let memory = ScenarioSpawner::mcp(&binary, memory_scenario);

    setup.state.memory_recall = MemoryRecall::new(memory.clone());
    setup.state.memory_server = MemoryServer::fixed(PathBuf::from("/opt/harness/harness-memory"));
    change(&mut setup.state);

    Some(Chats { setup, memory })
}

impl Chats {
    fn state(&self) -> &AppState {
        &self.setup.state
    }

    fn start(&self, request: AgentStartRequest) -> String {
        let (channel, _) = collecting_channel();

        block_on(agent::start(self.state(), request, channel))
            .expect("the chat starts")
            .key
    }

    fn new_chat(&self) -> String {
        self.start(start_request(&self.setup.project))
    }

    /// Sends `action`; fails when the agent did not accept it (a `match` failed).
    fn send(&self, key: &str, action: AgentAction) {
        block_on(send(self.state(), key, action)).expect("the agent accepts the action");
    }

    fn prompt(&self, key: &str, message: &str) {
        self.send(key, prompt(message, None));
    }

    /// Waits until the chat's warm-up has finished (the server is ready, or failed to start).
    fn wait_warm(&self, key: &str) {
        wait_until("the warm server", || {
            let Some(entry) = self.state().recall.slot(key) else {
                return false;
            };
            let Ok(slot) = entry.try_lock() else {
                return false;
            };

            let warm = slot.warm.as_ref();
            warm.is_some_and(|warm| !matches!(*lock(&warm.server), Warm::Starting))
        });
    }

    /// Whether the chat's slot holds a warm server that is starting or ready.
    fn has_warm(&self, key: &str) -> bool {
        let entry = self.state().recall.slot(key).expect("a tracked chat");
        let slot = entry.try_lock().expect("the slot is free");

        slot.warm.as_ref().is_some_and(WarmTask::is_alive)
    }

    /// Asserts that chat `key` is no longer tracked and waits for its (first) warm server to exit.
    fn assert_forgotten(&self, key: &str) {
        assert!(self.state().recall.slot(key).is_none());

        #[cfg(target_os = "linux")]
        {
            let server = self.memory.pids()[0];
            wait_until("the warm server to exit", || !is_running(server));
        }
    }

    /// Whether chat `key` is armed: its next message decides on a recall.
    fn is_armed(&self, key: &str) -> bool {
        let entry = self.state().recall.slot(key).expect("a tracked chat");
        let slot = entry.try_lock().expect("the slot is free");

        slot.armed
    }

    fn set_prefs(&self, change: impl FnOnce(&mut harness_business::model::AppPrefs)) {
        self.state()
            .prefs
            .with(|store| store.update(change).expect("prefs saved"));
    }
}

impl Drop for Chats {
    fn drop(&mut self) {
        block_on(self.setup.state.agents.stop_all());
    }
}

fn prompt(message: &str, streaming_behavior: Option<StreamingBehavior>) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior,
    }
}

/// `action` (a prompt) with an image the validation refuses before anything reaches the agent.
fn with_unsupported_image(action: AgentAction) -> AgentAction {
    let AgentAction::Prompt {
        message,
        streaming_behavior,
        ..
    } = action
    else {
        panic!("a prompt");
    };

    let image = ImageAttachment {
        data: "Qk0=".to_owned(),
        mime_type: "image/bmp".to_owned(),
    };

    AgentAction::Prompt {
        message,
        images: vec![image],
        streaming_behavior,
    }
}

/// Whether process `pid` runs: it exists and is not a zombie waiting to be reaped.
#[cfg(target_os = "linux")]
fn is_running(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };

    // `pid (comm) state …`; the command name may hold spaces and parentheses.
    let state = stat
        .rfind(')')
        .and_then(|end| stat[end + 1..].split_whitespace().next());

    !matches!(state, Some("Z" | "X") | None)
}

#[test]
fn the_first_prompt_of_a_new_chat_gets_the_notes_from_the_warm_server() {
    let Some(chats) = chats("omp_recall_first.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    let image = ImageAttachment {
        data: "iVBORw0KGgo=".to_owned(),
        mime_type: "image/png".to_owned(),
    };
    let first = AgentAction::Prompt {
        message: QUESTION.to_owned(),
        images: vec![image],
        streaming_behavior: None,
    };
    chats.send(&key, first);
    chats.prompt(&key, "And the tests?");

    assert_eq!(chats.memory.spawned(), 1, "the warm server answered");
}

#[test]
fn resumed_and_utility_chats_send_prompts_as_written_without_a_server() {
    let Some(chats) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let resumed = chats.start(AgentStartRequest {
        resume: Some(SessionRef {
            harness: HarnessId::Omp,
            id: "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57".to_owned(),
        }),
        ..start_request(&chats.setup.project)
    });
    let utility = chats.start(AgentStartRequest {
        utility: true,
        ..start_request(&chats.setup.project)
    });

    chats.prompt(&resumed, QUESTION);
    chats.prompt(&utility, QUESTION);

    assert_eq!(chats.memory.spawned(), 0);
}

#[test]
fn no_server_starts_while_either_pref_is_off() {
    let Some(chats) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };

    chats.set_prefs(|prefs| prefs.memory_recall = false);
    let recall_off = chats.new_chat();
    chats.prompt(&recall_off, QUESTION);

    chats.set_prefs(|prefs| {
        prefs.memory_recall = true;
        prefs.memory_enabled = false;
    });
    let memory_off = chats.new_chat();
    chats.prompt(&memory_off, QUESTION);

    assert_eq!(chats.memory.spawned(), 0);
}

#[test]
fn a_command_or_a_queued_prompt_first_disarms_the_chat() {
    let Some(slash) = chats("omp_recall_slash.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = slash.new_chat();
    slash.prompt(&key, "/review");
    slash.prompt(&key, QUESTION);

    let Some(queued) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = queued.new_chat();
    queued.send(&key, prompt(QUESTION, Some(StreamingBehavior::Steer)));
    queued.prompt(&key, QUESTION);
}

#[test]
fn a_silent_server_leaves_the_prompt_unchanged_within_the_limit() {
    let limit = Duration::from_millis(200);
    let Some(chats) = chats_with("omp_recall_plain.jsonl", "mcp_timeout.jsonl", |state| {
        state.memory_recall.timeout = limit;
    }) else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    let started = Instant::now();
    chats.prompt(&key, QUESTION);

    let elapsed = started.elapsed();
    assert!(elapsed >= limit, "gave up early: {elapsed:?}");
}

#[test]
fn a_new_session_arms_the_chat_again() {
    let Some(chats) = chats("omp_recall_new.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();
    chats.send(&key, prompt(QUESTION, Some(StreamingBehavior::FollowUp)));

    chats.send(&key, AgentAction::NewSession);
    chats.prompt(&key, QUESTION);
}

#[test]
fn a_utility_agents_new_session_does_not_arm_it() {
    let Some(chats) = chats("omp_recall_utility_new.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let utility = chats.start(AgentStartRequest {
        utility: true,
        ..start_request(&chats.setup.project)
    });

    chats.send(&utility, AgentAction::NewSession);
    chats.prompt(&utility, QUESTION);

    assert_eq!(chats.memory.spawned(), 0);
}

#[test]
fn a_first_prompt_refused_before_the_agent_arms_the_chat_again() {
    let Some(chats) = chats("omp_recall_once.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    let refused = block_on(send(
        chats.state(),
        &key,
        with_unsupported_image(prompt(QUESTION, None)),
    ));
    assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");

    chats.prompt(&key, QUESTION);
    assert_eq!(chats.memory.spawned(), 2, "the retry started its own server");
}

#[test]
fn a_first_prompt_the_agent_refused_is_not_recalled_again() {
    let Some(chats) = chats("omp_recall_retry.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    let refused = block_on(send(chats.state(), &key, prompt(QUESTION, None)));
    assert!(matches!(refused, Err(BusinessError::Protocol(_))), "{refused:?}");

    assert!(!chats.is_armed(&key), "the agent received the recalled prompt");
}

#[test]
fn a_first_steer_refused_before_the_agent_disarms_the_chat() {
    let Some(chats) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();

    let steer = with_unsupported_image(prompt(QUESTION, Some(StreamingBehavior::Steer)));
    let refused = block_on(send(chats.state(), &key, steer));
    assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");

    chats.prompt(&key, QUESTION);
}

#[test]
fn an_idle_warm_server_stops_and_the_recall_starts_its_own() {
    let Some(chats) = chats_with("omp_recall_once.jsonl", "mcp_memory_hits.jsonl", |state| {
        state.recall = Arc::new(RecallGate::new(Duration::from_millis(200), WARM_MAX));
    }) else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    wait_until("the idle server to stop", || !chats.has_warm(&key));
    #[cfg(target_os = "linux")]
    {
        let server = chats.memory.pids()[0];
        wait_until("the idle server to exit", || !is_running(server));
    }

    chats.prompt(&key, QUESTION);
    assert_eq!(chats.memory.spawned(), 2);
}

#[test]
fn arming_beyond_the_limit_stops_the_oldest_warm_server() {
    let Some(chats) = chats_with("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl", |state| {
        state.recall = Arc::new(RecallGate::new(WARM_IDLE, 2));
    }) else {
        return;
    };
    let keys: Vec<String> = (0..3)
        .map(|_| {
            let key = chats.new_chat();
            chats.wait_warm(&key);
            key
        })
        .collect();

    assert!(!chats.has_warm(&keys[0]), "the oldest server stopped");
    assert!(chats.has_warm(&keys[1]) && chats.has_warm(&keys[2]));
    #[cfg(target_os = "linux")]
    {
        let pids = chats.memory.pids();
        wait_until("the oldest server to exit", || !is_running(pids[0]));
        assert!(is_running(pids[1]) && is_running(pids[2]));
    }
}

#[test]
fn a_chat_that_is_gone_is_not_tracked() {
    let Some(chats) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };

    track(chats.state(), "gone", &chats.setup.project.to_string_lossy(), true);

    assert!(chats.state().recall.slot("gone").is_none());
}

#[test]
fn a_page_reload_forgets_every_chat_and_kills_their_servers() {
    let Some(chats) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    chats.state().recall.clear();

    chats.assert_forgotten(&key);
}

#[test]
fn a_fork_disarms_the_chat() {
    let Some(chats) = chats("omp_recall_fork.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();

    chats.send(
        &key,
        AgentAction::Fork {
            entry_id: "entry-1".to_owned(),
        },
    );
    chats.prompt(&key, QUESTION);
}

#[test]
fn stopping_a_chat_kills_its_warm_server() {
    let Some(chats) = chats("omp_recall_plain.jsonl", "mcp_memory_hits.jsonl") else {
        return;
    };
    let key = chats.new_chat();
    chats.wait_warm(&key);

    block_on(agent::stop(chats.state(), &key)).expect("the chat stops");

    chats.assert_forgotten(&key);
}
