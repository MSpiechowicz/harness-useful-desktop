//! Stopping OMP subagents: the panel's Stop and the stops an abort of the main turn sends.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::agent_support::{Run, scenario, start_omp};
use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, IpcErrorCode, SubagentStatus};
use harness_business::pi_family::rpc::EncodeContext;
use pretty_assertions::assert_eq;
use serde_json::json;

fn stop(id: &str) -> AgentAction {
    AgentAction::StopSubagent { id: id.to_owned() }
}

fn exit_code(events: &[AgentEvent]) -> Option<i32> {
    match events.last() {
        Some(AgentEvent::Exited { code, .. }) => *code,
        other => panic!("the agent did not exit: {other:?}"),
    }
}

/// The chat's `Abort` is answered with an ack.
async fn abort_is_acked(run: &Run) {
    assert_eq!(
        run.manager.send(run.key(), AgentAction::Abort).await.unwrap(),
        AgentReply::Ack
    );
}

/// The agent's next expected request (the scenarios end on it) is answered.
async fn ask_last_assistant_text(run: &Run) {
    run.manager
        .send(run.key(), AgentAction::GetLastAssistantText)
        .await
        .unwrap();
}

/// The session stops and the agent exits with code 0 (every expect of the scenario was met).
async fn stop_and_expect_clean_exit(run: &Run) {
    run.manager.stop(run.key()).await.unwrap();
    assert_eq!(exit_code(&run.events()), Some(0));
}

#[tokio::test]
async fn stop_sends_cancel_subagent_and_a_gone_subagent_is_still_an_ack() {
    let run = start_omp("omp_stop_subagent.jsonl").await;

    assert_eq!(
        run.manager.send(run.key(), stop("Scout")).await.unwrap(),
        AgentReply::Ack
    );
    assert_eq!(
        run.manager.send(run.key(), stop("Gone")).await.unwrap(),
        AgentReply::Ack
    );

    // An OMP without the command refuses it: an error, not a crash.
    let refused = run.manager.send(run.key(), stop("Old")).await.unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::Protocol);
    assert!(refused.to_string().contains("cancel_subagent failed"), "{refused}");

    stop_and_expect_clean_exit(&run).await;
}

#[tokio::test]
async fn abort_stops_each_running_subagent_and_a_refused_stop_does_not_fail_it() {
    let run = start_omp("omp_abort_subagents.jsonl").await;
    run.wait_until("both running and one finished", |events| {
        let finished = events.iter().any(|event| {
            matches!(event, AgentEvent::Subagent { subagent } if subagent.id == "Finished" && subagent.status == SubagentStatus::Completed)
        });
        let running = |id: &str| {
            events.iter().any(|event| {
                matches!(event, AgentEvent::Subagent { subagent } if subagent.id == id && subagent.status == SubagentStatus::Running)
            })
        };

        finished && running("Scout") && running("Mapper")
    })
    .await;

    // The stop of Scout is refused, the abort is still answered.
    abort_is_acked(&run).await;

    // The agent reports the stopped one as aborted, and the chat goes on.
    run.wait("Mapper aborted", |event| {
        matches!(event, AgentEvent::Subagent { subagent } if subagent.id == "Mapper" && subagent.status == SubagentStatus::Aborted)
    })
    .await;
    ask_last_assistant_text(&run).await;

    // A stop for the finished subagent would have failed the agent's last expect.
    stop_and_expect_clean_exit(&run).await;
}

#[tokio::test]
async fn abort_with_no_running_subagent_sends_no_stop() {
    let run = start_omp("omp_abort_idle.jsonl").await;
    run.wait(
        "Scout completed",
        |event| matches!(event, AgentEvent::Subagent { subagent } if subagent.status == SubagentStatus::Completed),
    )
    .await;

    abort_is_acked(&run).await;
    ask_last_assistant_text(&run).await;

    stop_and_expect_clean_exit(&run).await;
}

