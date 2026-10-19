//! `fake_agent --dialect opencode --scenario <file.jsonl>`: a scripted `opencode serve` (1.19.3,
//! recordings in `tests/fixtures/opencode/server`) for the OpenCode chat tests.
//!
//! It binds `127.0.0.1:0` and prints `opencode server listening on http://127.0.0.1:<port>` like
//! the real server, then plays the scenario (JSONL, `//` comments and `include` as for the other
//! dialects). HTTP details, auth and the SSE stream: [`super::opencode_http`]. Steps:
//!
//! - `{"allow": "GET /global/health", "reply": {...}}` — from now on, answer every request to
//!   this route with `reply`, any number of times, without consuming steps (idempotent GETs whose
//!   timing the client decides). Allow rules answer before `expect`; a later rule for the same
//!   route wins. The rules at the top of the scenario are in place before the listening line.
//! - `{"expect": "POST /session/*/prompt_async", "match": {...}, "reply": {...}}` — the next
//!   request no `allow` rule answered must be this route (`*` stands for one path segment, the
//!   query is not part of the path) and carry every `match` field: keys starting with `/` are JSON
//!   pointers into the request body, `query.<name>` a decoded query parameter, `header.<name>` a
//!   header (lower case), any other key a top-level body field; the value `"$absent"` requires the
//!   field to be missing. A mismatch is answered `500`, reported on stderr and remembered: the step
//!   is consumed, and the fake exits 4 at the end.
//! - `"hold": true` (with `"as": "name"`, default the route) keeps the request unanswered until
//!   `{"respond": "name", "reply": {...}}`, for routes that block (`command`, `shell`, `summarize`).
//! - A reply is `{"status", "body", "headers": {name: value}}`: without `status` 200 with a body,
//!   204 without one. `"recording": "../opencode/server/agent-list.json"` (relative to the
//!   scenario) takes `status` and `body` from a recorded `{request, status, body}` file, the other
//!   keys override them. No `reply` answers 204.
//! - `{"sse": {...}}` — send one event to every `GET /global/event` subscriber as
//!   `data: <json>\n\n` in its own chunk. An object with a `payload` key is sent as is, any other
//!   object is the payload and gets `"directory": "$directory"`. Waits up to 10 s for a
//!   subscriber, no subscriber by then counts as a failure. `server.connected` is sent on
//!   subscribe, there is no heartbeat.
//! - `{"sseClose": true}` — end every `GET /global/event` stream, as a server would that stops
//!   sending events.
//! - `{"listen": true}` — print the listening line here instead of before the first step (to
//!   script a slow or failed startup).
//! - `{"sleep": ms}`, `{"stderr": "line"}`, `{"stderrRepeat": …}` as for the other dialects;
//!   `{"stdout": "line"}` prints a line on stdout (after the listening line).
//! - `{"exit": code}` (4 instead after a failure), `{"hang": true}` (serve the allow rules and
//!   ignore stdin EOF until killed).
//!
//! `"$directory"` in replies and events becomes the last `?directory=` (or
//! `x-opencode-directory`) a request carried, the working directory before any. `"$password"`
//! (also in `stdout` and `stderr` lines) becomes `OPENCODE_SERVER_PASSWORD`, to check that the
//! app never shows it. `"$request/messageID"` (any JSON pointer after `$request`) in `match`
//! values, replies and events becomes that field of the last expected request with a JSON object
//! body (for a `match`, the one before), for ids the app chose.
//!
//! `OPENCODE_CONFIG_CONTENT` (an approval mode) changes the agents of `GET /agent` like OpenCode
//! does (see [`super::opencode_config`]). The probe start of a chat with a mode runs the same
//! scenario: the leading allow rules answer it before a SIGTERM ends it.
//!
//! End of the scenario: after a failure the fake exits 4 at once. Otherwise it keeps serving the
//! allow rules until stdin closes, then exits 0. The real server ignores stdin EOF and needs
//! SIGTERM: the fake ignores it too until the last step ran (and for good with `hang`), so a stop
//! during the scenario takes the SIGTERM path like the real one. A request that arrives after the
//! last step is answered `500` and exits 4.

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::fail;
use crate::opencode_http::{Request, Shared, lock, matches, respond};

/// How long an `sse` step waits for a subscriber.
const SUBSCRIBER_WAIT: Duration = Duration::from_secs(10);

