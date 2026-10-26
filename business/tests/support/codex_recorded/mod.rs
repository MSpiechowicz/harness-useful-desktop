//! The Codex 0.154.0 app-server traffic recorded on a ChatGPT login
//! (`fixtures/codex/app_server/*.jsonl`, one process across the files in order): one row per
//! frame, `{dir, frame}`, where `dir` is `in` (Codex → app) or `out` (app → Codex). The project
//! directory is `/work/project`; accounts, paths and long lists are scrubbed.
//!
//! [`replay`] drives a `CodexMapper` as the engine does: a `turn/start`, `thread/compact/start` or
//! `thread/shellCommand` the app wrote is announced with `sent`, a `turn/interrupt` calls
//! `abort_requested`, and every frame Codex wrote is mapped.

#![allow(dead_code)]

use std::fs;
use std::path::Path;

use harness_business::codex_rpc::mapper::{CodexMapper, Hint, Mapped, StartKind, events};
use harness_business::codex_rpc::wire::decode_value;
use harness_business::model::AgentEvent;
use serde::Deserialize;
use serde_json::Value;

/// The recorded project directory.
pub const PROJECT: &str = "/work/project";

/// The recorded thread.
pub const THREAD: &str = "01a164a0-4c41-7613-9735-20de2a8016a8";

/// Every recording, in the order they were recorded.
pub const FILES: [&str; 10] = [
    "01_handshake.jsonl",
    "02_turn_approvals_accept.jsonl",
    "03_turn_decline.jsonl",
    "04_steer.jsonl",
    "05_interrupt.jsonl",
    "06_ops.jsonl",
    "07_fork_list.jsonl",
    "08_ephemeral_naming.jsonl",
    "09_user_input.jsonl",
    "10_cleanup_list.jsonl",
];

/// Which way a recorded frame went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    /// Codex → app.
    In,
    /// App → Codex.
    Out,
}

/// One recorded row.
#[derive(Debug, Clone, Deserialize)]
pub struct Row {
    pub dir: Dir,
    pub frame: Value,
}

/// The rows of recording `file`.
pub fn rows(file: &str) -> Vec<Row> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex/app_server")
        .join(file);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut rows = Vec::new();

    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        rows.push(serde_json::from_str(line).unwrap_or_else(|error| panic!("{file}: {error}")));
    }

    rows
}

/// The frames of recording `file` that went `dir`.
pub fn frames(file: &str, dir: Dir) -> Vec<Value> {
    let mut frames = rows(file);
    frames.retain(|row| row.dir == dir);

    frames.into_iter().map(|row| row.frame).collect()
}

/// The `n`-th (0-based) frame of `file` that went `dir` with `method`.
pub fn frame(file: &str, dir: Dir, method: &str, nth: usize) -> Value {
    frames(file, dir)
        .into_iter()
        .filter(|frame| frame["method"] == method)
        .nth(nth)
        .unwrap_or_else(|| panic!("{file}: no {method} #{nth}"))
}

/// The result Codex answered the app's request `id` of `file` with.
pub fn result(file: &str, id: i64) -> Value {
    frames(file, Dir::In)
        .into_iter()
        .find(|frame| frame["id"] == id && frame.get("method").is_none())
        .unwrap_or_else(|| panic!("{file}: no response {id}"))["result"]
        .clone()
}

/// `frame` with the recorded project directory moved to `project`.
pub fn moved(frame: &Value, project: &Path) -> Value {
    let quoted = serde_json::to_string(&project.to_string_lossy()).expect("a path as JSON");
    let text = frame.to_string().replace(PROJECT, &quoted[1..quoted.len() - 1]);

    serde_json::from_str(&text).expect("the moved frame is JSON")
}

/// A replayed recording: the mapper afterwards, the chat events and the engine's hints.
pub struct Replay {
    pub mapper: CodexMapper,
    pub events: Vec<AgentEvent>,
    pub hints: Vec<Hint>,
}

impl Replay {
    /// How many events of `kind` (the serialized `type`).
    pub fn count(&self, wanted: &str) -> usize {
        self.events.iter().map(kind).filter(|kind| kind == wanted).count()
    }
}

/// Replays `file` in the recorded project directory.
pub fn replay(file: &str) -> Replay {
    replay_in(file, Path::new(PROJECT))
}

/// Replays `file` with the recorded project directory moved to `project`.
pub fn replay_in(file: &str, project: &Path) -> Replay {
    let mut mapper = CodexMapper::new(project);
    let out = replay_rows(&mut mapper, rows(file), project);
    let hints = out
        .iter()
        .filter_map(|mapped| match mapped {
            Mapped::Hint(hint) => Some(hint.clone()),
            Mapped::Event(_) => None,
        })
        .collect();

    Replay {
        mapper,
        events: events(&out),
        hints,
    }
}

/// Feeds `rows` to `mapper` as the engine does (see the module docs).
pub fn replay_rows(mapper: &mut CodexMapper, rows: Vec<Row>, project: &Path) -> Vec<Mapped> {
    let mut out = Vec::new();

    for row in rows {
        match row.dir {
            Dir::Out => announce(mapper, &row.frame),
            Dir::In => {
                let frame = decode_value(moved(&row.frame, project)).unwrap_or_else(|error| panic!("{error:?}"));
                out.extend(mapper.on_frame(&frame));
            }
        }
    }

    out
}

fn announce(mapper: &mut CodexMapper, frame: &Value) {
    match frame["method"].as_str() {
        Some("turn/start") => mapper.sent(StartKind::Prompt),
        Some("thread/compact/start") => mapper.sent(StartKind::Compact),
        Some("thread/shellCommand") => mapper.sent(StartKind::Shell),
        Some("turn/interrupt") => mapper.abort_requested(),
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

/// The kinds of `events`, with runs of deltas and updates folded into one entry.
pub fn shape(events: &[AgentEvent]) -> Vec<String> {
    let mut shape: Vec<String> = Vec::new();

    for event in events {
        let kind = kind(event);
        let repeats = kind.ends_with("Delta") || kind == "toolUpdate";

        if repeats && shape.last() == Some(&kind) {
            continue;
        }

        shape.push(kind);
    }

    shape
}
