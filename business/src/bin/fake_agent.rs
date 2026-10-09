//! Scripted OMP/PI RPC peer for the agent-session integration tests, and scripted MCP server for
//! the MCP client tests.
//!
//! `fake_agent --dialect omp|pi|mcp --scenario <file.jsonl>` plays a scenario: one JSON step per
//! line (blank lines and `//` comments are skipped). With `--dialect omp` the OMP `ready` frame is
//! sent first. With `--dialect mcp` messages are JSON-RPC 2.0: an `expect` names a `method`
//! instead of a `type`, and replies are `{"jsonrpc":"2.0","id":…, ...reply}` (so `reply` holds
//! `result` or `error`). Steps:
//!
//! - `{"emit": {...}}` — write a frame; string values `"$name"` become the id captured as `name`.
//! - `{"raw": "text"}` — write `text` and a newline verbatim (invalid JSON, CRLF, …).
//! - `{"big": {"type": "t", "id": "$name", "bytes": n}}` — write a frame of type `t` (and id)
//!   padded to `n` bytes (MCP: a response to `id` whose `result` is padded).
//! - `{"expect": "type", "as": "name", "match": {...}, "reply": {...}, "hold": true}` — wait for a
//!   frame of `type` (frames of other types are kept for later expects), capture its id (as
//!   `name`, default `type`), check that it contains every `match` field (keys starting with `/`
//!   are JSON pointers, e.g. `/params/name`), then answer
//!   `{"type":"response","id":…,"command":type, ...reply}` unless `hold` or no `reply`.
//! - `{"respond": "name", "command": "type", "reply": {...}}` — answer a held request later.
//! - `{"sleep": ms}`, `{"stderr": "line"}`, `{"stderrRepeat": {"line": "x", "count": n}}`.
//! - `{"exit": code}`, `{"hang": true}` (ignore stdin EOF, run until killed),
//!   `{"include": "other.jsonl"}` (relative to the scenario).
//!
//! After the last step the agent waits for stdin EOF and exits 0. An `expect` that sees EOF exits
//! 0 too; a failed `match` exits 4 with a message on stderr.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde_json::{Map, Value, json};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let option = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };

    let (Some(dialect), Some(scenario)) = (option("--dialect"), option("--scenario")) else {
        fail(2, "usage: fake_agent --dialect omp|pi|mcp --scenario <file.jsonl>");
    };

    let steps = load(Path::new(&scenario));
    let mut agent = Agent {
        stdin: spawn_stdin_reader(),
        unclaimed: Vec::new(),
        ids: HashMap::new(),
        mcp: dialect == "mcp",
    };

    if dialect == "omp" {
        agent.write(&json!({
            "type": "ready",
            "protocolVersion": 1,
            "supportedProtocolVersions": [1, 2],
            "maxFrameBytes": 1_048_576,
            "maxReassembledFrameBytes": 67_108_864
        }));
    }

    for step in &steps {
        agent.run(step);
    }

    agent.wait_eof();
    std::process::exit(0);
}

fn fail(code: i32, message: &str) -> ! {
    eprintln!("fake_agent: {message}");
    std::process::exit(code);
}

/// The scenario's steps with includes expanded.
fn load(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| fail(2, &format!("cannot read {}", path.display())));
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut steps = Vec::new();

    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        let step: Value = serde_json::from_str(line).unwrap_or_else(|_| fail(2, &format!("bad step: {line}")));

        match step.get("include").and_then(Value::as_str) {
            Some(other) => steps.extend(load(&PathBuf::from(&dir).join(other))),
            None => steps.push(step),
        }
    }

    steps
}

fn spawn_stdin_reader() -> Receiver<Value> {
    let (sender, receiver) = channel();

    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };

            if let Ok(frame) = serde_json::from_str::<Value>(&line)
                && sender.send(frame).is_err()
            {
                break;
            }
        }
    });

    receiver
}

struct Agent {
    stdin: Receiver<Value>,
    unclaimed: Vec<Value>,
    ids: HashMap<String, Value>,
    /// JSON-RPC framing: frames are matched by `method`, replies carry `jsonrpc` and no `type`.
    mcp: bool,
}

impl Agent {
    fn write(&self, frame: &Value) {
        let mut line = serde_json::to_vec(frame).unwrap_or_default();
        line.push(b'\n');
        self.write_bytes(&line);
    }

    fn write_bytes(&self, bytes: &[u8]) {
        let mut stdout = std::io::stdout().lock();

        if stdout.write_all(bytes).and_then(|()| stdout.flush()).is_err() {
            std::process::exit(0);
        }
    }

