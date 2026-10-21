//! The event stream, the scenario variables and the canned bodies of the fake OpenCode 2
//! `opencode serve` (see [`super::opencode2`]).
//!
//! `GET /api/event` answers a chunked `text/event-stream` (the head is assumed, not recorded) and
//! sends `data: {"id","type":"server.connected","data":{}}` and a `: heartbeat` comment first, as
//! 2.1.2 does. Every event is one chunk `data: <json>\n\n`; an event without `id` gets
//! `evt_fake<n>`, one without `created` the current time in milliseconds.
//!
//! Replies: a spec with `status`, `body` or `data` answers that (`data` is wrapped as
//! `{data, cursor?, location?}`). Any other spec answers the route's default: `prompt` and
//! `compact` an inbox entry made from the request (`{data: {id, sessionID, time, type, payload,
//! delivery}}`, `type` `user` or `compaction`, `delivery` the body's or `steer`), `interrupt`
//! `{"interrupted": true}`, `GET /api/info` the server info, `GET /api/config` the configuration
//! sources (`OPENCODE_CONFIG_CONTENT` as an inline document), anything else `204`.

use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::opencode_http::{lock, matches};
use crate::opencode2_http::{Reply, Request, Shared};

/// How long an event waits for a subscriber.
const SUBSCRIBER_WAIT: Duration = Duration::from_secs(10);

/// The version `GET /api/info` reports unless `FAKE_OPENCODE_VERSION` says otherwise.
const VERSION: &str = "2.1.2";

/// The directory the recordings were sanitized to.
const RECORDED_DIRECTORY: &str = "/work/proj";

/// The SSE response head.
const SSE_HEAD: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: \
                        keep-alive\r\nTransfer-Encoding: chunked\r\n\r\n";

/// The scenario variables (see the scenario docs in [`super::opencode2`]).
impl Shared {
    /// Replaces the scenario variables (see the scenario docs) in `value`; `$request…` and
    /// `$path/<n>` come from `scope`.
    pub fn substitute(&self, value: &Value, scope: Option<&Scope>) -> Value {
        let cwd = std::env::current_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        let directory = lock(&self.directory).clone().unwrap_or_else(|| cwd.clone());
        let inbox = lock(&self.inbox).clone();

        crate::substitute(value, &|name| match name {
            "directory" => Some(json!(directory)),
            "cwd" => Some(json!(cwd)),
            "password" => Some(json!(self.password.clone().unwrap_or_default())),
            "port" => Some(json!(self.port.load(Ordering::SeqCst))),
            "inbox" => inbox.clone().map(Value::String),
            _ => self.substitute_prefixed(name, &directory, &cwd, scope),
        })
    }

    fn substitute_prefixed(&self, name: &str, directory: &str, cwd: &str, scope: Option<&Scope>) -> Option<Value> {
        if let Some(path) = name.strip_prefix("directory/") {
            return Some(json!(format!("{directory}/{path}")));
        }

        if let Some(path) = name.strip_prefix("cwd/") {
            return Some(json!(format!("{cwd}/{path}")));
        }

        let scope = scope?;

        if let Some(pointer) = name.strip_prefix("request") {
            return scope.body.pointer(pointer).cloned();
        }

        let index: usize = name.strip_prefix("path/")?.parse().ok()?;
        scope.path.split('/').nth(index + 1).map(|segment| json!(segment))
    }

    /// The id of a new inbox entry (`chosen`, else `msg_fakeinbox<n>`), remembered as `"$inbox"`.
    pub fn next_inbox(&self, chosen: Option<&str>) -> String {
        let number = self.inbox_count.fetch_add(1, Ordering::SeqCst) + 1;
        let id = chosen.map_or_else(|| format!("msg_fakeinbox{number}"), str::to_owned);
        *lock(&self.inbox) = Some(id.clone());
        id
    }
}

/// What `$request…` and `$path/<n>` refer to: the triggering (or last expected) request.
#[derive(Clone, Default)]
pub struct Scope {
    pub path: String,
    pub body: Value,
}

/// The open `/api/event` streams.
#[derive(Default)]
pub struct EventStream {
    subscribers: Mutex<Vec<TcpStream>>,
    count: AtomicU64,
}

impl EventStream {
    /// Sends the SSE head, `server.connected` and a heartbeat, then keeps the stream. Holding the
    /// lock throughout keeps `server.connected` the first event.
    pub fn subscribe(&self, mut stream: TcpStream) {
        let connected = json!({ "id": "evt_fakeconnected", "type": "server.connected", "data": {} });
        let mut subscribers = lock(&self.subscribers);

        let sent = stream
            .write_all(SSE_HEAD.as_bytes())
            .and_then(|()| write_chunk(&mut stream, &format!("data: {connected}\n\n")))
            .and_then(|()| write_chunk(&mut stream, ": heartbeat\n\n"));

        if sent.is_ok() {
            subscribers.push(stream);
        }
    }

