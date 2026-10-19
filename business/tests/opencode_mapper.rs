//! The pure side of the OpenCode chat engine: the server's JSON (`wire`) and its events as chat
//! events (`Mapper`), against the recorded `opencode serve` run (`fixtures/opencode/server`) and
//! slices of it.

use std::path::{Path, PathBuf};

use harness_business::model::{AgentEvent, ChatBlock, ImageAttachment, MessageRole, NoticeLevel, PromptStatus};
use harness_business::opencode_server::wire::{
    ModelRef, envelope, is_default_title, listening_addr, primary_agents, prompt_body, provider_models, session_info,
};
use harness_business::opencode_server::{Mapped, Mapper};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/opencode/server")
        .join(name)
}

/// The `body` of a recorded `{request, status, body}` file.
fn recorded_body(name: &str) -> Value {
    let text = std::fs::read_to_string(fixture_path(name)).expect("recording");
    let recorded: Value = serde_json::from_str(&text).expect("recording is JSON");

    recorded["body"].clone()
}

/// Every `data:` payload of the recorded event stream, in order.
fn recorded_events() -> Vec<String> {
    let bytes = std::fs::read(fixture_path("global-event.sse")).expect("recorded stream");

    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(str::to_owned)
        .collect()
}

/// A mapper for the recorded session in a project that does not exist here.
fn mapper() -> Mapper {
    Mapper::new(SESSION, Path::new("/work/project"), Path::new("/work/project"))
}

/// The events of `mapped`, dropping the other outputs.
fn events(mapped: Vec<Mapped>) -> Vec<AgentEvent> {
    mapped
        .into_iter()
        .filter_map(|item| match item {
            Mapped::Event(event) => Some(event),
            _ => None,
        })
        .collect()
}

/// Maps each `{type, properties}` payload in order.
fn feed(mapper: &mut Mapper, payloads: &[Value]) -> Vec<Mapped> {
    payloads
        .iter()
        .flat_map(|payload| {
            let data = json!({ "directory": "/work/project", "payload": payload }).to_string();
            mapper.map(&envelope(&data).expect("event object"))
        })
        .collect()
}

fn status(kind: &str) -> Value {
    json!({ "type": "session.status", "properties": { "sessionID": SESSION, "status": { "type": kind } } })
}

fn assistant(id: &str, completed: bool) -> Value {
    let mut info = json!({ "id": id, "role": "assistant", "sessionID": SESSION, "modelID": "m1",
        "providerID": "mock", "time": { "created": 1 } });

    if completed {
        info["time"]["completed"] = json!(2);
        info["finish"] = json!("stop");
    }

    json!({ "type": "message.updated", "properties": { "sessionID": SESSION, "info": info } })
}

fn part(session: &str, part: Value) -> Value {
    let mut part = part;
    part["sessionID"] = json!(session);

    json!({ "type": "message.part.updated", "properties": { "sessionID": session, "part": part } })
}

fn delta(message: &str, part: &str, text: &str) -> Value {
    json!({ "type": "message.part.delta", "properties": { "sessionID": SESSION, "messageID": message,
        "partID": part, "field": "text", "delta": text } })
}

fn kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).expect("event serializes")["type"]
        .as_str()
        .expect("type tag")
        .to_owned()
}

#[test]
fn the_listening_line_gives_the_server_address() {
    let addr = listening_addr("opencode server listening on http://127.0.0.1:4096\n");
    assert_eq!(addr, Some("127.0.0.1:4096".parse().expect("address")));

    let ipv6 = listening_addr("opencode server listening on http://[::1]:39511");
    assert_eq!(ipv6, Some("[::1]:39511".parse().expect("address")));

    assert_eq!(listening_addr("Performing one time database migration..."), None);
    assert_eq!(
        listening_addr("opencode server listening on http://localhost:4096"),
        None
    );
}

#[test]
fn sessions_read_their_title_agent_and_model() {
    let created = session_info(&recorded_body("session-create.json")).expect("session");
    assert_eq!(created.id, SESSION);
    assert_eq!(created.directory, "/work/project");
    assert_eq!(created.title, None, "a default title is not a name");
    assert_eq!(created.model, None);

    let resumed = session_info(&recorded_body("session-get.json")).expect("session");
    assert_eq!(resumed.title.as_deref(), Some("Renamed chat"));
    assert_eq!(resumed.agent.as_deref(), Some("build"));
    assert_eq!(
        resumed.model,
        Some(ModelRef {
            provider: "mock".to_owned(),
            id: "m1".to_owned()
        })
    );
    assert_eq!(resumed.variant, None, "the `default` variant is no variant");

    assert!(is_default_title("Child session - 2026-10-19T19:36:01.225Z"));
    assert!(!is_default_title("New session - about the parser"));
}

