//! Agent chat fixtures for `ui/src/lib/ipc/mock/fixtures/agent/`, produced by the real engine.
//!
//! Each fixture runs a `fake_agent` scenario (`tests/fixtures/streams`) through
//! [`AgentManager`](harness_business::agent::AgentManager): the real codec, dialect, normalizer
//! and emitter. Keys and the clock are fixed and delta batching is off (it depends on timing), so
//! the output is deterministic. Streams end at their terminal event (`settled`, or `exited` for
//! the crash) and never include the `exited` caused by stopping the fake agent.

pub mod m3;
pub mod m4;
pub mod m5;

use std::sync::{Arc, OnceLock};

use harness_business::agent::SessionOptions;
use harness_business::model::{AgentAction, AgentEnvelope, AgentEvent, AgentReply, HarnessId, UiOutcome, UiResponse};
use serde::Serialize;
use serde_json::Value;

use crate::agent_support::{Run, start_with};
use crate::common::exported;

/// Session keys of the exported OMP and PI sessions.
fn session_key(harness: HarnessId) -> &'static str {
    match harness {
        HarnessId::Omp => "3f9a1c2e-7b4d-4e8f-9a6b-2c1d0e5f7a8b",
        HarnessId::Pi => "8c2d4e6f-1a3b-4c5d-8e7f-9a0b1c2d3e4f",
        HarnessId::OpenCode => unreachable!("OpenCode has no chat"),
        HarnessId::Claude => unreachable!("Claude Code has no chat"),
    }
}

fn options(harness: HarnessId) -> SessionOptions {
    SessionOptions {
        coalesce_window: std::time::Duration::ZERO,
        key: Arc::new(move || session_key(harness).to_owned()),
        clock: Arc::new(|| "2026-10-09T10:00:00Z".to_owned()),
        ..SessionOptions::default()
    }
}

async fn start(harness: HarnessId, scenario: &str) -> Run {
    let dialect = match harness {
        HarnessId::Omp => "omp",
        HarnessId::Pi => "pi",
        HarnessId::OpenCode => unreachable!("OpenCode has no chat"),
        HarnessId::Claude => unreachable!("Claude Code has no chat"),
    };

    start_with(harness, dialect, scenario, options(harness), |_| {})
        .await
        .unwrap_or_else(|error| panic!("{scenario} starts: {error}"))
}

/// `value` serialized with the run's temporary home and project rewritten to the demo paths.
fn portable(run: &Run, value: impl Serialize) -> Value {
    let project = dunce::canonicalize(run.project.path()).expect("project exists");
    exported(value, run.home.path(), &project)
}

fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

fn is_settled(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::Settled { .. })
}

/// The envelopes up to and including the first one matching `terminal`.
async fn stream_until(run: &Run, terminal: fn(&AgentEvent) -> bool) -> Vec<AgentEnvelope> {
    run.wait("terminal", terminal).await;

    let mut envelopes = run.sink.envelopes();
    let end = envelopes
        .iter()
        .position(|envelope| terminal(&envelope.event))
        .expect("terminal event");
    envelopes.truncate(end + 1);
    envelopes
}

async fn answer(run: &Run, id: &str, outcome: UiOutcome) {
    run.wait(
        "dialog",
        |event| matches!(event, AgentEvent::UiRequest { request } if request.id == id),
    )
    .await;
    let response = UiResponse {
        id: id.to_owned(),
        outcome,
    };
    run.manager.respond(run.key(), response).await.expect("dialog answered");
}

fn value(text: &str) -> UiOutcome {
    UiOutcome::Value { value: text.to_owned() }
}

/// A prompt that runs to `settled`.
async fn prompted(scenario: &str, message: &str) -> (Run, Vec<AgentEnvelope>) {
    let run = start(HarnessId::Omp, scenario).await;
    run.manager
        .send(run.key(), prompt(message))
        .await
        .expect("prompt accepted");
    let stream = stream_until(&run, is_settled).await;

    (run, stream)
}

async fn approval(scenario: &str, choice: &str) -> Vec<AgentEnvelope> {
    let run = start(HarnessId::Omp, scenario).await;
    run.manager
        .send(run.key(), prompt("Run the tests."))
        .await
        .expect("prompt accepted");
    answer(&run, "approval-1", value(choice)).await;

    let stream = stream_until(&run, is_settled).await;
    run.manager.stop(run.key()).await.ok();
    stream
}

