//! Helpers of the OpenCode 2 mapper tests: the recorded `opencode serve` runs
//! (`fixtures/opencode2/recorded`, via `opencode2_recorded`) replayed through a `Mapper` the way the
//! chat engine drives it, and crafted events of one session.
#![allow(dead_code)]

use std::path::Path;

use harness_business::model::{
    AgentEvent, ChatBlock, ChatMessage, MessageRole, UiOutcome, UiRequest, UiRequestKind, UiResponse,
};
use harness_business::opencode_server::wire::Outgoing;
use harness_business::opencode_server_v2::events::{Event, parse};
use harness_business::opencode_server_v2::mapper::{Answer, Mapped, Mapper, QuietKind, Reply, compaction_started};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::opencode2_recorded::{DIRECTORY, recordings};

/// The session of crafted events.
pub const SESSION: &str = "ses_test";

/// The option of a single-choice question that asks for a typed answer.
pub const OWN_ANSWER: &str = "Type your own answer…";

/// A recording replayed through a mapper.
pub struct Replay {
    /// The project the recorded directory stands for (it must exist for touched paths).
    pub project: TempDir,
    /// The recorded chat session.
    pub session: String,
    pub mapper: Mapper,
    /// Everything the mapper said, in order.
    pub out: Vec<Mapped>,
    /// The replies the recorded answers to dialogs became.
    pub replies: Vec<Reply>,
}

impl Replay {
    /// The events among [`Replay::out`].
    pub fn events(&self) -> Vec<AgentEvent> {
        events(self.out.clone())
    }

    /// How many events of `kind` were emitted.
    pub fn count(&self, kind: &str) -> usize {
        self.events().iter().filter(|event| self::kind(event) == kind).count()
    }
}

/// Replays recording `file`: its events through [`Mapper::map`], its HTTP exchanges as the
/// engine's calls (prompts, steering, `!command`s, compactions, interrupts, and the answers to
/// dialogs, given through [`Mapper::answer`]). The recorded directory becomes a temporary project.
pub fn replay(file: &str) -> Replay {
    let rows = recordings()
        .into_iter()
        .find(|(name, _)| name == file)
        .map(|(_, rows)| rows)
        .expect("recording");

    let project = TempDir::new().expect("project");
    let root = dunce::canonicalize(project.path()).expect("canonical project");
    let root_text = root.to_string_lossy().into_owned();

    let session = rows
        .iter()
        .find(|row| row["kind"] == "sse" && row["event"]["type"] == "session.created")
        .and_then(|row| row["event"]["data"]["sessionID"].as_str())
        .expect("a created session")
        .to_owned();

    let mut replay = Replay {
        mapper: Mapper::new(&session, &root, &root),
        project,
        session,
        out: Vec::new(),
        replies: Vec::new(),
    };

    for row in rows {
        match row["kind"].as_str() {
            Some("sse") => {
                let data = row["event"].to_string().replace(DIRECTORY, &root_text);
                let event = parse(&data).expect("recorded event");
                let mapped = replay.mapper.map(&event);
                replay.out.extend(mapped);
            }
            Some("http") => exchange(&mut replay, &row),
            _ => {}
        }
    }

    replay
}

/// One recorded HTTP exchange as the engine's call.
fn exchange(replay: &mut Replay, row: &Value) {
    let method = row["method"].as_str().unwrap_or_default();
    let path = row["path"]
        .as_str()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();
    let body = &row["body"];
    let ok = row["status"].as_u64().is_some_and(|status| status < 300);
    let mapper = &mut replay.mapper;

    if method != "POST" {
        return;
    }

    if path.ends_with("/prompt") {
        let text = body["text"].as_str().unwrap_or_default();
        let steer = body["delivery"] == "steer";

        if steer && mapper.is_working() {
            let update = mapper.steer_sent(Outgoing::new(text, &[]));
            replay.out.extend(update.into_iter().map(Mapped::event));

            if let Some(id) = row["response"]["data"]["id"].as_str() {
                mapper.steer_accepted(text, id);
            }
        } else {
            mapper.prompt_sent();
        }
    } else if path.ends_with("/command") {
        mapper.prompt_sent();
    } else if path.ends_with("/shell") && !mapper.is_working() {
        mapper.quiet_begin(QuietKind::Shell);
        let mapped = mapper.quiet_answered(ok, None);
        replay.out.extend(mapped);
    } else if path.ends_with("/compact") && !mapper.is_working() {
        mapper.quiet_begin(QuietKind::Compaction);
        replay.out.push(Mapped::event(compaction_started()));
        let mapped = mapper.quiet_answered(ok, None);
        replay.out.extend(mapped);
    } else if path.ends_with("/interrupt") {
        mapper.abort_requested();
    } else if path.contains("/permission/") && path.ends_with("/reply") {
        let id = segment_after(&path, "permission");
        let label = match body["decision"].as_str() {
            Some("once") => "Allow once",
            Some("always") => "Always allow",
            _ => "Reject",
        };
        let reply = send(mapper.answer(&value(&id, label)).expect("an open permission dialog"));
        replay.replies.push(reply);
    } else if path.contains("/form/") && path.ends_with("/reply") {
        let id = segment_after(&path, "form");
        let answers: Vec<String> = body["answer"]
            .as_object()
            .into_iter()
            .flat_map(|answer| answer.values())
            .map(|value| value.as_str().unwrap_or_default().to_owned())
            .collect();
        let reply = answer_form(replay, &id, &answers);
        replay.replies.push(reply);
    }
}

