//! `fake_agent --dialect opencode2 --scenario <file.jsonl> [serve --hostname 127.0.0.1 --port 0]`:
//! a scripted OpenCode 2 `opencode serve` (2.1.2, recordings in `tests/fixtures/opencode2/recorded`)
//! for the OpenCode 2 chat tests.
//!
//! The arguments after the scenario are checked like the real CLI: `serve`, `--hostname
//! 127.0.0.1`, `--port 0`; any `--mdns…` prints the usage on stderr and exits 1 (2.1.2 rejects
//! it), other options exit 1 too. Without them (tests that drive the fake directly) nothing is
//! checked. The fake binds `127.0.0.1:0` and prints `server listening on http://127.0.0.1:<port>`.
//! HTTP, Basic auth and the request log: [`super::opencode2_http`]; the event stream and default
//! replies: [`super::opencode2_events`].
//!
//! # Scenario format
//!
//! JSONL, one step per line; blank lines and `//` comments are skipped and `{"include":
//! "other.jsonl"}` inlines a file (relative to the scenario). Routes are `METHOD /path` with `*`
//! for one path segment; the query is not part of the path.
//!
//! - `{"allow": "GET /api/agent", "reply": {...}, "match": {...}, "times": n}` — from now on,
//!   answer matching requests with `reply`, any number of times (or `n`), without consuming steps.
//!   A later `allow` for the same route wins; a spent one falls back to the earlier ones, so
//!   `{"allow": "GET /api/info", "reply": {"status": 503}, "times": 2}` makes the first two
//!   readiness checks fail. `GET /api/info` answers the server info by default.
//! - `{"on": "POST /api/session/*/prompt", "match": {...}, "reply": {...}, "emit": [steps],
//!   "times": n}` — a trigger: the next matching request (or the next `n`) gets `reply`, then the
//!   `emit` steps run (`sse`, `sseRaw`, `heartbeat`, `sseClose`, `sleep`, `stderr`) with
//!   `$request…` and `$path/<n>` from that request. Triggers are checked oldest first and before
//!   every `allow`. One that has not fired when stdin closes after the scenario fails the run.
//! - `{"expect": "POST /api/session", "match": {...}, "reply": {...}}` — the next request no rule
//!   answered must be this route and carry every `match` field: keys starting with `/` are JSON
//!   pointers into the body, `query.<name>` a decoded query parameter (`query.location[directory]`),
//!   `header.<name>` a header (lower case), any other key a top-level body field; `"$absent"`
//!   requires the field to be missing. A mismatch is answered `500`, reported on stderr and
//!   remembered: the step is consumed and the fake exits 4 at the end.
//! - `"hold": true` (with `"as": "name"`, default the route) keeps the request unanswered until
//!   `{"respond": "name", "reply": {...}}` (`shell` answers once the command ended).
//! - A reply is `{"status", "body", "headers"}` (without `status`: 200 with a body, 204 without),
//!   `{"data", "cursor", "location"}` for the `{data}` envelope, or `{"recording": "<file.jsonl>",
//!   "route": "GET /api/command", "nth": 0}` for the `status` and `response` of a recorded `http`
//!   row (the other keys override them); `"appendData": [...]` adds items to the end of the
//!   reply's `data` list. No reply (or one without these keys) answers the
//!   route's default: an inbox entry for `prompt` and `compact` (`"inboxID"` picks its id, else
//!   `msg_fakeinbox<n>`), `{"interrupted": true}` for `interrupt`, the configuration sources for
//!   `GET /api/config`, else `204`. `GET /api/agent` replies show `OPENCODE_CONFIG_CONTENT`'s agent
//!   permissions (see [`super::opencode2_events`]) unless the reply says `"ignoreConfig": true`.
//! - `{"sse": {"type": "session.text.delta", "data": {...}, "location": {...}}}` — send one event
//!   to every `GET /api/event` subscriber as `data: <json>`; `id` and `created` are filled in when
//!   missing. Waits up to 10 s for a subscriber; none by then is a failure.
//!   `{"sseRaw": "text"}` sends `data: text` verbatim, `{"heartbeat": true}` a `: heartbeat`
//!   comment, `{"sseClose": true}` ends every stream.
//! - `{"listen": true}` prints the listening line here instead of before the first step that is
//!   not a rule; with `{"listen": false}` (and no `listen: true`) it is never printed.
//! - `{"sleep": ms}`, `{"stderr": "line"}`, `{"stderrRepeat": …}` as for the other dialects;
//!   `{"stdout": "line"}` prints a line on stdout.
//! - `{"exit": code}` (4 instead after a failure), `{"crash": "message"}` (the message on stderr,
//!   then exit 1 at once, without ending the event streams), `{"hang": true}` (serve the rules
//!   until killed).
//!
//! Variables in rules, replies and events: `"$directory"` (and `"$directory/<path>"`) is the last
//! `location[directory]` a request carried, the working directory before any; `"$cwd"` and
//! `"$cwd/<path>"` the working directory; `"$password"` the server password; `"$port"` the port;
//! `"$inbox"` the id of the last inbox entry answered; `"$request/<pointer>"` a field of the
//! request body and `"$path/<n>"` the n-th path segment (`$path/2` is the session id of
//! `/api/session/<id>/…`) of the triggering request, or for sequential steps of the last expected
//! request.
//!
//! End of the scenario: after a failure the fake exits 4 at once. Otherwise it keeps serving the
//! rules until stdin closes, then exits 0 (4 when a trigger never fired). Stdin EOF before the last
//! step is ignored, as the real server ignores it. A request that reaches the runner after the
//! last step is answered `500` and the fake exits 4.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