    /// The number of open streams.
    pub fn subscribers(&self) -> usize {
        lock(&self.subscribers).len()
    }

    /// Sends `event` (with `id` and `created` filled in) to every stream; how many took it.
    pub fn send(&self, event: &Value) -> usize {
        let mut event = event.clone();

        if let Value::Object(fields) = &mut event {
            if !fields.contains_key("id") {
                let number = self.count.fetch_add(1, Ordering::SeqCst) + 1;
                fields.insert("id".to_owned(), json!(format!("evt_fake{number}")));
            }

            if !fields.contains_key("created") {
                fields.insert("created".to_owned(), json!(now()));
            }
        }

        self.broadcast(&format!("data: {event}\n\n"))
    }

    /// Writes `text` as one chunk to every stream and drops the closed ones.
    pub fn broadcast(&self, text: &str) -> usize {
        let mut subscribers = lock(&self.subscribers);
        subscribers.retain_mut(|stream| write_chunk(stream, text).is_ok());
        subscribers.len()
    }

    /// Ends every stream (the last chunk, then the connection is shut).
    pub fn close(&self) {
        for mut stream in lock(&self.subscribers).drain(..) {
            stream.write_all(b"0\r\n\r\n").ok();
            stream.shutdown(Shutdown::Both).ok();
        }
    }

