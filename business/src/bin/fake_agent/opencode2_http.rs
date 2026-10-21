//! The HTTP side of the fake OpenCode 2 `opencode serve` (see [`super::opencode2`]).
//!
//! One thread per connection and one request per connection (`Connection: close`). With
//! `OPENCODE_SERVER_PASSWORD` set, every request needs `Authorization: Basic
//! base64(user:password)` (user `OPENCODE_SERVER_USERNAME`, default `opencode`), else it gets `401`
//! with `www-authenticate`, as OpenCode 2.1.2 answers. Every received request is appended to the
//! request log (`FAKE_OPENCODE_REQUEST_LOG`). `GET /api/event` becomes an SSE subscriber
//! ([`super::opencode2_events`]), `allow` and `on` rules answer here (newest first), the built-in
//! `GET /api/info` answers last, and every other request goes to the scenario runner together with
//! its connection.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::opencode_http::{lock, matches};
use crate::opencode2_events::{EventStream, Scope};

/// Largest request head (request line and headers) the fake reads.
const MAX_HEAD_BYTES: usize = 64 * 1024;

/// Largest request body the fake reads (image prompts are large).
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// How long a connection may take to send its request.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// The query parameter every located route carries (decoded name).
const LOCATION_QUERY: &str = "location[directory]";

/// A parsed request and the connection to answer it on.
pub struct Request {
    pub method: String,
    /// The request target as sent: path and raw query.
    pub target: String,
    /// The path without the query string.
    pub path: String,
    /// Percent-decoded query parameters.
    pub query: Vec<(String, String)>,
    /// Headers, names in lower case.
    pub headers: Vec<(String, String)>,
    /// The JSON body: `null` when empty, a string when it is not JSON.
    pub body: Value,
    pub stream: TcpStream,
}

impl Request {
    pub fn query(&self, name: &str) -> Option<&str> {
        find(&self.query, name)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        find(&self.headers, name)
    }

    /// `METHOD /path` for messages.
    pub fn line(&self) -> String {
        format!("{} {}", self.method, self.path)
    }

    /// What `$request…` and `$path/<n>` refer to.
    pub fn scope(&self) -> Scope {
        Scope {
            path: self.path.clone(),
            body: self.body.clone(),
        }
    }

