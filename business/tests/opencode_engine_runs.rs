//! `!command`s, compaction, statistics, the last reply and the HTML export of the OpenCode chat
//! engine against the scripted `opencode serve`.

mod agent_support;
mod opencode_engine_support;

use harness_business::agent::SessionOptions;
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, BashOutput, IpcErrorCode, StreamingBehavior, TokenUsage,
};
use opencode_engine_support::{SESSION, count, kind, start, start_with, until_marker};
use pretty_assertions::assert_eq;

fn code(result: harness_business::Result<AgentReply>) -> IpcErrorCode {
    result.expect_err("the action is refused").code()
}

#[tokio::test]
async fn commands_run_in_the_session_shell_without_showing_a_turn() {
    let chat = start("opencode_shell.jsonl").await;
    let session = chat.session();

    let busy = session.bash("ls").await.expect_err("OpenCode answered 409");
    assert_eq!(busy.code(), IpcErrorCode::Unsupported);
    assert!(busy.to_string().contains("OpenCode is working"), "{busy}");

    let output = session.bash("echo from-shell").await.expect("the command ran");
    assert_eq!(
        output,
        BashOutput {
            output: "from-shell\n".to_owned(),
            exit_code: None,
            cancelled: false,
            truncated: false,
        }
    );

    let model = AgentAction::SetModel {
        provider: "mock".to_owned(),
        model_id: "m1".to_owned(),
    };
    session.send(model).await.expect("model picked");

    let secret = session.bash("printenv").await.expect("answered before its idle");
    assert!(!secret.output.contains("abc123"), "{}", secret.output);
    assert!(secret.output.contains("HOME=/home/me"));

    let events = until_marker(&chat).await;
    let shown: Vec<String> = events.iter().map(kind).collect();
    for hidden in ["agentStart", "settled", "messageStart", "toolStart", "toolEnd"] {
        assert!(!shown.iter().any(|kind| kind == hidden), "{hidden} in {shown:?}");
    }

    let agent = AgentAction::SetAgent {
        agent: "plan".to_owned(),
    };
    assert_eq!(
        session
            .send(agent)
            .await
            .expect("idle once the last run's idle arrived"),
        AgentReply::Ack
    );
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn compaction_runs_in_the_background_and_holds_prompts_until_it_ends() {
    let chat = start("opencode_compact.jsonl").await;
    let session = chat.session();

    let instructed = AgentAction::Compact {
        instructions: Some("keep the API notes".to_owned()),
    };
    assert_eq!(code(session.send(instructed).await), IpcErrorCode::Unsupported);

    let compact = AgentAction::Compact { instructions: None };
    assert_eq!(
        session.send(compact.clone()).await.expect("compaction starts"),
        AgentReply::Ack
    );
    chat.wait(
        "the summarize call",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Compacting")),
    )
    .await;

    let state = match session.send(AgentAction::GetState).await {
        Ok(AgentReply::State { state }) => state,
        other => panic!("state: {other:?}"),
    };
    assert!(state.is_compacting);
    assert!(!state.is_streaming);
    assert_eq!(code(session.send(compact).await), IpcErrorCode::Unsupported);

    let steer = AgentAction::Prompt {
        message: "after compaction".to_owned(),
        images: Vec::new(),
        streaming_behavior: Some(StreamingBehavior::Steer),
    };
    let held = session.send(steer).await.expect("held");
    assert_eq!(
        held,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("queued".to_owned())
        },
        "steering waits while OpenCode compacts"
    );

    // The scenario's sync point: the prompt is held before the compaction ends.
    session.send(AgentAction::GetSessionStats).await.expect("stats");

    let events = until_marker(&chat).await;
    let shown: Vec<String> = events.iter().map(kind).collect();
    let compactions: Vec<&AgentEvent> = events.iter().filter(|event| kind(event) == "compaction").collect();
    assert_eq!(
        compactions,
        [
            &AgentEvent::Compaction {
                phase: "start".to_owned(),
                reason: Some("manual".to_owned()),
                error: None,
            },
            &AgentEvent::Compaction {
                phase: "end".to_owned(),
                reason: Some("manual".to_owned()),
                error: None,
            },
        ]
    );

    let end = shown.iter().rposition(|kind| kind == "compaction").expect("end");
    let first_start = shown
        .iter()
        .position(|kind| kind == "agentStart")
        .expect("the prompt's turn");
    assert!(end < first_start, "the summary's run is not a turn: {shown:?}");
    assert_eq!(count(&events, "agentStart"), 1);
    assert_eq!(count(&events, "settled"), 1);
    assert_eq!(
        count(&events, "textDelta"),
        0,
        "the summary is not streamed into the chat"
    );
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn rename_stats_last_reply_and_an_escaped_export_outside_the_project() {
    let exports = tempfile::tempdir().expect("export folder");
    let export_dir = exports.path().join("exports");
    let options = SessionOptions {
        export_dir: Some(export_dir.clone()),
        ..agent_support::options()
    };
    let chat = start_with("opencode_reports.jsonl", None, options).await;
    let session = chat.session();

    let rename = AgentAction::SetSessionName {
        name: "Renamed chat".to_owned(),
    };
    assert_eq!(session.send(rename).await.expect("rename"), AgentReply::Ack);
    chat.wait(
        "renamed state",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Renamed chat")),
    )
    .await;

    let stats = match session.send(AgentAction::GetSessionStats).await {
        Ok(AgentReply::Stats { stats }) => stats,
        other => panic!("stats: {other:?}"),
    };
    assert_eq!(
        (stats.user_messages, stats.assistant_messages, stats.tool_calls),
        (8, 11, 4)
    );
    assert_eq!(
        stats.tokens,
        TokenUsage {
            input: 300,
            output: 52,
            cache_read: 0,
            cache_write: 0,
            total: 352,
        }
    );
    assert_eq!(stats.context_usage.map(|usage| usage.tokens), Some(25));

    assert_eq!(
        session
            .send(AgentAction::GetLastAssistantText)
            .await
            .expect("last reply"),
        AgentReply::Text {
            text: Some("Hello from the mock model.".to_owned())
        }
    );

    let path = match session.send(AgentAction::ExportHtml).await {
        Ok(AgentReply::Exported { path }) => std::path::PathBuf::from(path),
        other => panic!("export: {other:?}"),
    };
    assert_eq!(path, export_dir.join(format!("opencode-{SESSION}.html")));
    assert!(!path.starts_with(&chat.project), "never into the project");

    let html = std::fs::read_to_string(&path).expect("the export");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>Renamed chat</title>"));
    assert!(!html.to_lowercase().contains("<script"), "{html}");
    assert!(!html.contains("\"><img"), "{html}");
    assert!(!html.contains("abc123"), "secrets are redacted");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("<details class=\"thinking\">"));
    assert!(html.contains("<h3>Result</h3>"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(&path).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "only the user reads the transcript");

        let folder = std::fs::metadata(&export_dir).expect("metadata").permissions().mode();
        assert_eq!(folder & 0o777, 0o700, "only the user lists the exports");
    }

    assert!(session.is_running(), "every request matched the scenario");
    session.stop().await;
}

