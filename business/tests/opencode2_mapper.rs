//! The OpenCode 2 chat engine's event mapping (`opencode_server_v2::mapper::Mapper`): the
//! recorded `opencode serve` runs (`fixtures/opencode2/recorded`) replayed as the engine drives
//! the mapper, and crafted events for turns, steps, errors and retries. The output matches what
//! the OpenCode 1 mapper gives for the same run.

mod opencode2_mapper_support;
mod opencode2_recorded;

use harness_business::model::{AgentEvent, ChatBlock, MessageRole, PromptStatus};
use harness_business::opencode_server_v2::mapper::{ContextNote, Mapped, SessionChange, history};
use harness_business::opencode_server_v2::messages::SessionMessage;
use opencode2_mapper_support::{
    ended, event, events, feed, kind, mapper, ours, replay, result_text, shape, text_of, tool_ends,
};
use pretty_assertions::assert_eq;
use serde_json::json;

const RECORDINGS: [&str; 11] = [
    "busy.jsonl",
    "edit_once.jsonl",
    "edit_reject.jsonl",
    "image.jsonl",
    "interrupt.jsonl",
    "ops.jsonl",
    "question.jsonl",
    "shell_command.jsonl",
    "steer_queue.jsonl",
    "tools.jsonl",
    "turn.jsonl",
];

fn replay_events(file: &str) -> Vec<AgentEvent> {
    replay(file).events()
}

#[test]
fn every_recorded_run_maps_to_turns_that_end_and_settle() {
    for file in RECORDINGS {
        let replay = replay(file);
        let starts = replay.count("agentStart");

        // `edit_reject` stops recording while a permission is still asked.
        let open = usize::from(file == "edit_reject.jsonl");
        assert!(starts > 0, "{file}");
        assert_eq!(replay.count("agentEnd") + open, starts, "{file}");
        assert_eq!(replay.count("settled") + open, starts, "{file}");
        assert_eq!(replay.count("notice"), 0, "{file}");
        assert_eq!(replay.mapper.is_working(), open == 1, "{file}");
    }
}

#[test]
fn a_turn_streams_its_reply_and_ends_with_usage_title_and_context() {
    let replay = replay("turn.jsonl");
    let events = replay.events();

    let kinds: Vec<String> = events.iter().map(kind).collect();
    assert_eq!(
        kinds,
        [
            "agentStart",
            "messageStart",
            "messageEnd",
            "messageStart",
            "textDelta",
            "messageEnd",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );
    assert_eq!(
        shape(&replay.out)[0],
        "taken",
        "the prompt is taken when the run starts"
    );

    let users = ended(&events, MessageRole::User);
    assert_eq!(text_of(&users[0]), "Reply with exactly the word: hello");
    assert_eq!(users[0].id.as_deref(), Some("msg_15d7bc2db001z7i7HF20ZowIPY"));

    let reply = &ended(&events, MessageRole::Assistant)[0];
    assert_eq!(text_of(reply), "hello");
    assert_eq!(reply.stop_reason.as_deref(), Some("stop"));
    assert_eq!(reply.model.as_deref(), Some("big-pickle"));
    assert_eq!(reply.provider.as_deref(), Some("opencode"));
    assert_eq!(reply.cost_usd, Some(0.0));
    let usage = reply.usage.as_ref().expect("usage");
    assert_eq!(
        (usage.input, usage.output, usage.cache_read, usage.total),
        (280, 3, 5120, 5403)
    );
    assert!(reply.timestamp.is_some());

    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Completed,
        error: None,
        retryable: false,
    }));
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(false) }));

    let title = Mapped::Session(SessionChange::Title("hello".to_owned()));
    assert!(replay.out.contains(&title));

    let context = replay.out.iter().find_map(|item| match item {
        Mapped::Context(ContextNote::Tokens { tokens, model }) => Some((*tokens, model.clone())),
        _ => None,
    });
    let (tokens, model) = context.expect("the step's context");
    assert_eq!(tokens, 5403);
    assert_eq!(model.map(|model| model.id), Some("big-pickle".to_owned()));
}