use crate::fail;
use crate::opencode_http::lock;
use crate::opencode2_events::{Scope, reply, resolve_recording, run_stream_step};
use crate::opencode2_http::{Reply, Request, Rule, Shared, respond};

/// How often the idle loop looks at stdin.
const IDLE_POLL: Duration = Duration::from_millis(20);

/// What a rejected `serve` command line prints, like the real CLI's usage.
const USAGE: &str = "opencode serve\n\nstarts a headless opencode server\n\nOptions:\n  --port      port to listen on\n  \
                     --hostname  hostname to listen on";

/// Plays `steps` (loaded from `scenario`) as `opencode serve`; `args` is the whole command line.
/// Never returns.
pub fn serve(args: &[String], steps: &[Value], scenario: &Path) -> ! {
    check_args(args);

    let base = scenario.parent().map(Path::to_path_buf).unwrap_or_default();
    let shared = Arc::new(Shared::from_env(base));
    let (sender, requests) = channel();
    let port = crate::opencode2_http::listen(shared.clone(), sender);

    let mut server = Server {
        shared,
        requests,
        port,
        announced: false,
        held: HashMap::new(),
        stdin_closed: watch_stdin(),
        last: Scope::default(),
    };

    // Leading rules are in place before the client can know the port.
    let is_rule = |step: &&Value| step.get("allow").is_some() || step.get("on").is_some();
    let leading = steps.iter().take_while(is_rule).count();
    let (rules, rest) = steps.split_at(leading);

    for step in rules {
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

/// Exits 1 with the usage unless `args` (after the dialect and scenario) are empty or `serve
/// --hostname 127.0.0.1 --port 0`.
fn check_args(args: &[String]) {
    let mut rest = Vec::new();
    let mut index = 1;

    while index < args.len() {
        if args[index] == "--dialect" || args[index] == "--scenario" {
            index += 2;
            continue;
        }

        rest.push(args[index].as_str());
        index += 1;
    }

    let Some((command, options)) = rest.split_first() else {
        return;
    };

    if *command != "serve" {
        reject(&format!("unknown command: {command}"));
    }

    let mut options = options.iter();

    while let Some(option) = options.next() {
        let (name, inline) = match option.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (*option, None),
        };

        if name.starts_with("--mdns") {
            reject(&format!("unknown option: {name}"));
        }

        let value = inline.or_else(|| options.next().copied()).unwrap_or_default();

        match (name, value) {
            ("--hostname", "127.0.0.1") | ("--port", "0") => {}
            ("--hostname" | "--port", _) => reject(&format!("the fake only serves 127.0.0.1:0, not {name} {value}")),
            _ => reject(&format!("unknown option: {name}")),
        }
    }
}

fn reject(problem: &str) -> ! {
    eprintln!("{USAGE}\n\n{problem}");
    std::process::exit(1);
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
    /// Requests no rule answered, in arrival order.
    requests: Receiver<Request>,
    port: u16,
    announced: bool,
    /// Requests kept by `"hold": true`, by name.
    held: HashMap<String, Request>,
    stdin_closed: Arc<AtomicBool>,
    /// The last expected request (`$request…`, `$path/<n>` in sequential steps).
    last: Scope,
}

impl Server {
    fn run(&mut self, step: &Value) {
        if run_stream_step(&self.shared, step, Some(&self.last)) {
            return;
        }

        let field = |name: &str| step.get(name);

        if let Some(route) = field("allow").and_then(Value::as_str) {
            self.add_rule(route, step, false);
        } else if let Some(route) = field("on").and_then(Value::as_str) {
            self.add_rule(route, step, true);
        } else if let Some(route) = field("expect").and_then(Value::as_str) {
            self.expect(route, step);
        } else if let Some(name) = field("respond").and_then(Value::as_str) {
            let Some(request) = self.held.remove(name) else {
                fail(2, &format!("nothing held as {name}"));
            };
            self.answer(request, field("reply"));
        } else if let Some(listen) = field("listen").and_then(Value::as_bool) {
            if listen {
                self.announce();
            }
        } else if let Some(line) = field("stdout").and_then(Value::as_str) {
            let line = self.shared.substitute(&json!(line), None);
            print(line.as_str().unwrap_or_default());
        } else if let Some(code) = field("exit").and_then(Value::as_i64) {
            self.exit(i32::try_from(code).unwrap_or(1));
        } else if let Some(message) = field("crash").and_then(Value::as_str) {
            eprintln!("{message}");
            std::process::exit(1);
        } else if field("hang").is_some() {
            self.idle(false);
        } else {
            fail(2, &format!("unknown step: {step}"));
        }
    }

    fn add_rule(&self, route: &str, step: &Value, trigger: bool) {
        let times = step.get("times").and_then(Value::as_u64);
        let emit = step.get("emit").and_then(Value::as_array).cloned().unwrap_or_default();

        let rule = Rule {
            trigger,
            route: route.to_owned(),
            matcher: step.get("match").cloned(),
            reply: self.resolve(step.get("reply")),
            emit,
            remaining: if trigger { Some(times.unwrap_or(1)) } else { times },
        };

        lock(&self.shared.rules).push(rule);
    }

    /// Prints the listening line once.
    fn announce(&mut self) {
        if self.announced {
            return;
        }

        self.announced = true;
        print(&format!("server listening on http://127.0.0.1:{}", self.port));
    }

    fn expect(&mut self, route: &str, step: &Value) {
        let Ok(request) = self.requests.recv() else {
            fail(2, "the listener stopped");
        };

        let expected = step
            .get("match")
            .map(|expected| self.shared.substitute(expected, Some(&self.last)));

        if let Some(problem) = request.mismatch(route, expected.as_ref()) {
            self.reject(request, &format!("expected {route}: {problem}"));
            return;
        }

        self.last = request.scope();

        if step.get("hold").and_then(Value::as_bool) == Some(true) {
            let name = step.get("as").and_then(Value::as_str).unwrap_or(route);
            self.held.insert(name.to_owned(), request);
            return;
        }

        self.answer(request, step.get("reply"));
    }

    fn answer(&self, request: Request, spec: Option<&Value>) {
        let spec = self.resolve(spec);
        let reply = reply(&self.shared, &spec, &request);
        respond(request.stream, &reply);
    }

    /// The reply spec with a `recording` folded in (`null` without a reply).
    fn resolve(&self, spec: Option<&Value>) -> Value {
        spec.map_or(Value::Null, |spec| resolve_recording(&self.shared.base, spec))
    }

    /// Answers `500` and remembers the failure.
    fn reject(&self, request: Request, message: &str) {
        self.shared.fail(message);

        let body = json!({ "name": "FakeOpenCodeMismatch", "data": { "message": message } });
        let reply = Reply {
            status: 500,
            body: Some(body),
            headers: Vec::new(),
        };
        respond(request.stream, &reply);
    }

    /// Serves after the scenario (or from `hang` on): requests the rules do not answer fail,
    /// stdin EOF ends the fake when `stop_on_eof`.
    fn idle(&mut self, stop_on_eof: bool) -> ! {
        if stop_on_eof && self.failed() {
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
                for route in self.shared.unspent_triggers() {
                    self.shared.fail(&format!("the trigger on {route} never fired"));
                }

                self.exit(0);
            }
        }
    }

    fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::SeqCst)
    }

    fn exit(&self, code: i32) -> ! {
        std::process::exit(if self.failed() { 4 } else { code });
    }
}

/// Prints `line` to stdout at once.
fn print(line: &str) {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}").ok();
    stdout.flush().ok();
}