#[test]
fn providers_give_display_fields_and_variants() {
    let models = provider_models(&recorded_body("config-providers.json"));

    let mock = models
        .iter()
        .find(|model| model.option.provider == "mock")
        .expect("mock model");
    assert_eq!(mock.option.id, "m1");
    assert_eq!(mock.option.name.as_deref(), Some("Mock One"));
    assert_eq!(mock.option.context_window, Some(100_000));
    assert_eq!(mock.option.max_tokens, Some(4096));
    assert_eq!(mock.option.input, vec!["text".to_owned()]);
    assert!(!mock.option.reasoning);
    assert!(mock.variants.is_empty());

    let longcat = models
        .iter()
        .find(|model| model.option.id == "longcat-2.5-preview-free")
        .expect("variant model");
    assert_eq!(longcat.variants, vec!["low", "medium", "high"]);
    assert_eq!(longcat.option.input, vec!["text".to_owned(), "image".to_owned()]);
    assert!(longcat.option.reasoning);

    let shown =
        serde_json::to_string(&models.iter().map(|model| &model.option).collect::<Vec<_>>()).expect("serializes");
    assert!(
        !shown.contains("baseURL") && !shown.contains("apiKey"),
        "no provider options"
    );
}

#[test]
fn only_visible_primary_agents_are_offered() {
    let agents = primary_agents(&recorded_body("agent-list.json"));
    let ids: Vec<&str> = agents.iter().map(|agent| agent.id.as_str()).collect();

    assert_eq!(ids, ["build", "plan"]);
    assert_eq!(
        agents[1].description.as_deref(),
        Some("Plan mode. Disallows all edit tools.")
    );
}

#[test]
fn the_prompt_body_carries_images_agent_model_and_variant_but_never_tools() {
    let image = ImageAttachment {
        data: "iVBORw0KGgo=".to_owned(),
        mime_type: "image/png".to_owned(),
    };
    let model = ModelRef {
        provider: "mock".to_owned(),
        id: "m1".to_owned(),
    };

    let full = prompt_body("Look", &[image], "plan", Some(&model), Some("high"));
    assert_eq!(
        full,
        json!({
            "parts": [
                { "type": "text", "text": "Look" },
                { "type": "file", "mime": "image/png", "filename": "image-1.png",
                  "url": "data:image/png;base64,iVBORw0KGgo=" },
            ],
            "agent": "plan",
            "model": { "providerID": "mock", "modelID": "m1" },
            "variant": "high",
        })
    );

    let plain = prompt_body("Say hello", &[], "build", None, None);
    assert_eq!(
        plain,
        json!({ "parts": [{ "type": "text", "text": "Say hello" }], "agent": "build" })
    );
}

#[test]
fn the_recorded_run_maps_to_settled_turns_of_its_own_session() {
    let mut mapper = mapper();
    let mut out = Vec::new();

    for data in recorded_events() {
        let envelope = envelope(&data).expect("every recorded event is an event object");
        out.extend(events(mapper.map(&envelope)));
    }

    let count = |name: &str| out.iter().filter(|event| kind(event) == name).count();

    // Hello, tool, edit, question, slow with a steer, abort, image, shell, compaction, bad model.
    assert_eq!(count("agentStart"), 10);
    assert_eq!(count("agentEnd"), 10);
    assert_eq!(count("settled"), 10);
    assert_eq!(count("promptResult"), 0, "no prompt was sent through this mapper");
    assert_eq!(count("notice"), 0, "the repeated model error is not shown again");
    assert_eq!(count("frameError"), 0);

    let aborted: Vec<_> = out
        .iter()
        .filter(|event| matches!(event, AgentEvent::Settled { aborted: Some(true) }))
        .collect();
    assert_eq!(aborted.len(), 1);

    let user_texts: Vec<String> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::User => {
                match message.blocks.first() {
                    Some(ChatBlock::Text { text }) => Some(text.clone()),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        user_texts,
        [
            "Say hello",
            "MAKE_TOOL now",
            "MAKE_EDIT now",
            "MAKE_QUESTION now",
            "SLOW answer",
            "steered message",
            "SLOW again",
            "Look",
            "x"
        ]
    );

    let tools: Vec<(String, bool)> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd { name, is_error, .. } => Some((name.clone(), *is_error)),
            _ => None,
        })
        .collect();
    let expected = [("bash", false), ("write", true), ("question", false), ("bash", false)];
    assert_eq!(tools, expected.map(|(name, error)| (name.to_owned(), error)).to_vec());

    let forks = ["msg_155aa031b002fyBhX1Yb6XFc7z", "msg_155aa0327002XTNdA98H1Pmb7R"];
    let shown = serde_json::to_string(&out).expect("events serialize");
    assert!(
        forks.iter().all(|id| !shown.contains(id)),
        "forked sessions are not this chat"
    );
}