/// A scenario file in `dir` that starts like `omp_startup.jsonl`, then plays `steps`.
fn write_scenario(dir: &Path, steps: &[String]) -> String {
    let startup = scenario("omp_startup.jsonl");
    let mut lines = vec![json!({ "include": startup.to_string_lossy() }).to_string()];
    lines.extend_from_slice(steps);

    let path = dir.join("scenario.jsonl");
    std::fs::write(&path, lines.join("\n")).unwrap();

    path.to_string_lossy().into_owned()
}

fn started(id: &str) -> String {
    json!({ "emit": { "type": "subagent_lifecycle", "payload": { "id": id, "agent": "scout", "status": "started" } } })
        .to_string()
}

fn expect_cancel(id: &str) -> String {
    json!({ "expect": "cancel_subagent", "match": { "subagentId": id }, "reply": { "success": true, "data": { "cancelled": true } } }).to_string()
}

fn running_count(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(
            |event| matches!(event, AgentEvent::Subagent { subagent } if subagent.status == SubagentStatus::Running),
        )
        .count()
}

#[tokio::test]
async fn abort_stops_no_more_than_the_tracked_limit() {
    const TRACKED: usize = 512;

    let dir = tempfile::tempdir().unwrap();
    let mut steps: Vec<String> = (0..=TRACKED).map(|n| started(&format!("S{n:03}"))).collect();
    steps.push(r#"{"expect":"abort","reply":{"success":true}}"#.to_owned());
    steps.extend((0..TRACKED).map(|n| expect_cancel(&format!("S{n:03}"))));
    steps.push(r#"{"expect":"get_last_assistant_text","reply":{"success":true,"data":{"text":null}}}"#.to_owned());
    // The 513th subagent would match this expect and fail it.
    steps.push(r#"{"expect":"cancel_subagent","match":{"subagentId":"NEVER"}}"#.to_owned());

    let run = start_omp(&write_scenario(dir.path(), &steps)).await;
    run.wait_until("every subagent running", |events| running_count(events) == TRACKED + 1)
        .await;

    abort_is_acked(&run).await;
    ask_last_assistant_text(&run).await;

    stop_and_expect_clean_exit(&run).await;
}

#[tokio::test]
async fn abort_after_a_new_session_stops_no_subagent_of_the_old_one() {
    let run = start_omp("omp_abort_after_new_session.jsonl").await;
    run.wait(
        "Scout running",
        |event| matches!(event, AgentEvent::Subagent { subagent } if subagent.status == SubagentStatus::Running),
    )
    .await;

    assert_eq!(
        run.manager.send(run.key(), AgentAction::NewSession).await.unwrap(),
        AgentReply::Ack
    );
    abort_is_acked(&run).await;
    ask_last_assistant_text(&run).await;

    stop_and_expect_clean_exit(&run).await;
}

#[tokio::test]
async fn abort_sends_its_stops_together_and_waits_one_time_limit_for_all_of_them() {
    let run = start_omp("omp_abort_stuck_subagents.jsonl").await;
    run.wait_until("three running", |events| running_count(events) == 3)
        .await;

    let started_at = Instant::now();
    let reply = run.manager.send(run.key(), AgentAction::Abort).await.unwrap();
    let elapsed = started_at.elapsed();

    assert_eq!(reply, AgentReply::Ack);
    // One stop that never answers holds the abort for the limit (5 s). Three in a row would take 15 s.
    assert!(elapsed < Duration::from_secs(9), "the abort took {elapsed:?}");

    ask_last_assistant_text(&run).await;
    stop_and_expect_clean_exit(&run).await;
}

#[test]
fn omp_stops_a_subagent_with_cancel_subagent_and_pi_cannot() {
    let stop = AgentAction::StopSubagent {
        id: "FamilyGalleryResearch".to_owned(),
    };

    let context = EncodeContext {
        sessions_dir: Path::new("/nonexistent"),
    };

    assert_eq!(
        Omp.dialect().encode(&stop, "req-9", &context).unwrap(),
        json!({ "id": "req-9", "type": "cancel_subagent", "subagentId": "FamilyGalleryResearch" })
    );

    let refused = Pi.dialect().encode(&stop, "req-9", &context).unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
}
