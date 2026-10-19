//! How agent runs end with a window around: a stop ends the agent even while a window shows the
//! chat, an agent that exits early fails the run, the time limit counts from the chat's start, a
//! window attaches only to a chat that still runs, and a notification click opens only the run it
//! names.

use std::collections::HashMap;
use std::time::Duration;

use harness_business::agent::EventSink;
use harness_business::model::{AgentEnvelope, AgentEvent, IpcErrorCode, RunStatus};
use serde_json::Value;
use tauri::async_runtime::block_on;

use super::agent_run::{AGENT_GONE, AgentTiming};
use super::notify::{Alert, AlertKind};
use super::relay::RelaySink;
use super::tests_agent::{Fixture, agent, agent_with, timing};
use crate::commands::agent;
use crate::commands::routines::chat::attach;
use crate::test_support::{ProjectHome, collecting_channel};

#[test]
fn stopping_a_run_a_window_shows_stops_its_agent() {
    let Some(fixture) = Fixture::new("omp_routine_hang.jsonl", timing()) else {
        return;
    };

    let (id, run_id, _, _open) = fixture.run_shown(agent("Check the build"));

    fixture.stop_by_user(&id, &run_id);
}

#[test]
fn an_agent_that_exits_while_a_window_shows_it_fails_the_run() {
    let Some(fixture) = Fixture::new("omp_routine_hang.jsonl", timing()) else {
        return;
    };

    let (id, _, info, _open) = fixture.run_shown(agent("Check the build"));

    block_on(agent::stop(&fixture.state, &info.key)).expect("the agent stops");

    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(run.record.message.as_deref(), Some(AGENT_GONE));
}

#[test]
fn the_time_limit_counts_from_the_start_of_the_chat() {
    let short = AgentTiming {
        minute: Duration::from_millis(400),
        ..timing()
    };
    let Some(fixture) = Fixture::new("omp_basic.jsonl", short) else {
        return;
    };

    let keys = fixture.fill_sessions();

    // A limit of one (short) minute, most of it spent waiting for a free session.
    let (id, _) = fixture.run(agent_with("Make `parse` handle empty input.", None, Some(1)));
    std::thread::sleep(Duration::from_millis(600));
    block_on(agent::stop(&fixture.state, &keys[0])).expect("a chat closes");

    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
}

#[test]
fn a_window_attaches_only_to_a_chat_that_still_runs() {
    let setup = ProjectHome::new("routines-attach-gone");
    let (relay, _signals) = RelaySink::new("0123456789abcdef0123456789abcdef");
    relay.emit(AgentEnvelope {
        session_key: "gone".to_owned(),
        seq: 1,
        event: AgentEvent::TurnStart,
    });
    setup.state.routines.relays().insert(relay.clone());

    let (window, _) = collecting_channel();
    let refused = attach(&setup.state, relay.run_id(), window).expect_err("no such chat");

    assert_eq!(refused.code(), IpcErrorCode::NotFound);
    assert!(!relay.is_attached());
}

/// The extra data of the notification of `kind` about run `run_id`.
fn clicked(kind: AlertKind, run_id: &str) -> HashMap<String, Value> {
    let alert = Alert {
        kind,
        routine_id: "fedcba9876543210fedcba9876543210".to_owned(),
        run_id: run_id.to_owned(),
        name: "Nightly".to_owned(),
    };

    alert
        .extra()
        .into_iter()
        .map(|(key, value)| (key.to_owned(), Value::String(value)))
        .collect()
}

#[test]
fn a_notification_click_opens_only_the_waiting_run_it_names() {
    let setup = ProjectHome::new("routines-click");
    let runtime = &setup.state.routines;
    let (listener, events) = collecting_channel();
    runtime.listen(listener);

    let (older, newer, idle) = (
        "00000000000000000000000000000001",
        "00000000000000000000000000000002",
        "00000000000000000000000000000003",
    );

    for run_id in [older, newer, idle] {
        let (relay, _signals) = RelaySink::new(run_id);
        relay.set_waiting(run_id != idle);
        runtime.relays().insert(relay);
    }

    runtime.on_notification_click(&clicked(AlertKind::Failed, newer));
    runtime.on_notification_click(&clicked(AlertKind::Finished, newer));
    runtime.on_notification_click(&HashMap::new());
    runtime.on_notification_click(&clicked(AlertKind::NeedsYou, idle));
    assert!(events.lock().unwrap().is_empty(), "nothing to open");

    runtime.on_notification_click(&clicked(AlertKind::NeedsYou, older));
    let opened: Vec<Value> = events
        .lock()
        .unwrap()
        .iter()
        .map(|event| event["runId"].clone())
        .collect();
    assert_eq!(opened, [older], "the run the notification named, not the newest");
}