    /// Whether this is `route` (`METHOD /a/*/b`) and carries every `match` field (see the
    /// scenario docs); the problem when it does not.
    pub fn mismatch(&self, route: &str, expected: Option<&Value>) -> Option<String> {
        if !matches(route, &self.method, &self.path) {
            return Some(format!("got {}", self.line()));
        }

        let expected = expected.and_then(Value::as_object).into_iter().flatten();

        for (key, value) in expected {
            let actual = self.field(key);

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

    /// A `match` key's value: `/pointer` into the body, `query.<name>`, `header.<name>` or a
    /// top-level body field.
    fn field(&self, key: &str) -> Option<Value> {
        if key.starts_with('/') {
            return self.body.pointer(key).cloned();
        }

        if let Some(name) = key.strip_prefix("query.") {
            return self.query(name).map(|value| json!(value));
        }

        if let Some(name) = key.strip_prefix("header.") {
            return self.header(name).map(|value| json!(value));
        }

        self.body.get(key).cloned()
    }
}

fn find<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// A response without a body or with a JSON body.
pub struct Reply {
    pub status: u16,
    pub body: Option<Value>,
    pub headers: Vec<(String, String)>,
}

/// An `allow` or `on` rule: answers matching requests without consuming steps.
pub struct Rule {
    /// An `on` trigger (checked oldest first, before every `allow` rule) or an `allow` rule
    /// (checked newest first).
    pub trigger: bool,
    /// `METHOD /path` with `*` segments.
    pub route: String,
    /// Fields the request must carry, as for `expect` (substituted per request).
    pub matcher: Option<Value>,
    /// The reply spec (`null`: the route's default reply).
    pub reply: Value,
    /// Stream steps run after the reply (`on` only).
    pub emit: Vec<Value>,
    /// How many more requests the rule answers; `None` for any number.
    pub remaining: Option<u64>,
}

/// State shared by the connection threads and the scenario runner.
pub struct Shared {
    /// `user:password` a request must present, `None` without `OPENCODE_SERVER_PASSWORD`.
    credentials: Option<String>,
    /// `OPENCODE_SERVER_PASSWORD`, for `"$password"`.
    pub password: Option<String>,
    /// `allow` and `on` rules, newest last.
    pub rules: Mutex<Vec<Rule>>,
    /// The open `/api/event` streams.
    pub events: EventStream,
    /// The last `location[directory]` a request carried.
    pub directory: Mutex<Option<String>>,
    /// `FAKE_OPENCODE_REQUEST_LOG`, opened for appending.
    log: Option<Mutex<File>>,
    /// `OPENCODE_CONFIG_CONTENT`, parsed.
    pub config: Option<Value>,
    /// The scenario's directory (`recording` paths are relative to it).
    pub base: PathBuf,
    /// The port the server listens on.
    pub port: AtomicU16,
    /// Numbers the inbox entries the fake makes up.
    pub inbox_count: AtomicU64,
    /// The id of the last inbox entry the fake answered (`"$inbox"`).
    pub inbox: Mutex<Option<String>>,
    /// Whether an expectation failed (the fake exits 4 at the end).
    pub failed: AtomicBool,
}

impl Shared {
    pub fn from_env(base: PathBuf) -> Self {
        let password = std::env::var("OPENCODE_SERVER_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty());
        let user = std::env::var("OPENCODE_SERVER_USERNAME").unwrap_or_else(|_| "opencode".to_owned());

        let log = std::env::var_os("FAKE_OPENCODE_REQUEST_LOG").map(|path| {
            let file = OpenOptions::new().create(true).append(true).open(&path);
            Mutex::new(file.unwrap_or_else(|error| crate::fail(2, &format!("request log: {error}"))))
        });

        Self {
            credentials: password.as_ref().map(|password| format!("{user}:{password}")),
            password,
            rules: Mutex::default(),
            events: EventStream::default(),
            directory: Mutex::default(),
            log,
            config: crate::opencode_config::from_env(),
            base,
            port: AtomicU16::new(0),
            inbox_count: AtomicU64::new(0),
            inbox: Mutex::default(),
            failed: AtomicBool::new(false),
        }
    }

    /// The `on` triggers that never fired as often as scripted, as `METHOD /path` lines.
    pub fn unspent_triggers(&self) -> Vec<String> {
        let rules = lock(&self.rules);
        let unspent = rules
            .iter()
            .filter(|rule| rule.trigger && rule.remaining.is_some_and(|left| left > 0));

        unspent.map(|rule| rule.route.clone()).collect()
    }

    /// Marks the run failed and reports `message` on stderr.
    pub fn fail(&self, message: &str) {
        eprintln!("fake_agent: {message}");
        self.failed.store(true, Ordering::SeqCst);
    }

    fn authorized(&self, request: &Request) -> bool {
        let Some(expected) = &self.credentials else {
            return true;
        };

        request
            .header("authorization")
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("basic"))
            .is_some_and(|(_, token)| {
                STANDARD
                    .decode(token.trim())
                    .is_ok_and(|decoded| decoded == expected.as_bytes())
            })
    }

    /// Appends `{method, target, body, authorized}` to the request log.
    fn record(&self, request: &Request, authorized: bool) {
        let Some(log) = &self.log else {
            return;
        };

        let entry = json!({
            "method": request.method,
            "target": request.target,
            "body": request.body,
            "authorized": authorized,
        });
        writeln!(lock(log), "{entry}").ok();
    }

    /// The rule that takes `request` (one use spent): the oldest live `on` trigger, else the
    /// newest live `allow` rule. Its reply and emits.
    fn take_rule(&self, request: &Request) -> Option<(Value, Vec<Value>)> {
        let scope = request.scope();
        let mut rules = lock(&self.rules);

        let takes = |rule: &Rule| {
            let matcher = rule
                .matcher
                .as_ref()
                .map(|matcher| self.substitute(matcher, Some(&scope)));
            rule.remaining != Some(0) && request.mismatch(&rule.route, matcher.as_ref()).is_none()
        };

        let trigger = rules.iter().position(|rule| rule.trigger && takes(rule));
        let allow = || rules.iter().rposition(|rule| !rule.trigger && takes(rule));
        let index = trigger.or_else(allow)?;
        let rule = &mut rules[index];

        if let Some(remaining) = rule.remaining.as_mut() {
            *remaining -= 1;
        }

        Some((rule.reply.clone(), rule.emit.clone()))
    }
}

/// Binds `127.0.0.1:0` and serves connections on background threads; returns the port.
pub fn listen(shared: Arc<Shared>, runner: Sender<Request>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| crate::fail(2, &format!("bind: {error}")));
    let port = listener.local_addr().map(|address| address.port()).unwrap_or_default();
    shared.port.store(port, Ordering::SeqCst);

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let shared = shared.clone();
            let runner = runner.clone();
            std::thread::spawn(move || handle(&shared, stream, &runner));
        }
    });

    port
}