#[test]
fn tool_calls_become_cards_with_v2_ids_inputs_and_outputs() {
    let events = replay_events("tools.jsonl");

    let starts: Vec<(String, serde_json::Value)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolStart { name, args, .. } => Some((name.clone(), args.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        [
            ("read".to_owned(), json!({ "path": "notes.txt" })),
            ("glob".to_owned(), json!({ "pattern": "*.txt" }))
        ]
    );

    assert_eq!(
        tool_ends(&events),
        [
            ("read".to_owned(), false, Vec::new()),
            ("glob".to_owned(), false, Vec::new())
        ]
    );

    let read = events.iter().find_map(|event| match event {
        AgentEvent::ToolEnd { result, name, .. } if name == "read" => Some(result_text(result)),
        _ => None,
    });
    assert_eq!(read.as_deref(), Some("Read file notes.txt, lines 1-1\n1: first line"));

    // The first step's reply holds its text and both calls, in arrival order.
    let reply = &ended(&events, MessageRole::Assistant)[0];
    let blocks: Vec<String> = reply
        .blocks
        .iter()
        .map(|block| match block {
            ChatBlock::Text { text } => format!("text:{text}"),
            ChatBlock::ToolCall { id, name, .. } => format!("tool:{name}:{id}"),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        blocks,
        [
            "text:I'll read the notes file and list the txt files.",
            "tool:read:call_function_ergyosjl9cx3_1",
            "tool:glob:call_function_ergyosjl9cx3_2"
        ]
    );
    assert_eq!(reply.stop_reason.as_deref(), Some("tool-calls"));
}

#[test]
fn edits_report_the_files_they_touch() {
    assert_eq!(
        tool_ends(&replay_events("edit_once.jsonl")),
        [
            ("read".to_owned(), false, Vec::new()),
            ("edit".to_owned(), false, vec!["notes.txt".to_owned()]),
            ("shell".to_owned(), false, Vec::new())
        ]
    );

    let rejected = tool_ends(&replay_events("edit_reject.jsonl"));
    assert_eq!(
        rejected[0],
        ("write".to_owned(), true, vec!["notes.txt".to_owned()]),
        "a refused write still names its file, as on OpenCode 1"
    );
}

#[test]
fn a_rejected_permission_fails_its_call_with_the_reason() {
    let events = replay_events("edit_reject.jsonl");

    let error = events.iter().find_map(|event| match event {
        AgentEvent::ToolEnd {
            is_error: true, result, ..
        } => Some(result_text(result)),
        _ => None,
    });
    assert_eq!(error.as_deref(), Some("Do not change that file."));
}

#[test]
fn an_interrupt_aborts_the_running_call_and_settles_as_aborted() {
    let events = replay_events("interrupt.jsonl");

    assert_eq!(tool_ends(&events), [("write".to_owned(), true, Vec::new())]);

    let aborted = events.iter().find_map(|event| match event {
        AgentEvent::ToolEnd { result, .. } => Some(result_text(result)),
        _ => None,
    });
    assert_eq!(aborted.as_deref(), Some("Tool execution interrupted"));

    let reply = &ended(&events, MessageRole::Assistant)[0];
    assert_eq!(reply.stop_reason.as_deref(), Some("aborted"));
    assert_eq!(text_of(reply), "I'll write that for you.");

    let tail: Vec<String> = events.iter().rev().take(3).rev().map(kind).collect();
    assert_eq!(tail, ["agentEnd", "promptResult", "settled"]);
    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Aborted,
        error: None,
        retryable: false,
    }));
    assert_eq!(events.last(), Some(&AgentEvent::Settled { aborted: Some(true) }));
}

