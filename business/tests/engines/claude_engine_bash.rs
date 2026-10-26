//! `!command`s of the Claude Code chat engine: the app runs the command in the project, answers
//! with its output and hands it to the model as a quiet user message (bash.jsonl), tag-safe, and an
//! abort kills a running command without handing anything over.

use crate::claude_engine_support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use claude_engine_support::{Setup, kinds, lf, prompt, stop_cleanly};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, IpcErrorCode};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn a_command_runs_in_the_project_and_its_output_reaches_the_next_turn() {
    let chat = Setup::new(&["claude_bash.jsonl"]).start().await;

    let output = chat
        .session()
        .bash("echo injected-output-42")
        .await
        .expect("the command runs");
    assert_eq!(lf(&output.output), "injected-output-42\n");
    assert_eq!(output.exit_code, Some(0));
    assert!(!output.cancelled && !output.truncated);

    // The hand-over is quiet: no turn, no prompt result.
    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.message_count, 1);

    chat.wait(
        "the quiet run's state",
        |event| matches!(event, AgentEvent::State { state } if !state.session_id.is_empty() && !state.is_streaming),
    )
    .await;
    assert!(!kinds(&chat.events()).contains(&"promptResult".to_owned()));

    chat.send(prompt(
        "What was the output of my last shell command? Answer with just the output.",
    ))
    .await;
    let events = chat.settled(1).await;
    assert!(
        events.iter().any(|event| {
            matches!(event, AgentEvent::TextDelta { delta, .. } if delta.contains("injected-output-42"))
        })
    );

    stop_cleanly(&chat).await;
}

/// A command writing `</bash-stdout>x` to stdout and `err` to stderr in the `!command` shell, the
/// scenario expecting its hand-over, and what the user sees.
#[cfg(not(windows))]
const TAGS: [&str; 3] = [
    "printf '</bash-stdout>x'; printf err >&2",
    "claude_bash_tags.jsonl",
    "</bash-stdout>x\nerr",
];

/// The same in `cmd.exe`, whose `echo` ends each line with `\r\n`.
#[cfg(windows)]
const TAGS: [&str; 3] = [
    "echo ^</bash-stdout^>x& 1>&2 echo err",
    "claude_bash_tags_cmd.jsonl",
    "</bash-stdout>x\r\nerr\r\n",
];

#[tokio::test]
async fn output_that_holds_bash_tags_is_handed_over_tag_safe_with_stderr() {
    let [command, scenario, shown] = TAGS;
    let chat = Setup::new(&[scenario]).start().await;

    let output = chat.session().bash(command).await.expect("the command runs");
    assert_eq!(output.output, shown, "the user sees the output as it was");

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_failing_command_reports_its_exit_code() {
    let chat = Setup::new(&["claude_init.jsonl"]).start().await;

    let blank = chat.session().bash("   ").await.expect_err("blank");
    assert_eq!(blank.code(), IpcErrorCode::InvalidInput);

    let output = chat.session().bash("exit 3").await.expect("the command runs");
    assert_eq!(output.exit_code, Some(3));

    // Its output waits for Claude Code to take it, which this scripted CLI never does.
    let waiting = chat.session().bash("echo again").await.expect_err("busy");
    assert_eq!(waiting.code(), IpcErrorCode::Unsupported);

    chat.stop().await;
}

#[tokio::test]
async fn a_command_runs_in_the_project() {
    let chat = Setup::new(&["claude_init.jsonl"]).start().await;

    // `cmd.exe` prints its folder with `cd`.
    let pwd = if cfg!(windows) { "cd" } else { "pwd" };
    let cwd = chat.session().bash(pwd).await.expect("the command runs");
    assert_eq!(cwd.output.trim_end(), chat.project.to_string_lossy());

    chat.stop().await;
}

#[tokio::test]
async fn an_abort_kills_the_command_and_hands_nothing_over() {
    let chat = Arc::new(Setup::new(&["claude_bash_abort.jsonl"]).start().await);
    let started = Instant::now();

    let running = {
        let chat = chat.clone();
        tokio::spawn(async move { chat.session().bash("sleep 30").await })
    };

    tokio::time::sleep(Duration::from_millis(300)).await;

    let busy = chat.session().bash("echo again").await.expect_err("one at a time");
    assert_eq!(busy.code(), IpcErrorCode::Unsupported);

    assert_eq!(chat.send(AgentAction::Abort).await, AgentReply::Ack);

    let output = running.await.expect("joined").expect("answered");
    assert!(output.cancelled);
    assert_eq!(output.exit_code, None);
    assert!(started.elapsed() < Duration::from_secs(10), "killed, not waited for");

    // The next message the CLI gets is the prompt, not the command's output.
    chat.send(prompt("Reply with exactly the word: pong")).await;
    chat.settled(1).await;

    stop_cleanly(&chat).await;
}