#[test]
fn streamed_text_matches_the_final_text_of_each_reply() {
    let mut mapper = mapper();
    let mut out = Vec::new();

    for data in recorded_events() {
        out.extend(events(mapper.map(&envelope(&data).expect("event"))));
    }

    for event in &out {
        let AgentEvent::MessageEnd { message_id, message } = event else {
            continue;
        };

        if message.role != MessageRole::Assistant || message.stop_reason.as_deref() == Some("aborted") {
            continue;
        }

        let streamed: String = out
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TextDelta {
                    message_id: id, delta, ..
                } if id == message_id => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        let finished: String = message
            .blocks
            .iter()
            .filter_map(|block| match block {
                ChatBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(streamed, finished, "message {message_id}");
    }
}

#[test]
fn a_final_part_sends_only_the_text_not_streamed_yet() {
    let mut mapper = mapper();
    let message = "msg_1";
    let text = json!({ "id": "prt_t", "messageID": message, "type": "text", "text": "" });

    let out = events(feed(
        &mut mapper,
        &[
            status("busy"),
            assistant(message, false),
            part(SESSION, text),
            delta(message, "prt_t", "Hello "),
            part(
                SESSION,
                json!({ "id": "prt_t", "messageID": message, "type": "text", "text": "Hello world" }),
            ),
            part(
                SESSION,
                json!({ "id": "prt_t", "messageID": message, "type": "text", "text": "Rewritten" }),
            ),
            assistant(message, true),
        ],
    ));

    let deltas: Vec<(u32, &str)> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta {
                content_index, delta, ..
            } => Some((*content_index, delta.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, [(0, "Hello "), (0, "world")]);

    let Some(AgentEvent::MessageEnd { message, .. }) = out.last() else {
        panic!("the reply ends last: {out:#?}");
    };
    assert_eq!(
        message.blocks,
        [ChatBlock::Text {
            text: "Rewritten".to_owned()
        }]
    );
}

#[test]
fn a_prompt_is_taken_before_its_turn_starts_and_answered_when_it_ends() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    assert!(mapper.is_working());

    let mapped = feed(&mut mapper, &[status("busy"), status("busy"), status("idle")]);
    let shape: Vec<String> = mapped
        .iter()
        .map(|item| match item {
            Mapped::Event(event) => kind(event),
            Mapped::PromptTaken => "taken".to_owned(),
            Mapped::Session(_) => "session".to_owned(),
        })
        .collect();

    assert_eq!(shape, ["taken", "agentStart", "agentEnd", "promptResult", "settled"]);
    assert!(!mapper.is_working());

    mapper.prompt_sent();
    mapper.prompt_failed();
    assert!(!mapper.is_working(), "a prompt that failed to send is taken back");
}

#[test]
fn subagent_sessions_at_any_depth_report_their_tool_calls() {
    let mut mapper = mapper();
    let created = |id: &str, parent: &str| {
        json!({ "type": "session.created", "properties": { "sessionID": id,
            "info": { "id": id, "parentID": parent, "directory": "/work/project" } } })
    };
    let read = |session: &str, call: &str| {
        part(
            session,
            json!({ "id": format!("prt_{call}"), "messageID": "msg_c", "type": "tool", "tool": "read",
                "callID": call, "state": { "status": "completed", "input": { "filePath": "a.rs" },
                "output": "text", "title": "a.rs" } }),
        )
    };

    let out = events(feed(
        &mut mapper,
        &[
            created("ses_child", SESSION),
            created("ses_grandchild", "ses_child"),
            created("ses_stranger", "ses_unknown"),
            read("ses_grandchild", "call_deep"),
            read("ses_stranger", "call_stranger"),
            status("busy"),
            json!({ "type": "session.status", "properties": { "sessionID": "ses_child", "status": { "type": "idle" } } }),
        ],
    ));

    let ids: Vec<String> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd { tool_call_id, .. } => Some(tool_call_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["call_deep"]);
    assert_eq!(
        out.iter().map(kind).collect::<Vec<_>>(),
        ["toolStart", "toolEnd", "agentStart"],
        "a subagent going idle does not end the chat's turn"
    );
}

#[test]
fn errors_fail_the_turn_or_become_a_notice_once() {
    let mut mapper = mapper();
    mapper.prompt_sent();

    let error = |message: &str| {
        json!({ "type": "session.error", "properties": { "sessionID": SESSION,
            "error": { "name": "APIError", "data": { "message": message, "isRetryable": true } } } })
    };

    let out = events(feed(
        &mut mapper,
        &[
            status("busy"),
            error("Rate limited"),
            error("A later error"),
            status("idle"),
            error("APIError: Rate limited\n    at stack"),
            error("Something else"),
        ],
    ));

    let result = out
        .iter()
        .find(|event| matches!(event, AgentEvent::PromptResult { .. }))
        .expect("prompt result");
    assert_eq!(
        result,
        &AgentEvent::PromptResult {
            request_id: None,
            status: PromptStatus::Error,
            error: Some("Rate limited".to_owned()),
            retryable: true,
        }
    );

    let notices: Vec<&AgentEvent> = out
        .iter()
        .filter(|event| matches!(event, AgentEvent::Notice { .. }))
        .collect();
    assert_eq!(
        notices,
        [&AgentEvent::Notice {
            level: NoticeLevel::Error,
            message: "Something else".to_owned()
        }]
    );
}
