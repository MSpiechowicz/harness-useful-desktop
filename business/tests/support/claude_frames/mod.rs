//! Helpers of the crafted Claude mapper tests: the hints of mapped output and frames they build.
#![allow(dead_code)]

use std::path::Path;

use harness_business::claude_stream::mapper::{ClaudeMapper, Hint, Mapped, events};
use harness_business::claude_stream::wire::decode_value;
use serde_json::{Value, json};

/// A mapper for a chat in `/work/project`.
pub fn mapper() -> ClaudeMapper {
    ClaudeMapper::new(Path::new("/work/project"))
}

/// Everything `frames` map to, in order.
pub fn feed(mapper: &mut ClaudeMapper, frames: &[Value]) -> Vec<Mapped> {
    frames
        .iter()
        .flat_map(|frame| mapper.on_frame(&decode_value(frame.clone()).unwrap()))
        .collect()
}

/// The `type` tag of each event in `mapped`, in order.
pub fn kinds(mapped: &[Mapped]) -> Vec<String> {
    events(mapped)
        .iter()
        .map(|event| {
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// The hints of `mapped`, in order.
pub fn hints(mapped: &[Mapped]) -> Vec<Hint> {
    mapped
        .iter()
        .filter_map(|mapped| match mapped {
            Mapped::Hint(hint) => Some(hint.clone()),
            Mapped::Event(_) => None,
        })
        .collect()
}

/// `system/status requesting` serving prompt `uuid`.
pub fn requesting(uuid: &str) -> Value {
    json!({ "type": "system", "subtype": "status", "status": "requesting", "user_message_uuids": [uuid] })
}