#[test]
fn steering_is_queued_until_the_run_takes_it() {
    let replay = replay("steer_queue.jsonl");
    let events = replay.events();

    let queues: Vec<Vec<String>> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::QueueUpdate { steering, .. } => Some(steering.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(queues, [vec!["Actually stop at 10.".to_owned()], Vec::new()]);

    let users: Vec<String> = ended(&events, MessageRole::User).iter().map(text_of).collect();
    assert_eq!(
        users,
        [
            "Count slowly from 1 to 40, one number per line.",
            "Actually stop at 10.",
            "Now say goodbye."
        ]
    );

    // The prompt and the server-queued prompt each get a result, steering does not.
    assert_eq!(replay.count("promptResult"), 2);
    assert_eq!(replay.count("agentStart"), 1);
}

#[test]
fn images_of_a_prompt_show_by_type() {
    let users = ended(&replay_events("image.jsonl"), MessageRole::User);

    assert_eq!(
        users[0].blocks,
        [
            ChatBlock::Text {
                text: "What colour is this image? One word.".to_owned()
            },
            ChatBlock::Image {
                mime_type: "image/png".to_owned()
            }
        ]
    );
}

#[test]
fn session_ops_rename_and_ignore_forks_and_deletion() {
    let replay = replay("ops.jsonl");

    let title = Mapped::Session(SessionChange::Title("Recorded title".to_owned()));
    assert!(replay.out.contains(&title));

    let shown = format!("{:?}", replay.out);
    for fork in ["ses_ea27da712ffe14Nxq8YVmHrHyr", "ses_ea27da70fffea5dJXkjhzIlcBu"] {
        assert!(!shown.contains(fork), "forks are other chats");
    }

    assert_eq!(replay.count("agentStart"), 2, "the compaction is not a turn");
}

#[test]
fn deltas_add_up_to_the_final_text_and_a_full_text_sends_only_the_rest() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    let message = json!("msg_a");

    let out = feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours(
                "session.step.started",
                json!({ "assistantMessageID": message, "agent": "build" }),
            ),
            ours(
                "session.reasoning.started",
                json!({ "assistantMessageID": message, "ordinal": 0 }),
            ),
            ours(
                "session.reasoning.delta",
                json!({ "assistantMessageID": message, "ordinal": 0, "delta": "think" }),
            ),
            ours(
                "session.text.started",
                json!({ "assistantMessageID": message, "ordinal": 0 }),
            ),
            ours(
                "session.text.delta",
                json!({ "assistantMessageID": message, "ordinal": 0, "delta": "Hello " }),
            ),
            ours(
                "session.text.ended",
                json!({ "assistantMessageID": message, "ordinal": 0, "text": "Hello world" }),
            ),
            ours(
                "session.reasoning.ended",
                json!({ "assistantMessageID": message, "ordinal": 0, "text": "think" }),
            ),
            ours(
                "session.step.ended",
                json!({ "assistantMessageID": message, "finish": "stop", "cost": 0.5,
                    "tokens": { "input": 1, "output": 2, "reasoning": 3, "cache": { "read": 4, "write": 5 } } }),
            ),
        ],
    );

    let deltas: Vec<(String, u32, String)> = events(out.clone())
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta {
                content_index, delta, ..
            } => Some(("text".to_owned(), content_index, delta)),
            AgentEvent::ThinkingDelta {
                content_index, delta, ..
            } => Some(("thinking".to_owned(), content_index, delta)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deltas,
        [
            ("thinking".to_owned(), 0, "think".to_owned()),
            ("text".to_owned(), 1, "Hello ".to_owned()),
            ("text".to_owned(), 1, "world".to_owned())
        ]
    );

    let reply = &ended(&events(out.clone()), MessageRole::Assistant)[0];
    assert_eq!(
        reply.blocks,
        [
            ChatBlock::Thinking {
                text: "think".to_owned(),
                redacted: false
            },
            ChatBlock::Text {
                text: "Hello world".to_owned()
            }
        ]
    );
    assert_eq!(reply.cost_usd, Some(0.5));
    assert_eq!(reply.usage.as_ref().map(|usage| usage.total), Some(15));
    assert_eq!(reply.timestamp, Some(1));
    assert!(out.contains(&Mapped::Context(ContextNote::Tokens {
        tokens: 15,
        model: None
    })));
}

#[test]
fn only_the_chats_own_session_counts() {
    let mut mapper = mapper();

    let other = event("session.execution.started", json!({ "sessionID": "ses_other" }));
    let global = event("server.connected", json!({}));

    assert!(feed(&mut mapper, &[other, global]).is_empty());
    assert!(!mapper.is_working());
}

#[test]
fn seen_messages_are_not_shown_again_and_history_reads_them() {
    let mut mapper = mapper();
    let page: Vec<SessionMessage> = [
        json!({ "type": "user", "id": "msg_u", "time": { "created": 1 }, "text": "old prompt" }),
        json!({ "type": "assistant", "id": "msg_a", "time": { "created": 2 },
            "content": [{ "type": "tool", "id": "call_1", "name": "shell",
                "state": { "status": "completed", "input": { "command": "ls" }, "content": [] }, "time": {} }] }),
    ]
    .iter()
    .map(SessionMessage::parse)
    .collect();

    mapper.seen(&page);

    let out = feed(
        &mut mapper,
        &[
            ours(
                "session.inbox.enqueued",
                json!({ "inboxID": "msg_u", "item": { "type": "user", "payload": { "text": "old prompt" } } }),
            ),
            ours("session.inbox.delivered", json!({ "inboxID": "msg_u" })),
            ours(
                "session.text.started",
                json!({ "assistantMessageID": "msg_a", "ordinal": 0 }),
            ),
            ours(
                "session.tool.input.started",
                json!({ "assistantMessageID": "msg_b", "id": "call_1", "name": "shell" }),
            ),
            ours(
                "session.tool.success",
                json!({ "assistantMessageID": "msg_b", "id": "call_1", "content": [] }),
            ),
        ],
    );
    assert!(out.is_empty(), "{out:?}");

    let roles: Vec<MessageRole> = history(&page).iter().map(|message| message.role).collect();
    assert_eq!(
        roles,
        [MessageRole::User, MessageRole::Assistant, MessageRole::ToolResult]
    );
}