/// Answers form `id` field by field with `answers`, typing those that are not an option.
fn answer_form(replay: &mut Replay, id: &str, answers: &[String]) -> Reply {
    let mut dialog = last_request(&replay.out, id).expect("the form's dialog");

    for answer in answers {
        let options = match &dialog.kind {
            UiRequestKind::Select { options, .. } => options.clone(),
            _ => Vec::new(),
        };

        let mut response = value(&dialog.id, answer);

        if !options.is_empty() && !options.contains(answer) {
            match replay
                .mapper
                .answer(&value(&dialog.id, OWN_ANSWER))
                .expect("own answer")
            {
                Answer::Next(next) => dialog = request(&next[0]).clone(),
                Answer::Send(_) => panic!("the typed answer comes next"),
            }
            response = value(&dialog.id, answer);
        }

        match replay.mapper.answer(&response).expect("an answer") {
            Answer::Next(next) => dialog = request(&next[0]).clone(),
            Answer::Send(reply) => return reply,
        }
    }

    panic!("form {id} was not answered");
}

fn segment_after(path: &str, name: &str) -> String {
    let mut segments = path.split('/');
    segments.find(|segment| *segment == name);
    segments.next().expect("an id").to_owned()
}

/// The newest dialog whose id starts with `prefix`.
fn last_request(out: &[Mapped], prefix: &str) -> Option<UiRequest> {
    out.iter().rev().find_map(|item| match item {
        Mapped::Event(event) => match event.as_ref() {
            AgentEvent::UiRequest { request } if request.id.starts_with(prefix) => Some(request.clone()),
            _ => None,
        },
        _ => None,
    })
}

/// The reply of an answer that sends one.
pub fn send(answer: Answer) -> Reply {
    match answer {
        Answer::Send(reply) => reply,
        Answer::Next(_) => panic!("expected a reply, the form goes on"),
    }
}

/// A dialog answer with `value`.
pub fn value(id: &str, value: &str) -> UiResponse {
    UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Value {
            value: value.to_owned(),
        },
    }
}

/// A dismissed dialog.
pub fn dismissed(id: &str) -> UiResponse {
    UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Cancelled,
    }
}

/// The dialog of a `uiRequest` event.
pub fn request(event: &AgentEvent) -> &UiRequest {
    match event {
        AgentEvent::UiRequest { request } => request,
        other => panic!("not a dialog: {other:?}"),
    }
}

/// Every dialog among `events`.
pub fn requests(events: &[AgentEvent]) -> Vec<UiRequest> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::UiRequest { request } => Some(request.clone()),
            _ => None,
        })
        .collect()
}

/// A mapper for [`SESSION`] in a project that does not exist here.
pub fn mapper() -> Mapper {
    Mapper::new(SESSION, Path::new("/work/proj"), Path::new("/work/proj"))
}

/// An event of `kind` with `data`, created at `1`.
pub fn event(kind: &str, data: Value) -> Event {
    let envelope = json!({ "id": "evt_1", "created": 1, "type": kind, "data": data });

    parse(&envelope.to_string()).expect("a crafted event")
}

/// An event of `kind` of [`SESSION`]: `data` gets its `sessionID`.
pub fn ours(kind: &str, mut data: Value) -> Event {
    data["sessionID"] = json!(SESSION);
    event(kind, data)
}

/// Maps `events` in order.
pub fn feed(mapper: &mut Mapper, events: &[Event]) -> Vec<Mapped> {
    events.iter().flat_map(|event| mapper.map(event)).collect()
}

/// The events of `mapped`, dropping the other outputs.
pub fn events(mapped: Vec<Mapped>) -> Vec<AgentEvent> {
    mapped
        .into_iter()
        .filter_map(|item| match item {
            Mapped::Event(event) => Some(*event),
            _ => None,
        })
        .collect()
}

/// The `type` of an event.
pub fn kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).expect("event serializes")["type"]
        .as_str()
        .expect("type tag")
        .to_owned()
}

/// The kinds of `mapped`: event types, `taken`, `session`, `send:<text>`, `abort`, `context`,
/// `usage` and `shell`.
pub fn shape(mapped: &[Mapped]) -> Vec<String> {
    mapped
        .iter()
        .map(|item| match item {
            Mapped::Event(event) => kind(event),
            Mapped::PromptTaken => "taken".to_owned(),
            Mapped::Session(_) => "session".to_owned(),
            Mapped::Send(message) => format!("send:{}", message.text),
            Mapped::Abort => "abort".to_owned(),
            Mapped::Context(_) => "context".to_owned(),
            Mapped::Usage(_) => "usage".to_owned(),
            Mapped::ShellDone(_) => "shell".to_owned(),
        })
        .collect()
}

/// The messages that ended with `role`, in order.
pub fn ended(events: &[AgentEvent], role: MessageRole) -> Vec<ChatMessage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == role => Some(message.clone()),
            _ => None,
        })
        .collect()
}

/// The text blocks of `message`, joined.
pub fn text_of(message: &ChatMessage) -> String {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// `(name, is_error, touched paths)` of every `toolEnd`.
pub fn tool_ends(events: &[AgentEvent]) -> Vec<(String, bool, Vec<String>)> {
    events
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
        .collect()
}

/// The text of a tool result (`{content: [{type: text, text}]}`).
pub fn result_text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_owned()
}
