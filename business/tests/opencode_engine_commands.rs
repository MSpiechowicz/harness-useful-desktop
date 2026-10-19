//! Slash commands, history, retries and the configured default model of the OpenCode chat engine
//! against the scripted `opencode serve`.

mod agent_support;
mod opencode_engine_support;

use harness_business::model::{AgentAction, AgentEvent, AgentReply, ChatBlock, MessageRole, PromptStatus};
use opencode_engine_support::{Chat, count, kind, prompt, start, until_marker};
use pretty_assertions::assert_eq;

/// Waits until `settled` arrived `times` times.
async fn settled(chat: &Chat, times: usize) {
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;

    while count(&chat.events(), "settled") < times {
        assert!(
            tokio::time::Instant::now() < deadline,
            "never settled {times} times: {:#?}",
            chat.events()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

fn history(limit: u32) -> AgentAction {
    AgentAction::GetHistory { cursor: None, limit }
}

#[tokio::test]
async fn server_commands_use_the_command_route_and_other_slashes_go_as_text() {
    let chat = start("opencode_commands.jsonl").await;

    prompt(&chat, "/nonexistent arg").await;
    settled(&chat, 1).await;

    let taken_at = prompt(&chat, "/review the parser").await;
    settled(&chat, 2).await;

    prompt(&chat, "/init").await;
    let events = until_marker(&chat).await;

    let results: Vec<(PromptStatus, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, error, .. } => Some((*status, error.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        results,
        [
            (PromptStatus::Completed, None),
            (PromptStatus::Completed, None),
            (
                PromptStatus::Error,
                Some("protocol error: the OpenCode server answered HTTP 400".to_owned())
            ),
        ]
    );
    assert_eq!(
        count(&events, "settled"),
        3,
        "a command that fails before its turn settles alone"
    );

    let second_start = events
        .iter()
        .enumerate()
        .filter(|(_, event)| **event == AgentEvent::AgentStart)
        .nth(1)
        .map(|(index, _)| index + 3)
        .expect("the command's turn");
    let taken_at = taken_at.lock().unwrap().expect("taken");
    assert!(taken_at <= second_start, "the command was taken when its turn started");
    assert!(chat.session().is_running(), "every request matched the scenario");

    chat.session().stop().await;
}

#[tokio::test]
async fn history_reads_every_page_and_serves_the_requested_one() {
    let chat = start("opencode_history.jsonl").await;
    let session = chat.session();

    let AgentReply::History {
        messages,
        next_cursor,
        total,
    } = session.send(history(200)).await.expect("history")
    else {
        panic!("not a history reply");
    };

    assert_eq!(next_cursor, None);
    assert_eq!(total, u32::try_from(messages.len()).ok());
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(
        messages[0].blocks,
        [ChatBlock::Text {
            text: "Say hello".to_owned()
        }]
    );
    assert!(
        messages.iter().any(|message| message.role == MessageRole::ToolResult),
        "tool results follow their calls"
    );
    let ids: Vec<Option<String>> = messages.iter().map(|message| message.id.clone()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids.first(), sorted.first(), "oldest first across both pages");

    let AgentReply::History {
        messages: newest,
        next_cursor,
        ..
    } = session.send(history(2)).await.expect("history")
    else {
        panic!("not a history reply");
    };

    assert_eq!(newest, messages[messages.len() - 2..]);
    assert_eq!(next_cursor, Some((messages.len() - 2).to_string()));
    assert!(session.is_running(), "both reads paged as scripted");

    session.stop().await;
}

#[tokio::test]
async fn a_retried_model_call_reports_its_start_and_end() {
    let chat = start("opencode_retry.jsonl").await;
    prompt(&chat, "Say hello").await;
    let events = until_marker(&chat).await;

    let retries: Vec<&AgentEvent> = events.iter().filter(|event| kind(event) == "retry").collect();
    assert_eq!(
        retries,
        [
            &AgentEvent::Retry {
                phase: "start".to_owned(),
                attempt: Some(1),
                max_attempts: None,
                message: Some("Provider is overloaded".to_owned()),
            },
            &AgentEvent::Retry {
                phase: "end".to_owned(),
                attempt: Some(1),
                max_attempts: None,
                message: None,
            },
        ]
    );
    assert_eq!(count(&events, "settled"), 1);

    chat.session().stop().await;
}

#[tokio::test]
async fn a_new_chat_shows_the_configured_model_but_leaves_it_to_opencode() {
    let chat = start("opencode_config_model.jsonl").await;

    let state = match chat
        .wait("state", |event| matches!(event, AgentEvent::State { .. }))
        .await
    {
        AgentEvent::State { state } => state,
        _ => unreachable!(),
    };
    let model = state.model.expect("the configured model");
    assert_eq!((model.provider.as_str(), model.id.as_str()), ("mock", "m1"));
    assert_eq!(model.name.as_deref(), Some("Mock One"));

    // The scenario requires the prompt without a model.
    prompt(&chat, "Say hello").await;
    assert!(chat.session().is_running());

    chat.session().stop().await;
}
