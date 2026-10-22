//! Replays a Claude Code recording through `ClaudeMapper` as the engine drives it: every user
//! message the app wrote is announced with `sent` (its kind read from `priority`, `shouldQuery`
//! and a `/compact` text), an `interrupt` request calls `abort_requested`, and every frame the
//! CLI wrote is mapped.

#![allow(dead_code)]

use std::path::Path;

use harness_business::claude_stream::mapper::{ClaudeMapper, Hint, Mapped, SendKind, events};
use harness_business::claude_stream::wire::decode_value;
use harness_business::model::{AgentEvent, ChatMessage, MessageRole};
use serde_json::Value;

use crate::claude_recorded::{Dir, PROJECT, moved, rows};

/// A replayed recording.
pub struct Replay {
    pub mapper: ClaudeMapper,
    pub out: Vec<Mapped>,
}

impl Replay {
    pub fn events(&self) -> Vec<AgentEvent> {
        events(&self.out)
    }

    pub fn hints(&self) -> Vec<Hint> {
        self.out
            .iter()
            .filter_map(|mapped| match mapped {
                Mapped::Hint(hint) => Some(hint.clone()),
                Mapped::Event(_) => None,
            })
            .collect()
    }

    /// How many events of `kind` (the serialized `type`).
    pub fn count(&self, kind: &str) -> usize {
        self.events().iter().filter(|event| self::kind(event) == kind).count()
    }
}

/// Replays `file` with the recorded project directory moved to `project`.
pub fn replay_in(file: &str, project: &Path) -> Replay {
    let mut mapper = ClaudeMapper::new(project);
    let mut out = Vec::new();

    for row in rows(file) {
        match row.dir {
            Dir::In => announce(&mut mapper, &row.frame),
            Dir::Out => {
                let frame = serde_json::from_str(&moved(&row.frame, project)).unwrap();
                let frame = decode_value(frame).unwrap_or_else(|error| panic!("{file}: {error:?}"));
                out.extend(mapper.on_frame(&frame));
            }
            Dir::Other => {}
        }
    }

    Replay { mapper, out }
}

/// Replays `file` in the recorded project directory.
pub fn replay(file: &str) -> Replay {
    replay_in(file, Path::new(PROJECT))
}

fn announce(mapper: &mut ClaudeMapper, frame: &Value) {
    match frame["type"].as_str() {
        Some("user") => {
            let uuid = frame["uuid"].as_str().unwrap_or_default();
            let text = frame["message"]["content"].as_str().unwrap_or_default();
            let kind = if frame["shouldQuery"] == false {
                SendKind::Quiet
            } else if text.starts_with("/compact") {
                SendKind::Compact
            } else {
                match frame["priority"].as_str() {
                    Some("next") => SendKind::Steer,
                    Some("later") => SendKind::FollowUp,
                    _ => SendKind::Prompt,
                }
            };
            mapper.sent(uuid, kind);
        }
        Some("control_request") if frame["request"]["subtype"] == "interrupt" => mapper.abort_requested(),
        _ => {}
    }
}

/// The serialized `type` of `event`.
pub fn kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).unwrap()["type"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The kinds of `events`, with runs of deltas folded into one entry.
pub fn shape(events: &[AgentEvent]) -> Vec<String> {
    let mut shape: Vec<String> = Vec::new();

    for event in events {
        let kind = kind(event);

        if kind.ends_with("Delta") && shape.last() == Some(&kind) {
            continue;
        }

        shape.push(kind);
    }

    shape
}

/// The messages that ended, by role.
pub fn ended(events: &[AgentEvent], role: MessageRole) -> Vec<ChatMessage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == role => Some(message.clone()),
            _ => None,
        })
        .collect()
}

/// The text streamed into message `id` (text deltas only).
pub fn streamed_text(events: &[AgentEvent], id: &str) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { message_id, delta, .. } if message_id == id => Some(delta.as_str()),
            _ => None,
        })
        .collect()
}