#[tokio::test]
async fn replies_never_show_the_server_password() {
    let exports = tempfile::tempdir().expect("export folder");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(exports.path(), std::fs::Permissions::from_mode(0o755)).expect("open the folder");
    }
    let options = SessionOptions {
        export_dir: Some(exports.path().to_path_buf()),
        ..agent_support::options()
    };
    let chat = start_with("opencode_password_replies.jsonl", None, options).await;
    let session = chat.session();
    let password = chat.password();

    let last = session
        .send(AgentAction::GetLastAssistantText)
        .await
        .expect("last reply");
    let AgentReply::Text { text: Some(text) } = last else {
        panic!("last reply: {last:?}");
    };
    assert!(!text.contains(&password), "{text}");
    assert!(text.contains("••••"), "{text}");

    let path = match session.send(AgentAction::ExportHtml).await {
        Ok(AgentReply::Exported { path }) => std::path::PathBuf::from(path),
        other => panic!("export: {other:?}"),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let folder = std::fs::metadata(exports.path())
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(folder & 0o777, 0o700, "an existing exports folder is narrowed");
    }

    let html = std::fs::read_to_string(&path).expect("the export");
    assert!(!html.contains(&password), "the export holds the password");

    let output = session.bash("env").await.expect("ran");
    assert!(!output.output.contains(&password), "{output:?}");
    assert!(output.output.contains("••••"), "{output:?}");

    assert!(session.is_running(), "every request matched the scenario");
    session.stop().await;
}