/// How often the idle loop looks at stdin.
const IDLE_POLL: Duration = Duration::from_millis(20);

/// Plays `steps` (loaded from `scenario`) as `opencode serve`; never returns.
pub fn serve(steps: &[Value], scenario: &Path) -> ! {
    let shared = Arc::new(Shared::from_env());
    let (sender, requests) = channel();
    let port = crate::opencode_http::listen(shared.clone(), sender);

    let mut server = Server {
        shared,
        requests,
        port,
        announced: false,
        held: HashMap::new(),
        failed: false,
        stdin_closed: watch_stdin(),
        base: scenario.parent().map(Path::to_path_buf).unwrap_or_default(),
        last_body: Value::Null,
    };

    // Leading allow rules are in place before the client can know the port.
    let leading = steps.iter().take_while(|step| step.get("allow").is_some()).count();
    let (allows, rest) = steps.split_at(leading);

    for step in allows {
        server.run(step);
    }

    if !rest.iter().any(|step| step.get("listen").is_some()) {
        server.announce();
    }

    for step in rest {
        server.run(step);
    }

    server.idle(true)
}

/// Set once stdin reached EOF.
fn watch_stdin() -> Arc<AtomicBool> {
    let closed = Arc::new(AtomicBool::new(false));
    let flag = closed.clone();

    std::thread::spawn(move || {
        std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink()).ok();
        flag.store(true, Ordering::SeqCst);
    });

    closed
}

struct Server {
    shared: Arc<Shared>,
    /// Requests no allow rule answered, in arrival order.
    requests: Receiver<Request>,
    port: u16,
    announced: bool,
    /// Requests kept by `"hold": true`, by name.
    held: HashMap<String, TcpStream>,
    /// Whether an expectation failed.
    failed: bool,
    stdin_closed: Arc<AtomicBool>,
    /// The scenario's directory (`recording` paths are relative to it).
    base: PathBuf,
    /// The body of the last expected request that had a JSON object body (`$request…`).
    last_body: Value,
}

impl Server {
    fn run(&mut self, step: &Value) {
        if crate::run_output_step(&self.shared.substitute_password(step)) {
            return;
        }

        let field = |name: &str| step.get(name);

        if let Some(route) = field("allow").and_then(Value::as_str) {
            let reply = self.resolve(field("reply"));
            lock(&self.shared.allows).push((route.to_owned(), reply));
        } else if let Some(route) = field("expect").and_then(Value::as_str) {
            self.expect(route, step);
        } else if let Some(name) = field("respond").and_then(Value::as_str) {
            let Some(stream) = self.held.remove(name) else {
                fail(2, &format!("nothing held as {name}"));
            };
            let reply = self.with_request(&self.resolve(field("reply")));
            respond(stream, &self.shared.reply(&reply));
        } else if let Some(event) = field("sse") {
            self.sse(event);
        } else if field("sseClose").is_some() {
            self.shared.close_subscribers();
        } else if field("listen").is_some() {
            self.announce();
        } else if let Some(line) = field("stdout").and_then(Value::as_str) {
            let line = self.shared.substitute_password(&json!(line));
            print(line.as_str().unwrap_or_default());
        } else if let Some(code) = field("exit").and_then(Value::as_i64) {
            self.exit(i32::try_from(code).unwrap_or(1));
        } else if field("hang").is_some() {
            self.idle(false);
        } else {
            fail(2, &format!("unknown step: {step}"));
        }
    }

    /// Prints the listening line once.
    fn announce(&mut self) {
        if self.announced {
            return;
        }

        self.announced = true;
        print(&format!("opencode server listening on http://127.0.0.1:{}", self.port));
    }

    fn expect(&mut self, route: &str, step: &Value) {
        let Ok(request) = self.requests.recv() else {
            fail(2, "the listener stopped");
        };

        let expected = step.get("match").map(|expected| self.with_request(expected));

        if let Some(problem) = mismatch(route, expected.as_ref(), &request) {
            self.reject(request, &format!("expected {route}: {problem}"));
            return;
        }

        if request.body.is_object() {
            self.last_body = request.body.clone();
        }

        if step.get("hold").and_then(Value::as_bool) == Some(true) {
            let name = step.get("as").and_then(Value::as_str).unwrap_or(route);
            self.held.insert(name.to_owned(), request.stream);
            return;
        }

        let reply = self.with_request(&self.resolve(step.get("reply")));
        let reply = self.shared.reply_to(&request, &reply);
        respond(request.stream, &reply);
    }

