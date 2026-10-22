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
//!   becomes the working directory (as for `replay`).
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

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use serde_json::{Map, Value, json};

use crate::{check_match, fail, run_end_step, run_output_step, spawn_stdin_reader, substitute};

/// The project directory of the recordings.
const RECORDED_PROJECT: &str = "/home/user/project";

/// Plays `steps` (see the module docs) and exits.
pub fn serve(steps: &[Value], scenario: &Path) -> ! {
    let mut fake = Fake {
        stdin: spawn_stdin_reader(),
        unclaimed: Vec::new(),
        captured: HashMap::new(),
        dir: scenario.parent().map(Path::to_path_buf).unwrap_or_default(),
        cwd: std::env::current_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };

    for step in steps {
        fake.run(step);
    }

    while fake.stdin.recv().is_ok() {}
    std::process::exit(0);
}

struct Fake {
    stdin: Receiver<Value>,
    unclaimed: Vec<Value>,
    captured: HashMap<String, Value>,
    /// The scenario's folder (replayed recordings are relative to it).
    dir: PathBuf,
    cwd: String,
}

impl Fake {
    fn run(&mut self, step: &Value) {
        if run_output_step(step) {
            return;
        }

        let field = |name: &str| step.get(name);

        if let Some(kind) = field("expect").and_then(Value::as_str) {
            self.expect(kind, step);
        } else if let Some(name) = field("respond").and_then(Value::as_str) {
            let id = self.captured.get(name).cloned().unwrap_or(Value::Null);
            self.answer(&id, step);
        } else if let Some(frame) = field("emit") {
            let frame = self.recorded(&self.substitute(frame), &[]);
            write(&frame);
        } else if let Some(file) = field("replay").and_then(Value::as_str) {
            self.replay(file, step);
        } else {
            run_end_step(step);
        }
    }

    fn expect(&mut self, kind: &str, step: &Value) {
        let subtype = step.get("subtype").and_then(Value::as_str);
        let frame = self.next_of(kind, subtype);
        let label = subtype.unwrap_or(kind);
        let name = step.get("as").and_then(Value::as_str).unwrap_or(label).to_owned();

        check_match(label, &frame, step);

        if kind == "user" {
            self.captured.insert(name, frame["uuid"].clone());
            return;
        }

        if kind == "control_response" {
            return;
        }

        let id = frame["request_id"].clone();
        self.captured.insert(name, id.clone());

        if !step.get("hold").and_then(Value::as_bool).unwrap_or(false) {
            self.answer(&id, step);
        }
    }

    /// Answers the request `id` with the step's `reply` (or `error`).
    fn answer(&self, id: &Value, step: &Value) {
        let mut response = Map::new();
        response.insert("request_id".to_owned(), id.clone());

        if let Some(error) = step.get("error") {
            response.insert("subtype".to_owned(), json!("error"));
            response.insert("error".to_owned(), error.clone());
        } else {
            response.insert("subtype".to_owned(), json!("success"));

            if let Some(reply) = step.get("reply") {
                response.insert("response".to_owned(), self.substitute(reply));
            }
        }

        write(&json!({ "type": "control_response", "response": Value::Object(response) }));
    }

    /// The next inbound frame of `kind` (and request `subtype`); exits quietly when stdin closes
    /// first.
    fn next_of(&mut self, kind: &str, subtype: Option<&str>) -> Value {
        let wanted = |frame: &Value| {
            frame["type"] == kind
                && subtype.is_none_or(|subtype| frame.pointer("/request/subtype") == Some(&json!(subtype)))
        };

        if let Some(index) = self.unclaimed.iter().position(wanted) {
            return self.unclaimed.remove(index);
        }

        loop {
            let Ok(frame) = self.stdin.recv() else {
                std::process::exit(0);
            };

            if wanted(&frame) {
                return frame;
            }

            self.unclaimed.push(frame);
        }
    }

    fn replay(&self, file: &str, step: &Value) {
        let path = self.dir.join(file);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|_| fail(2, &format!("cannot read {}", path.display())));
        let line = |name: &str| step.get(name).and_then(Value::as_u64).map(|line| line as usize);
        let (from, to) = (line("from").unwrap_or(1), line("to").unwrap_or(usize::MAX));
        let except: Vec<&str> = step
            .get("except")
            .and_then(Value::as_array)
            .map(|kinds| kinds.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let replace: Vec<(Value, Value)> = step
            .get("replace")
            .and_then(Value::as_object)
            .map(|pairs| {
                pairs
                    .iter()
                    .map(|(recorded, value)| (json!(recorded), self.substitute(value)))
                    .collect()
            })
            .unwrap_or_default();

        for (index, row) in text.lines().enumerate() {
            let number = index + 1;
            if number < from || number > to || row.trim().is_empty() {
                continue;
            }

            let row: Value =
                serde_json::from_str(row).unwrap_or_else(|_| fail(2, &format!("bad row {number} of {file}")));
            let frame = &row["frame"];
            let left_out = frame["type"].as_str().is_some_and(|kind| except.contains(&kind));

            if row["dir"] != "out" || left_out {
                continue;
            }

            write(&self.recorded(frame, &replace));
        }
    }

    /// A recorded frame with `replace` and the working directory applied.
    fn recorded(&self, value: &Value, replace: &[(Value, Value)]) -> Value {
        if let Some((_, value)) = replace.iter().find(|(recorded, _)| recorded == value) {
            return value.clone();
        }

        match value {
            Value::String(text) => match text.strip_prefix(RECORDED_PROJECT) {
                Some(rest) => Value::String(format!("{}{rest}", self.cwd)),
                None => value.clone(),
            },
            Value::Array(items) => Value::Array(items.iter().map(|item| self.recorded(item, replace)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, item)| (key.clone(), self.recorded(item, replace)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// Replaces `"$name"` strings with captured values.
    fn substitute(&self, value: &Value) -> Value {
        substitute(value, &|name| self.captured.get(name).cloned())
    }
}

/// Writes one frame and a newline; exits once stdout is gone.
fn write(frame: &Value) {
    let mut line = serde_json::to_vec(frame).unwrap_or_default();
    line.push(b'\n');

    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(&line).and_then(|()| stdout.flush()).is_err() {
        std::process::exit(0);
    }
}