fn handle(shared: &Shared, stream: TcpStream, runner: &Sender<Request>) {
    let Some(request) = read_request(stream) else {
        return;
    };

    let authorized = shared.authorized(&request);
    shared.record(&request, authorized);

    if !authorized {
        let challenge = ("www-authenticate".to_owned(), "Basic realm=\"Secure Area\"".to_owned());
        let reply = Reply {
            status: 401,
            body: None,
            headers: vec![challenge],
        };
        respond(request.stream, &reply);
        return;
    }

    if let Some(directory) = request.query(LOCATION_QUERY) {
        *lock(&shared.directory) = Some(directory.to_owned());
    }

    if request.method == "GET" && request.path == "/api/event" {
        shared.events.subscribe(request.stream);
        return;
    }

    if let Some((reply, emit)) = shared.take_rule(&request) {
        let scope = request.scope();
        let reply = crate::opencode2_events::reply(shared, &reply, &request);
        respond(request.stream, &reply);

        for step in &emit {
            crate::opencode2_events::run_stream_step(shared, step, Some(&scope));
        }

        return;
    }

    if request.method == "GET" && request.path == "/api/info" {
        let reply = crate::opencode2_events::reply(shared, &Value::Null, &request);
        respond(request.stream, &reply);
        return;
    }

    runner.send(request).ok();
}

/// Writes `reply` and closes the connection.
pub fn respond(mut stream: TcpStream, reply: &Reply) {
    let body = reply
        .body
        .as_ref()
        .map(|body| serde_json::to_vec(body).unwrap_or_default())
        .unwrap_or_default();

    let mut head = format!("HTTP/1.1 {} {}\r\n", reply.status, reason(reply.status));
    if reply.body.is_some() {
        head.push_str("Content-Type: application/json\r\n");
    }

    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }

    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()));

    let written = stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .and_then(|()| stream.flush());

    if written.is_ok() {
        stream.shutdown(Shutdown::Write).ok();
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Reads one request (`Content-Length` bodies only); `None` for anything malformed or too big.
fn read_request(stream: TcpStream) -> Option<Request> {
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    let mut reader = BufReader::new(stream.try_clone().ok()?).take(MAX_HEAD_BYTES as u64);
    let mut lines = Vec::new();

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }

        let line = line.trim_end();
        if line.is_empty() {
            break;
        }

        lines.push(line.to_owned());
    }

    let (first, header_lines) = lines.split_first()?;
    let mut words = first.split(' ');
    let method = words.next()?.to_owned();
    let target = words.next()?.to_owned();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));

    let headers: Vec<(String, String)> = header_lines
        .iter()
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();

    let length: usize = find(&headers, "content-length").map_or(Some(0), |value| value.parse().ok())?;
    if length > MAX_BODY_BYTES {
        return None;
    }

    let mut reader = reader.into_inner();
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).ok()?;

    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)))
    };

    let query = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(name), percent_decode(value))
        })
        .collect();

    Some(Request {
        method,
        path: path.to_owned(),
        target: target.clone(),
        query,
        headers,
        body,
        stream,
    })
}

/// Decodes `%XX` escapes (like `decodeURIComponent`, `+` stays).
fn percent_decode(text: &str) -> String {
    let mut decoded = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();

    while let Some((&byte, tail)) = rest.split_first() {
        let escaped = tail
            .get(..2)
            .filter(|pair| byte == b'%' && pair.iter().all(u8::is_ascii_hexdigit))
            .and_then(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok());

        match escaped {
            Some(value) => {
                decoded.push(value);
                rest = &tail[2..];
            }
            None => {
                decoded.push(byte);
                rest = tail;
            }
        }
    }

    String::from_utf8_lossy(&decoded).into_owned()
}