    /// `value` with `"$request<pointer>"` replaced from the last expected request's body.
    fn with_request(&self, value: &Value) -> Value {
        crate::substitute(value, &|name| {
            name.strip_prefix("request")
                .and_then(|pointer| self.last_body.pointer(pointer))
                .cloned()
        })
    }

    /// Answers `500`, reports `message` on stderr and remembers the failure.
    fn reject(&mut self, request: Request, message: &str) {
        eprintln!("fake_agent: {message}");
        self.failed = true;

        let body = json!({ "name": "FakeOpenCodeMismatch", "data": { "message": message } });
        let reply = self.shared.reply(&json!({ "status": 500, "body": body }));
        respond(request.stream, &reply);
    }

    fn sse(&mut self, event: &Value) {
        let event = if event.get("payload").is_some() {
            event.clone()
        } else {
            json!({ "directory": "$directory", "payload": event })
        };
        let event = self.shared.substitute(&self.with_request(&event));

        let deadline = Instant::now() + SUBSCRIBER_WAIT;
        while self.shared.subscribers() == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        if self.shared.broadcast(&event) == 0 {
            eprintln!("fake_agent: no /global/event subscriber for {event}");
            self.failed = true;
        }
    }

    /// The reply spec with a `recording` file folded in (`null` without a reply).
    fn resolve(&self, reply: Option<&Value>) -> Value {
        let Some(Value::Object(spec)) = reply else {
            return Value::Null;
        };

        let Some(file) = spec.get("recording").and_then(Value::as_str) else {
            return Value::Object(spec.clone());
        };

        let path = self.base.join(file);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|_| fail(2, &format!("cannot read {}", path.display())));
        let recorded: Value =
            serde_json::from_str(&text).unwrap_or_else(|_| fail(2, &format!("bad recording {}", path.display())));

        let mut merged = Map::new();
        for key in ["status", "body"] {
            merged.insert(key.to_owned(), recorded.get(key).cloned().unwrap_or(Value::Null));
        }

        let overrides = spec.iter().filter(|(key, _)| *key != "recording");
        merged.extend(overrides.map(|(key, value)| (key.clone(), value.clone())));

        Value::Object(merged)
    }

    /// Serves after the scenario (or from `hang` on): requests the allow rules do not answer
    /// fail, stdin EOF ends the fake when `stop_on_eof`.
    fn idle(&mut self, stop_on_eof: bool) -> ! {
        if stop_on_eof && self.failed {
            self.exit(4);
        }

        loop {
            match self.requests.recv_timeout(IDLE_POLL) {
                Ok(request) => {
                    let message = format!("unexpected {} after the scenario", request.line());
                    self.reject(request, &message);
                    self.exit(4);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => fail(2, "the listener stopped"),
            }

            if stop_on_eof && self.stdin_closed.load(Ordering::SeqCst) {
                self.exit(0);
            }
        }
    }

    fn exit(&self, code: i32) -> ! {
        std::process::exit(if self.failed { 4 } else { code });
    }
}

/// Prints `line` to stdout at once.
fn print(line: &str) {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}").ok();
    stdout.flush().ok();
}

/// Why `request` does not fit `route` and `expected`, if it does not.
fn mismatch(route: &str, expected: Option<&Value>, request: &Request) -> Option<String> {
    if !matches(route, &request.method, &request.path) {
        return Some(format!("got {}", request.line()));
    }

    let expected = expected.and_then(Value::as_object).into_iter().flatten();

    for (key, value) in expected {
        let actual = field_of(request, key);

        if value == "$absent" {
            if actual.is_some() {
                return Some(format!("{key} was {actual:?}, expected none"));
            }

            continue;
        }

        if actual.as_ref() != Some(value) {
            return Some(format!("{key} was {actual:?}, expected {value}"));
        }
    }

    None
}

/// A `match` key's value in `request` (see the module docs).
fn field_of(request: &Request, key: &str) -> Option<Value> {
    if key.starts_with('/') {
        return request.body.pointer(key).cloned();
    }

    if let Some(name) = key.strip_prefix("query.") {
        return request.query(name).map(|value| json!(value));
    }

    if let Some(name) = key.strip_prefix("header.") {
        return request.header(name).map(|value| json!(value));
    }

    request.body.get(key).cloned()
}