async fn crash() -> Vec<AgentEnvelope> {
    let run = start(HarnessId::Omp, "omp_crash.jsonl").await;
    run.manager
        .send(run.key(), prompt("Refactor the config loader."))
        .await
        .expect("prompt accepted");

    stream_until(&run, |event| matches!(event, AgentEvent::Exited { .. })).await
}

async fn ui_requests() -> Vec<AgentEnvelope> {
    let run = Arc::new(start(HarnessId::Omp, "omp_ui_requests.jsonl").await);
    let sender = run.clone();
    let pending = tokio::spawn(async move { sender.manager.send(sender.key(), prompt("/setup")).await });

    answer(&run, "ui-1", UiOutcome::Confirmed { confirmed: true }).await;
    answer(&run, "ui-2", value("pre-commit")).await;
    answer(
        &run,
        "ui-3",
        value("#!/bin/sh\ncargo fmt --all --check\ncargo clippy\n"), // guard-allow: installer-shell
    )
    .await;
    pending.await.expect("prompt task").expect("prompt handled");

    let stream = stream_until(&run, is_settled).await;
    run.manager.stop(run.key()).await.ok();
    stream
}

/// The `agent_send` reply fixtures.
async fn replies() -> Vec<(&'static str, Value)> {
    let run = start(HarnessId::Omp, "omp_replies.jsonl").await;
    let send = |action: AgentAction| run.manager.send(run.key(), action);
    let mut fixtures = Vec::new();

    let history = AgentAction::GetHistory { cursor: None, limit: 4 };
    for (name, action) in [
        ("agent/models.json", AgentAction::GetAvailableModels),
        ("agent/state.json", AgentAction::GetState),
        ("agent/stats.json", AgentAction::GetSessionStats),
        ("agent/history.json", history),
    ] {
        let reply: AgentReply = send(action).await.unwrap_or_else(|error| panic!("{name}: {error}"));
        fixtures.push((name, portable(&run, reply)));
    }

    run.manager.stop(run.key()).await.ok();
    fixtures
}

async fn pi_session_info() -> Value {
    let run = start(HarnessId::Pi, "pi_startup.jsonl").await;
    let info = portable(&run, &run.info);

    run.manager.stop(run.key()).await.ok();
    info
}

async fn export() -> Vec<(&'static str, Value)> {
    let (basic_run, basic) = prompted(
        "omp_basic.jsonl",
        "Make `parse` handle empty input and explain the change.",
    )
    .await;
    let omp_info = portable(&basic_run, &basic_run.info);
    let basic = portable(&basic_run, basic);
    basic_run.manager.stop(basic_run.key()).await.ok();

    let (error_run, error) = prompted("omp_error.jsonl", "Summarize the open TODOs.").await;
    error_run.manager.stop(error_run.key()).await.ok();

    let to_json = |value: Vec<AgentEnvelope>| serde_json::to_value(value).expect("envelopes serialize");

    let mut fixtures = vec![
        ("agent/session_info.omp.json", omp_info),
        ("agent/session_info.pi.json", pi_session_info().await),
        ("agent/stream_basic.json", basic),
        (
            "agent/stream_approval.json",
            to_json(approval("omp_approval.jsonl", "Approve").await),
        ),
        (
            "agent/stream_approval_denied.json",
            to_json(approval("omp_approval_denied.jsonl", "Deny").await),
        ),
        ("agent/stream_error.json", to_json(error)),
        ("agent/stream_crash.json", to_json(crash().await)),
        ("agent/stream_ui_requests.json", to_json(ui_requests().await)),
    ];
    fixtures.extend(replies().await);

    fixtures
}

/// Every agent fixture path (relative to the fixtures directory) with its content; computed once
/// per test binary.
pub fn agent_fixtures() -> Vec<(&'static str, Value)> {
    static FIXTURES: OnceLock<Vec<(&'static str, Value)>> = OnceLock::new();

    FIXTURES
        .get_or_init(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");

            runtime.block_on(export())
        })
        .clone()
}
