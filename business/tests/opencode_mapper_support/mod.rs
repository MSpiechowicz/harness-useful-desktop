//! Helpers of the OpenCode mapper tests: the recorded `opencode serve` run
//! (`fixtures/opencode/server`) and crafted events of its session.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use harness_business::model::AgentEvent;
use harness_business::opencode_server::wire::envelope;
use harness_business::opencode_server::{Mapped, Mapper};
use serde_json::{Value, json};

pub const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/opencode/server")
        .join(name)
}

/// The `body` of a recorded `{request, status, body}` file.
pub fn recorded_body(name: &str) -> Value {
    let text = std::fs::read_to_string(fixture_path(name)).expect("recording");
    let recorded: Value = serde_json::from_str(&text).expect("recording is JSON");

    recorded["body"].clone()
}

/// Every `data:` payload of the recorded event stream, in order.
pub fn recorded_events() -> Vec<String> {
    let bytes = std::fs::read(fixture_path("global-event.sse")).expect("recorded stream");

    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(str::to_owned)
        .collect()
}

/// A mapper for the recorded session in a project that does not exist here.
pub fn mapper() -> Mapper {
    Mapper::new(SESSION, Path::new("/work/project"), Path::new("/work/project"))
}

/// The events of `mapped`, dropping the other outputs.
pub fn events(mapped: Vec<Mapped>) -> Vec<AgentEvent> {
    mapped
        .into_iter()
        .filter_map(|item| match item {
            Mapped::Event(event) => Some(event),
            _ => None,
        })
        .collect()
}

/// Maps each `{type, properties}` payload in order.
pub fn feed(mapper: &mut Mapper, payloads: &[Value]) -> Vec<Mapped> {
    payloads
        .iter()
        .flat_map(|payload| {
            let data = json!({ "directory": "/work/project", "payload": payload }).to_string();
            mapper.map(&envelope(&data).expect("event object"))
        })
        .collect()
}

pub fn status(kind: &str) -> Value {
    json!({ "type": "session.status", "properties": { "sessionID": SESSION, "status": { "type": kind } } })
}

pub fn assistant(id: &str, completed: bool) -> Value {
    let mut info = json!({ "id": id, "role": "assistant", "sessionID": SESSION, "modelID": "m1",
        "providerID": "mock", "time": { "created": 1 } });

    if completed {
        info["time"]["completed"] = json!(2);
        info["finish"] = json!("stop");
    }

    json!({ "type": "message.updated", "properties": { "sessionID": SESSION, "info": info } })
}

pub fn part(session: &str, part: Value) -> Value {
    let mut part = part;
    part["sessionID"] = json!(session);

    json!({ "type": "message.part.updated", "properties": { "sessionID": session, "part": part } })
}

pub fn delta(message: &str, part: &str, text: &str) -> Value {
    json!({ "type": "message.part.delta", "properties": { "sessionID": SESSION, "messageID": message,
        "partID": part, "field": "text", "delta": text } })
}

pub fn kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).expect("event serializes")["type"]
        .as_str()
        .expect("type tag")
        .to_owned()
}

/// The kinds of `mapped`: event types, `taken`, `session`, `send:<text>`, `check:<texts>` and
/// `abort`.
pub fn shape(mapped: &[Mapped]) -> Vec<String> {
    mapped
        .iter()
        .map(|item| match item {
            Mapped::Event(event) => kind(event),
            Mapped::PromptTaken => "taken".to_owned(),
            Mapped::Session(_) => "session".to_owned(),
            Mapped::Abort => "abort".to_owned(),
            Mapped::Send(message) => format!("send:{}", message.text),
            Mapped::CheckStranded(steering) => {
                let texts: Vec<&str> = steering.iter().map(|message| message.text.as_str()).collect();
                format!("check:{}", texts.join("|"))
            }
        })
        .collect()
}
