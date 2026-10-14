//! Misbehaving agents: broken and oversized frames, crashes, hangs, missing handshakes and noisy
//! stderr. The session must keep reading, fail what is waiting, and end with exactly one `exited`.

mod agent_support;

use std::time::{Duration, Instant};

use agent_support::{options, start_omp, start_with};
use harness_business::agent::{MAX_STDERR_LINE, SessionOptions};
use harness_business::model::{AgentAction, AgentEvent, HarnessId, IpcErrorCode};
use pretty_assertions::assert_eq;

fn exits(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Exited { .. }))
        .count()
}

fn frame_errors(events: &[AgentEvent]) -> Vec<(Option<String>, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::FrameError { original_type, error } => Some((original_type.clone(), error.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn broken_frames_become_frame_errors_and_reading_continues() {
    let run = start_omp("omp_bad_frames.jsonl").await;

    let state = run.manager.send(run.key(), AgentAction::GetState).await;
    let error = state.unwrap_err();
    assert_eq!(
        error.code(),
        IpcErrorCode::Protocol,
        "an oversized response fails its request at once"
    );

    run.wait(
        "last notice",
        |event| matches!(event, AgentEvent::Notice { message, .. } if message == "still reading"),
    )
    .await;
    let events = run.events();

    let errors = frame_errors(&events);
    assert_eq!(errors.len(), 5, "{errors:#?}");
    assert!(errors[0].1.starts_with("invalid JSON"));
    assert_eq!(errors[1].1, "frame is not a JSON object");
    assert_eq!(errors[2].0.as_deref(), Some("message_update"));
    assert!(errors[2].1.contains("4 MiB"));
    assert_eq!(errors[3].0.as_deref(), Some("tool_execution_end"));
    assert_eq!(errors[4].0.as_deref(), Some("response"));

    let notices: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Notice { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(notices, ["line\u{2028}separator\u{2029}kept", "crlf", "still reading"]);
    assert!(run.is_listed());

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_crash_fails_waiting_requests_and_emits_exited_once() {
    let run = start_omp("omp_crash_pending.jsonl").await;

    let error = run.manager.send(run.key(), AgentAction::GetState).await.unwrap_err();
    assert_eq!(error.code(), IpcErrorCode::Process);

    run.wait("exit", |event| {
        matches!(event, AgentEvent::Exited { code: Some(3), .. })
    })
    .await;
    let events = run.events();
    assert!(
        matches!(events.last(), Some(AgentEvent::Exited { .. })),
        "exited is the last event"
    );
    assert_eq!(exits(&events), 1);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Stderr { line } if line.contains("out of memory")))
    );

    let after = run.manager.send(run.key(), AgentAction::GetState).await.unwrap_err();
    assert_eq!(after.code(), IpcErrorCode::Process);
    assert!(run.manager.list().is_empty(), "exited sessions are forgotten");

    run.manager.stop(run.key()).await.unwrap();
    assert_eq!(exits(&run.events()), 1);
}

#[tokio::test]
async fn stop_closes_stdin_and_exits_cleanly() {
    let run = start_omp("omp_out_of_order.jsonl").await;

    let started = Instant::now();
    run.manager.stop(run.key()).await.unwrap();

    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    let events = run.events();
    assert!(matches!(events.last(), Some(AgentEvent::Exited { code: Some(0), .. })));
    assert_eq!(exits(&events), 1);
}

#[tokio::test]
async fn an_agent_ignoring_stdin_eof_is_killed_after_the_grace_period() {
    let run = start_omp("omp_hang.jsonl").await;

    let started = Instant::now();
    run.manager.stop(run.key()).await.unwrap();
    let elapsed = started.elapsed();

    assert!(elapsed >= Duration::from_millis(2900), "killed too early: {elapsed:?}");
    assert!(elapsed < Duration::from_secs(8), "killed too late: {elapsed:?}");

    let last = run.events().last().cloned();
    let Some(AgentEvent::Exited { code, signal }) = last else {
        panic!("exited is the last event");
    };
    // Unix reports the SIGKILL, Windows has no signals and TerminateProcess leaves exit code 1.
    if cfg!(unix) {
        assert_eq!((code, signal), (None, Some(9)));
    } else {
        assert_eq!((code, signal), (Some(1), None));
    }
}

#[tokio::test]
async fn a_missing_ready_frame_fails_startup_without_events() {
    let quick = SessionOptions {
        startup_timeout: Duration::from_millis(400),
        ..options()
    };

    // The PI-flavoured fake never sends OMP's ready frame.
    let result = start_with(HarnessId::Omp, "pi", "pi_startup.jsonl", quick, |_| {}).await;

    let error = result.err().expect("startup fails");
    assert_eq!(error.code(), IpcErrorCode::Timeout);
}

#[tokio::test]
async fn stderr_is_redacted_truncated_and_rate_limited() {
    let run = start_omp("omp_stderr_flood.jsonl").await;
    run.wait(
        "stderr",
        |event| matches!(event, AgentEvent::Stderr { line } if line.starts_with("noisy")),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    run.manager.stop(run.key()).await.unwrap();

    let lines: Vec<String> = run
        .events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Stderr { line } => Some(line),
            _ => None,
        })
        .collect();

    assert!(lines.iter().all(|line| line.len() <= MAX_STDERR_LINE));
    assert!(
        lines[0].starts_with("config dump:") && !lines[0].contains("CANARY"),
        "{}",
        lines[0]
    );
    assert_eq!(lines[1].len(), MAX_STDERR_LINE);

    let noisy = lines.iter().filter(|line| line.starts_with("noisy")).count();
    assert_eq!(
        noisy, 18,
        "20 lines in the first second, two of them were the secret and the long line"
    );
    assert_eq!(lines.last().unwrap(), "[82 more stderr lines were not shown]");
}

#[tokio::test]
async fn invalid_requests_are_rejected_before_reaching_the_agent() {
    let run = start_omp("omp_out_of_order.jsonl").await;

    let huge = AgentAction::Prompt {
        message: "x".repeat(512 * 1024 + 1),
        images: Vec::new(),
        streaming_behavior: None,
    };
    assert_eq!(
        run.manager.send(run.key(), huge).await.unwrap_err().code(),
        IpcErrorCode::InvalidInput
    );

    let page = AgentAction::GetHistory { cursor: None, limit: 0 };
    assert_eq!(
        run.manager.send(run.key(), page).await.unwrap_err().code(),
        IpcErrorCode::InvalidInput
    );

    let blank = run.manager.bash(run.key(), "   ").await.unwrap_err();
    assert_eq!(blank.code(), IpcErrorCode::InvalidInput);

    let unknown = run
        .manager
        .send("no-such-key", AgentAction::GetState)
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), IpcErrorCode::NotFound);

    run.manager.stop(run.key()).await.unwrap();
}
