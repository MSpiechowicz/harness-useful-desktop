//! Agent sessions end to end: the real engine (spawner, codec, dialects, routing, emitter) against
//! scripted `fake_agent` peers.

mod agent_support;

use agent_support::{kinds, start_omp, start_pi};
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ChatBlock, CommandInvocation, CommandSource, MessageRole, PromptStatus,
};
use pretty_assertions::assert_eq;

fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        streaming_behavior: None,
    }
}

#[tokio::test]
async fn omp_stream_starts_with_ready_state_and_commands_and_numbers_every_event() {
    let run = start_omp("omp_basic.jsonl").await;

    assert_eq!(run.info.protocol_version, Some(1));
    run.manager
        .send(run.key(), prompt("Make `parse` handle empty input"))
        .await
        .unwrap();
    run.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;

    let envelopes = run.sink.envelopes();
    let events = run.events();
    assert_eq!(kinds(&events[..3]), ["ready", "state", "commandsUpdated"]);
    assert!(envelopes.iter().all(|envelope| envelope.session_key == run.info.key));
    assert!(
        envelopes
            .iter()
            .enumerate()
            .all(|(index, envelope)| envelope.seq == index as u64 + 1)
    );

    // The early `available_commands_update` is superseded by the startup catalog.
    let catalogs = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::CommandsUpdated { .. }))
        .count();
    assert_eq!(catalogs, 1);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn omp_startup_catalog_merges_native_live_and_shared_commands() {
    let run = start_omp("omp_basic.jsonl").await;

    let commands = run.catalog().await;
    let find = |name: &str| commands.iter().find(|command| command.name == name);

    assert_eq!(find("skill:plain-writing").unwrap().source, CommandSource::Skill);
    assert_eq!(find("github:summarize-pr").unwrap().source, CommandSource::McpPrompt);
    assert_eq!(find("deploy").unwrap().argument_hint.as_deref(), Some("<env>"));
    assert_eq!(find("release-notes").unwrap().invocation, CommandInvocation::PromptText);
    // Native beats live: OMP's static builtin keeps its own hint.
    assert_eq!(find("compact").unwrap().source, CommandSource::Builtin);
    assert_ne!(find("compact").unwrap().argument_hint.as_deref(), Some("[focus]"));
    assert_eq!(find("new").unwrap().source, CommandSource::Shared);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn omp_prompt_streams_deltas_tools_and_completes_with_its_request_id() {
    let run = start_omp("omp_basic.jsonl").await;
    run.manager.send(run.key(), prompt("go")).await.unwrap();
    run.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;
    let events = run.events();

    let thinking: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ThinkingDelta { message_id, delta, .. } if message_id == "msg-2" => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert!(thinking.starts_with("The parser panics on empty input."));

    let edit_end = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnd {
                tool_call_id,
                touched_paths,
                result,
                ..
            } if tool_call_id == "call-2" => Some((touched_paths.clone(), result.clone())),
            _ => None,
        })
        .expect("edit tool end");
    assert_eq!(edit_end.0, ["src/lib.rs"]);
    assert_eq!(edit_end.1["content"][0]["text"], "Updated src/lib.rs (1 replacement)");
    assert!(edit_end.1.get("details").is_none());

    let assistant = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::MessageEnd { message_id, message } if message_id == "msg-2" => Some(message.clone()),
            _ => None,
        })
        .expect("assistant message end");
    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(assistant.stop_reason.as_deref(), Some("toolUse"));
    assert_eq!(assistant.usage.unwrap().total, 4200 + 612 + 12_800);
    assert!(matches!(&assistant.blocks[2], ChatBlock::ToolCall { name, .. } if name == "read"));

    let result = events
        .iter()
        .find(|event| matches!(event, AgentEvent::PromptResult { .. }))
        .unwrap();
    assert_eq!(
        result,
        &AgentEvent::PromptResult {
            request_id: Some("req-5".to_owned()),
            status: PromptStatus::Completed,
            error: None,
            retryable: false,
        }
    );

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn omp_replies_drop_provider_secrets_and_serve_history_newest_page_first() {
    let run = start_omp("omp_replies.jsonl").await;
    let send = |action: AgentAction| run.manager.send(run.key(), action);

    let AgentReply::Models { models } = send(AgentAction::GetAvailableModels).await.unwrap() else {
        panic!("models reply");
    };
    assert_eq!(models.len(), 4);
    assert_eq!(models[3].name, None);
    assert!(!serde_json::to_string(&models).unwrap().contains("CANARY"));

    let AgentReply::State { state } = send(AgentAction::GetState).await.unwrap() else {
        panic!("state reply");
    };
    assert_eq!(state.message_count, 6);
    assert_eq!(state.context_usage.unwrap().tokens, 17_600);

    let AgentReply::Stats { stats } = send(AgentAction::GetSessionStats).await.unwrap() else {
        panic!("stats reply");
    };
    assert_eq!((stats.tokens.total, stats.cost_usd), (82_150, Some(0.0874)));

    let page = send(AgentAction::GetHistory { cursor: None, limit: 4 }).await.unwrap();
    let AgentReply::History {
        messages,
        next_cursor,
        total,
    } = page
    else {
        panic!("history reply");
    };
    assert_eq!((next_cursor.as_deref(), total), (Some("2"), Some(6)));
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(
        messages[2].blocks[1],
        ChatBlock::Image {
            mime_type: "image/png".to_owned()
        }
    );
    assert!(!serde_json::to_string(&messages).unwrap().contains("CANARY"));

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn out_of_order_responses_reach_their_own_requests() {
    let run = start_omp("omp_out_of_order.jsonl").await;

    let (state, stats) = tokio::join!(
        run.manager.send(run.key(), AgentAction::GetState),
        run.manager.send(run.key(), AgentAction::GetSessionStats),
    );

    assert!(matches!(state.unwrap(), AgentReply::State { state } if state.message_count == 2));
    assert!(matches!(stats.unwrap(), AgentReply::Stats { stats } if stats.total_messages == 2));
    run.wait("notice", |event| matches!(event, AgentEvent::Notice { .. }))
        .await;
    assert!(run.is_listed());

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn remaining_actions_map_to_replies_and_follow_up_events() {
    let run = start_omp("omp_actions.jsonl").await;
    let send = |action: AgentAction| run.manager.send(run.key(), action);

    let levels = send(AgentAction::GetAvailableThinkingLevels).await.unwrap();
    assert!(matches!(levels, AgentReply::ThinkingLevels { levels } if levels.len() == 4));
    let exported = send(AgentAction::ExportHtml).await.unwrap();
    assert_eq!(
        exported,
        AgentReply::Exported {
            path: "/tmp/session.html".to_owned()
        }
    );
    assert_eq!(
        send(AgentAction::GetLastAssistantText).await.unwrap(),
        AgentReply::Text { text: None }
    );

    let model = AgentAction::SetModel {
        provider: "openai".to_owned(),
        model_id: "gpt-5".to_owned(),
    };
    assert_eq!(send(model).await.unwrap(), AgentReply::Ack);
    run.wait(
        "model change",
        |event| matches!(event, AgentEvent::ModelChanged { model: Some(model) } if model.id == "gpt-5"),
    )
    .await;

    assert_eq!(send(AgentAction::NewSession).await.unwrap(), AgentReply::Ack);
    run.wait_until("refreshed the state after a new session", |events| {
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::State { .. }))
            .count()
            == 2
    })
    .await;

    let rename = AgentAction::SetSessionName {
        name: "Renamed".to_owned(),
    };
    let error = send(rename).await.unwrap_err().to_string();
    assert!(error.contains("set_session_name failed"), "{error}");
    assert!(!error.contains("CANARY"), "agent errors are redacted: {error}");

    let output = run.manager.bash(run.key(), "ls -la").await.unwrap();
    assert_eq!((output.exit_code, output.cancelled), (Some(0), false));
    assert!(output.output.starts_with("total 0\n") && !output.output.contains("CANARY"));

    let clone = send(AgentAction::Clone).await.unwrap_err();
    assert_eq!(clone.code(), harness_business::model::IpcErrorCode::Unsupported);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn pi_has_no_handshake_and_completes_prompts_when_the_run_settles() {
    let run = start_pi("pi_basic.jsonl").await;
    assert_eq!(run.info.protocol_version, None);

    let reply = run.manager.send(run.key(), prompt("Hello")).await.unwrap();
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: None,
            disposition: Some("started".to_owned()),
        }
    );
    run.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;
    let events = run.events();

    let AgentEvent::State { state } = &events[1] else {
        panic!("state follows ready");
    };
    assert_eq!((state.session_id.as_str(), state.queued_count), ("pi-session-1", 2));

    let ids: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageStart { message_id, .. } | AgentEvent::TextDelta { message_id, .. } => {
                Some(message_id.as_str())
            }
            AgentEvent::MessageEnd { message_id, .. } => Some(message_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["msg-1", "msg-1", "msg-2", "msg-2", "msg-2", "msg-2"]);

    let touched = events.iter().find_map(|event| match event {
        AgentEvent::ToolEnd { touched_paths, .. } => Some(touched_paths.clone()),
        _ => None,
    });
    assert_eq!(touched.unwrap(), ["src/new.rs"]);

    let tail = kinds(&events[events.len() - 2..]);
    assert_eq!(tail, ["promptResult", "settled"]);
    assert!(matches!(
        &events[events.len() - 2],
        AgentEvent::PromptResult { request_id: Some(id), status: PromptStatus::Completed, .. } if id == "req-3"
    ));

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn pi_history_is_read_once_and_sliced_newest_first() {
    let run = start_pi("pi_basic.jsonl").await;
    run.manager.send(run.key(), prompt("Hello")).await.unwrap();
    run.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;

    let page = |cursor: Option<&str>| AgentAction::GetHistory {
        cursor: cursor.map(str::to_owned),
        limit: 2,
    };
    let AgentReply::History {
        messages,
        next_cursor,
        total,
    } = run.manager.send(run.key(), page(None)).await.unwrap()
    else {
        panic!("history reply");
    };

    assert_eq!((messages.len(), next_cursor.as_deref(), total), (2, Some("3"), Some(5)));
    assert_eq!(messages[1].role, MessageRole::Assistant);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn pi_live_commands_add_prompts_skills_and_extensions_but_builtins_win() {
    let run = start_pi("pi_basic.jsonl").await;

    let commands = run.catalog().await;
    let find = |name: &str| commands.iter().find(|command| command.name == name).unwrap();

    assert_eq!(find("fix").source, CommandSource::Prompt);
    assert_eq!(find("skill:plain-writing").invocation, CommandInvocation::PromptText);
    assert_eq!(find("setup").source, CommandSource::Extension);
    assert_eq!(find("new").source, CommandSource::Builtin);
    assert!(!serde_json::to_string(&commands).unwrap().contains(".pi/prompts"));

    run.manager.stop(run.key()).await.unwrap();
}
