//! Turns of the Claude Code chat engine against the scripted `claude` replaying recorded traffic:
//! a streamed reply, tool calls with touched paths, images, the prompt-taken call, the last reply,
//! and the CLI's requests the engine answers itself or shows (dialogs are in `claude_engine_asks.rs`).

use crate::claude_engine_support;

use std::sync::{Arc, Mutex};

use claude_engine_support::{BASIC_SESSION, image_prompt, kinds, marker, prompt, start, stop_cleanly};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, PromptStatus};
use pretty_assertions::assert_eq;

/// The events from the first `agentStart` on.
fn from_turn(events: &[AgentEvent]) -> Vec<AgentEvent> {
    let start = events
        .iter()
        .position(|event| matches!(event, AgentEvent::AgentStart))
        .expect("a turn");

    events[start..].to_vec()
}

#[tokio::test]
async fn a_prompt_streams_one_turn_and_settles() {
    let chat = start("claude_basic.jsonl").await;

    let reply = chat.send(prompt("Reply with exactly the word: pong")).await;
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned())
        }
    );

    let events = chat.settled(1).await;
    assert_eq!(
        kinds(&from_turn(&events)),
        [
            "agentStart",
            "turnStart",
            "messageStart",
            "textDelta",
            "messageEnd",
            "turnEnd",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::TextDelta { delta, .. } if delta == "pong"))
    );
    assert!(events.iter().any(|event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Completed,
                ..
            }
        )
    }));

    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ModelChanged { .. })),
        "system/init names the default's model, reported from the start"
    );

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.session_id, BASIC_SESSION);
    assert_eq!(state.model.map(|model| model.id).as_deref(), Some("opus"));
    assert!(!state.is_streaming);
    assert_eq!(state.queued_count, 0);

    assert_eq!(
        chat.send(AgentAction::GetLastAssistantText).await,
        AgentReply::Text {
            text: Some("pong".to_owned())
        }
    );
    assert_eq!(
        chat.send(AgentAction::Abort).await,
        AgentReply::Ack,
        "nothing to stop once settled"
    );

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn the_prompt_taken_call_runs_once_claude_queued_the_prompt_before_the_turn_starts() {
    let chat = start("claude_basic.jsonl").await;
    let seen: Arc<Mutex<Option<Vec<AgentEvent>>>> = Arc::default();

    let sink = chat.sink.clone();
    let record = seen.clone();
    chat.session().on_prompt_taken(Box::new(move || {
        *record.lock().unwrap() = Some(sink.events());
    }));

    chat.send(prompt("Reply with exactly the word: pong")).await;
    chat.settled(1).await;

    let before = seen.lock().unwrap().clone().expect("the call ran");
    assert!(
        !before.iter().any(|event| matches!(event, AgentEvent::AgentStart)),
        "it runs before the turn's events: {before:#?}"
    );
    assert_eq!(kinds(&before[..3]), ["ready", "state", "commandsUpdated"]);

    chat.stop().await;
}

#[tokio::test]
async fn tool_calls_end_with_their_results_and_the_project_files_they_touched() {
    let chat = start("claude_tools.jsonl").await;
    chat.send(prompt("Read, write, then list")).await;
    let events = chat.settled(1).await;

    let ends: Vec<(String, bool, Vec<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd {
                name,
                is_error,
                touched_paths,
                ..
            } => Some((name.clone(), *is_error, touched_paths.clone())),
            _ => None,
        })
        .collect();

    assert_eq!(
        ends,
        [
            ("Read".to_owned(), false, vec![]),
            ("Write".to_owned(), false, vec!["hello.txt".to_owned()]),
            ("Bash".to_owned(), true, vec![]),
        ]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::TurnStart))
            .count(),
        4,
        "one model step per API message"
    );

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_prompt_with_an_image_sends_a_text_and_an_image_block() {
    let chat = start("claude_image.jsonl").await;

    chat.send(image_prompt()).await;
    marker(&chat).await;

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn hook_callbacks_and_unknown_requests_are_answered_and_permission_prompts_wait_in_a_dialog() {
    let chat = start("claude_requests.jsonl").await;
    let events = marker(&chat).await;

    assert!(
        events.iter().any(
            |event| matches!(event, AgentEvent::UiRequest { request } if request.id == "f9c23b59-dafb-4bcb-aa8f-fd0d3dd816c9")
        ),
        "the permission prompt waits in its dialog: {events:#?}"
    );

    stop_cleanly(&chat).await;
}