    /// Waits up to [`SUBSCRIBER_WAIT`] for a subscriber.
    fn wait_for_subscriber(&self) {
        let deadline = Instant::now() + SUBSCRIBER_WAIT;

        while self.subscribers() == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn write_chunk(stream: &mut TcpStream, text: &str) -> std::io::Result<()> {
    write!(stream, "{:x}\r\n{text}\r\n", text.len())?;
    stream.flush()
}

/// Milliseconds since the epoch.
fn now() -> u64 {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

/// Runs a stream step (`sse`, `sseRaw`, `heartbeat`, `sseClose`) or an output step (`sleep`,
/// `stderr`, ...) with `scope` for `$request…`; false for any other step.
pub fn run_stream_step(shared: &Shared, step: &Value, scope: Option<&Scope>) -> bool {
    let field = |name: &str| step.get(name);

    if let Some(event) = field("sse") {
        let event = shared.substitute(event, scope);
        shared.events.wait_for_subscriber();

        if shared.events.send(&event) == 0 {
            shared.fail(&format!("no /api/event subscriber for {event}"));
        }
    } else if let Some(text) = field("sseRaw").and_then(Value::as_str) {
        shared.events.wait_for_subscriber();
        shared.events.broadcast(&format!("data: {text}\n\n"));
    } else if field("heartbeat").is_some() {
        shared.events.broadcast(": heartbeat\n\n");
    } else if field("sseClose").is_some() {
        shared.events.close();
    } else {
        return crate::run_output_step(&shared.substitute(step, scope));
    }

    true
}

/// The reply `spec` describes for `request` (see the module docs).
pub fn reply(shared: &Shared, spec: &Value, request: &Request) -> Reply {
    let spec = shared.substitute(spec, Some(&request.scope()));
    let explicit = ["status", "body", "data"].iter().any(|key| spec.get(key).is_some());

    let mut reply = if explicit {
        explicit_reply(&spec)
    } else {
        canned(shared, &spec, request)
    };

    let headers = spec.get("headers").and_then(Value::as_object).into_iter().flatten();
    reply.headers = headers
        .map(|(name, value)| {
            let text = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
            (name.clone(), text)
        })
        .collect();

    reply
}

/// `{status, body}` or `{data, cursor?, location?}`; without a `status` 200 with a body, 204
/// without one.
fn explicit_reply(spec: &Value) -> Reply {
    let body = match spec.get("data") {
        Some(data) => {
            let mut envelope = Map::new();
            envelope.insert("data".to_owned(), data.clone());

            for key in ["cursor", "location"] {
                if let Some(value) = spec.get(key) {
                    envelope.insert(key.to_owned(), value.clone());
                }
            }

            Some(Value::Object(envelope))
        }
        None => spec.get("body").filter(|body| !body.is_null()).cloned(),
    };

    let default_status = if body.is_some() { 200 } else { 204 };
    let status = spec
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|status| u16::try_from(status).ok())
        .unwrap_or(default_status);

    Reply {
        status,
        body,
        headers: Vec::new(),
    }
}

/// The route's default reply.
fn canned(shared: &Shared, spec: &Value, request: &Request) -> Reply {
    let route = |pattern: &str| matches(pattern, &request.method, &request.path);
    let chosen = spec.get("inboxID").and_then(Value::as_str);

    let body = if route("POST /api/session/*/prompt") {
        Some(inbox(shared, "user", request, chosen))
    } else if route("POST /api/session/*/compact") {
        Some(inbox(shared, "compaction", request, chosen))
    } else if route("POST /api/session/*/interrupt") {
        Some(json!({ "interrupted": true }))
    } else if route("GET /api/info") {
        Some(info(shared))
    } else if route("GET /api/config") {
        Some(config_sources(shared))
    } else {
        None
    };

    Reply {
        status: if body.is_some() { 200 } else { 204 },
        body,
        headers: Vec::new(),
    }
}

/// The inbox entry a `prompt` or `compact` request makes.
fn inbox(shared: &Shared, kind: &str, request: &Request, chosen: Option<&str>) -> Value {
    let id = shared.next_inbox(chosen);
    let session = request.path.split('/').nth(3).unwrap_or_default();
    let mut payload = Map::new();

    if kind == "user" {
        let text = request.body.get("text").cloned().unwrap_or(json!(""));
        payload.insert("text".to_owned(), text);

        let files = request.body.get("files").and_then(Value::as_array);
        if let Some(files) = files.filter(|files| !files.is_empty()) {
            payload.insert("files".to_owned(), json!(files));
        }
    }

    let delivery = request.body.get("delivery").cloned().unwrap_or(json!("steer"));

    json!({
        "data": {
            "id": id,
            "sessionID": session,
            "time": { "created": now() },
            "type": kind,
            "payload": payload,
            "delivery": delivery,
        }
    })
}

fn info(shared: &Shared) -> Value {
    let version = std::env::var("FAKE_OPENCODE_VERSION").unwrap_or_else(|_| VERSION.to_owned());
    let port = shared.port.load(Ordering::SeqCst);

    json!({
        "version": version,
        "pid": std::process::id(),
        "urls": [format!("http://127.0.0.1:{port}")],
        "paths": { "tmp": std::env::temp_dir() },
        "capabilities": { "persistentPty": true },
    })
}

/// The global configuration directory, then `OPENCODE_CONFIG_CONTENT` as an inline document.
fn config_sources(shared: &Shared) -> Value {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut sources = vec![json!({ "type": "directory", "path": format!("{home}/.config/opencode") })];

    if let Some(config) = &shared.config {
        sources.push(json!({ "type": "document", "info": config }));
    }

    Value::Array(sources)
}

/// A reply spec with its `recording` folded in: `{"recording": "<file.jsonl>", "route": "GET
/// /api/command", "nth": 0}` takes `status` and the `response` (as `body`) of the `nth` (default
/// first) recorded `http` row of that route; the other keys override them. Paths under the
/// recorded `/work/proj` become `$directory`. Specs without `recording` stay as they are.
pub fn resolve_recording(base: &Path, spec: &Value) -> Value {
    let Value::Object(fields) = spec else {
        return spec.clone();
    };

    let Some(file) = fields.get("recording").and_then(Value::as_str) else {
        return spec.clone();
    };

    let route = fields.get("route").and_then(Value::as_str).unwrap_or_default();
    let nth = fields.get("nth").and_then(Value::as_u64).unwrap_or(0);
    let path = base.join(file);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|_| crate::fail(2, &format!("cannot read {}", path.display())));

    let row = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row["kind"] == "http")
        .filter(|row| {
            let method = row["method"].as_str().unwrap_or_default();
            let target = row["path"].as_str().unwrap_or_default();
            matches(route, method, target.split('?').next().unwrap_or_default())
        })
        .nth(usize::try_from(nth).unwrap_or(usize::MAX))
        .unwrap_or_else(|| crate::fail(2, &format!("no {route} in {}", path.display())));

    let mut merged = Map::new();
    merged.insert("status".to_owned(), row["status"].clone());
    merged.insert("body".to_owned(), relocate(&row["response"]));

    let overrides = fields
        .iter()
        .filter(|(key, _)| !["recording", "route", "nth"].contains(&key.as_str()));
    merged.extend(overrides.map(|(key, value)| (key.clone(), value.clone())));

    Value::Object(merged)
}

/// `value` with the recorded project directory replaced by `$directory`.
fn relocate(value: &Value) -> Value {
    match value {
        Value::String(text) if text == RECORDED_DIRECTORY => json!("$directory"),
        Value::String(text) => match text.strip_prefix(&format!("{RECORDED_DIRECTORY}/")) {
            Some(rest) => json!(format!("$directory/{rest}")),
            None => value.clone(),
        },
        Value::Array(items) => Value::Array(items.iter().map(relocate).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(key, item)| (key.clone(), relocate(item))).collect()),
        other => other.clone(),
    }
}
