//! `fake_agent --dialect claude --scenario <file.jsonl>`: a scripted `claude` in stream-json mode
//! (Claude Code 2.1.304, recordings in `tests/fixtures/claude/stream`) for the Claude Code chat
//! tests. It reads NDJSON frames on stdin and writes frames on stdout, like the CLI.
//!
//! # Scenario format
//!
//! JSONL, one step per line; blank lines and `//` comments are skipped and `{"include":
//! "other.jsonl"}` inlines a file (relative to the scenario).
//!
//! - `{"expect": "control_request", "subtype": "initialize", "as": "name", "match": {...},
//!   "reply": {...}}` — wait for the next inbound `control_request` with that `request.subtype`
//!   (frames of other kinds are kept for later expects), capture its `request_id` as `name`
//!   (default: the subtype), check that it carries every `match` field (keys starting with `/` are
//!   JSON pointers, `"/request/model"`), then answer `{"type": "control_response", "response":
//!   {"subtype": "success", "request_id": …, "response": reply}}`. `"error": "text"` answers
//!   `subtype: error` instead, no `reply` a bare success, and `"hold": true` nothing until
//!   `{"respond": "name", "reply": {...}}` (or `"error"`).
//! - `{"expect": "user", "as": "name", "match": {...}}` — wait for the next inbound user message,
//!   capture its `uuid` as `name` (default `user`) and check `match`. It gets no answer: the steps
//!   that follow are the CLI's output.
//! - `{"expect": "control_response", "match": {...}}` — wait for the app's next answer to one of
//!   the CLI's requests and check `match` (`"/response/request_id"`, …). It gets no answer.
//! - `{"emit": {...}}` — write a frame; string values `"$name"` become the value captured as
//!   `name`, and the recorded project directory `/home/user/project` at the start of a string
//!   becomes the working directory (as in `replay`). With `"times": n` it is written n times,
//!   `"$index"` being `ask-0`, `ask-1`, … in turn.
//! - `{"replay": "../claude/stream/basic.jsonl", "from": 7, "to": 20, "except": ["control_request"],
//!   "replace": {"<recorded value>": "$name"}}` — write the CLI's frames (rows with `dir: out`) of
//!   that recording (relative to the scenario) from line `from` to line `to` (1-based, both
//!   included; the whole file without them), leaving out frames whose `type` is in `except`. A
//!   string equal to a `replace` key becomes its value (`"$name"` the captured one), and the
//!   recorded project directory `/home/user/project` at the start of a string becomes the working
//!   directory.
//! - `{"sleep": ms}`, `{"stderr": "line"}`, `{"stderrRepeat": …}` as for the other dialects;
//!   `{"exit": code}`, `{"hang": true}` (ignore stdin EOF, run until killed).
//!
//! After the last step the fake waits for stdin EOF and exits 0, as the CLI does. An `expect`
//! that sees EOF exits 0 too; a failed `match` exits 4 with a message on stderr.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::check_match;
use crate::recorded::{Dialect, Player, write};

/// The project directory of the recordings.
const RECORDED_PROJECT: &str = "/home/user/project";

/// Plays `steps` (see the module docs) and exits.
pub fn serve(steps: &[Value], scenario: &Path) -> ! {
    Player::play::<Claude>(steps, scenario, RECORDED_PROJECT, "out")
}

struct Claude;

impl Dialect for Claude {
    fn kind(frame: &Value) -> Option<String> {
        frame["type"].as_str().map(str::to_owned)
    }

    fn index(index: u64) -> Value {
        json!(format!("ask-{index}"))
    }

    fn expect(player: &mut Player, kind: &str, step: &Value) {
        let subtype = step.get("subtype").and_then(Value::as_str);
        let frame = player.inbox.next(|frame: &Value| {
            frame["type"] == kind
                && subtype.is_none_or(|subtype| frame.pointer("/request/subtype") == Some(&json!(subtype)))
        });
        let label = subtype.unwrap_or(kind);
        let name = step.get("as").and_then(Value::as_str).unwrap_or(label).to_owned();

        check_match(label, &frame, step);

        if kind == "user" {
            player.captured.insert(name, frame["uuid"].clone());
            return;
        }

        if kind == "control_response" {
            return;
        }

        let id = frame["request_id"].clone();
        player.captured.insert(name, id.clone());

        if !step.get("hold").and_then(Value::as_bool).unwrap_or(false) {
            Self::answer(player, &id, step);
        }
    }

    fn answer(player: &Player, id: &Value, step: &Value) {
        let mut response = Map::new();
        response.insert("request_id".to_owned(), id.clone());

        if let Some(error) = step.get("error") {
            response.insert("subtype".to_owned(), json!("error"));
            response.insert("error".to_owned(), error.clone());
        } else {
            response.insert("subtype".to_owned(), json!("success"));

            if let Some(reply) = step.get("reply") {
                response.insert("response".to_owned(), player.substitute(reply));
            }
        }

        write(&json!({ "type": "control_response", "response": Value::Object(response) }));
    }
}