    fn run(&mut self, step: &Value) {
        let field = |name: &str| step.get(name);

        if let Some(frame) = field("emit") {
            let frame = self.substitute(frame);
            self.write(&frame);
        } else if let Some(text) = field("raw").and_then(Value::as_str) {
            self.write_bytes(format!("{text}\n").as_bytes());
        } else if let Some(big) = field("big") {
            self.write_big(big);
        } else if let Some(kind) = field("expect").and_then(Value::as_str) {
            self.expect(kind, step);
        } else if let Some(name) = field("respond").and_then(Value::as_str) {
            let id = self.ids.get(name).cloned().unwrap_or(Value::Null);
            let command = field("command").and_then(Value::as_str).unwrap_or(name);
            self.reply(id, command, field("reply"));
        } else if let Some(ms) = field("sleep").and_then(Value::as_u64) {
            std::thread::sleep(Duration::from_millis(ms));
        } else if let Some(line) = field("stderr").and_then(Value::as_str) {
            eprintln!("{line}");
        } else if let Some(repeat) = field("stderrRepeat") {
            let line = repeat.get("line").and_then(Value::as_str).unwrap_or("line");
            let count = repeat.get("count").and_then(Value::as_u64).unwrap_or(1);
            let mut stderr = std::io::stderr().lock();

            for index in 0..count {
                writeln!(stderr, "{line} {index}").ok();
            }
        } else if let Some(code) = field("exit").and_then(Value::as_i64) {
            std::process::exit(i32::try_from(code).unwrap_or(1));
        } else if field("hang").is_some() {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        } else {
            fail(2, &format!("unknown step: {step}"));
        }
    }

    fn write_big(&self, big: &Value) {
        let kind = big.get("type").and_then(Value::as_str).unwrap_or("message_update");
        let bytes = big.get("bytes").and_then(Value::as_u64).unwrap_or(0);
        let padding = "x".repeat(usize::try_from(bytes).unwrap_or(0));
        let id = big.get("id").map_or(Value::Null, |id| self.substitute(id));

        if self.mcp {
            self.write(&json!({ "jsonrpc": "2.0", "id": id, "result": { "pad": padding } }));
        } else {
            self.write(&json!({ "type": kind, "id": id, "pad": padding }));
        }
    }

    fn expect(&mut self, kind: &str, step: &Value) {
        let frame = self.next_of(kind);
        let name = step.get("as").and_then(Value::as_str).unwrap_or(kind).to_owned();
        let id = frame.get("id").cloned().unwrap_or(Value::Null);

        if let Some(Value::Object(expected)) = step.get("match") {
            for (key, value) in expected {
                // A key starting with `/` is a JSON pointer into the frame.
                let actual = if key.starts_with('/') {
                    frame.pointer(key)
                } else {
                    frame.get(key)
                };

                if actual != Some(value) {
                    fail(4, &format!("{kind}.{key} was {actual:?}, expected {value}"));
                }
            }
        }

        self.ids.insert(name, id.clone());

        let hold = step.get("hold").and_then(Value::as_bool).unwrap_or(false);
        if !hold && let Some(reply) = step.get("reply") {
            self.reply(id, kind, Some(reply));
        }
    }

    fn reply(&self, id: Value, command: &str, reply: Option<&Value>) {
        let mut frame = Map::new();

        if self.mcp {
            frame.insert("jsonrpc".to_owned(), json!("2.0"));
            frame.insert("id".to_owned(), id);
        } else {
            frame.insert("type".to_owned(), json!("response"));
            frame.insert("id".to_owned(), id);
            frame.insert("command".to_owned(), json!(command));
        }

        if let Some(Value::Object(fields)) = reply.map(|reply| self.substitute(reply)) {
            frame.extend(fields);
        }

        self.write(&Value::Object(frame));
    }

    /// The next frame of `kind`; exits quietly when stdin closes first.
    fn next_of(&mut self, kind: &str) -> Value {
        let key = if self.mcp { "method" } else { "type" };

        if let Some(index) = self.unclaimed.iter().position(|frame| frame[key] == kind) {
            return self.unclaimed.remove(index);
        }

        loop {
            let Ok(frame) = self.stdin.recv() else {
                std::process::exit(0);
            };

            if frame[key] == kind {
                return frame;
            }

            self.unclaimed.push(frame);
        }
    }

    fn wait_eof(&mut self) {
        while self.stdin.recv().is_ok() {}
    }

    /// Replaces `"$name"` strings with captured ids.
    fn substitute(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => match text.strip_prefix('$').and_then(|name| self.ids.get(name)) {
                Some(id) => id.clone(),
                None => value.clone(),
            },
            Value::Array(items) => Value::Array(items.iter().map(|item| self.substitute(item)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, item)| (key.clone(), self.substitute(item)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}
