//! The HTTP side of the fake `opencode serve` (see [`super::opencode`]).
//!
//! One thread per connection and one request per connection (`Connection: close`). Basic auth
//! works like the real server: with `OPENCODE_SERVER_PASSWORD` set, a request needs
//! `Authorization: Basic base64(user:password)` (user `OPENCODE_SERVER_USERNAME`, default
//! `opencode`) or `?auth_token=base64(user:password)`, else it gets `401` with
//! `www-authenticate`. `GET /global/event` becomes an SSE subscriber (chunked
//! `text/event-stream`, `server.connected` first), `allow` rules answer here, and every other
//! request goes to the scenario runner together with its connection.
//!
//! **Stored session rules:** with `FAKE_OPENCODE_STORE=<file>` (a JSON object of session id →
//! rule list, missing or empty at first) the fake keeps sessions' permission rules there like
//! OpenCode's database: `PATCH /session/{id} {permission}` *appends* to the stored list
//! (`Permission.merge` concatenates), and the replies to `GET` and `PATCH /session/{id}` carry the
//! stored list as `permission` (without one when the session has none).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};

/// Largest request head (request line and headers) the fake reads.
const MAX_HEAD_BYTES: usize = 64 * 1024;

/// Largest request body the fake reads (image prompts are large).
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// How long a connection may take to send its request.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// The SSE response head, as `opencode serve` 1.19.3 sends it (`global-event.sse`, without `Date`).
const SSE_HEAD: &str = "HTTP/1.1 200 OK\r\nCache-Control: no-cache, no-transform\r\nX-Content-Type-Options: \
                        nosniff\r\nContent-Type: text/event-stream\r\nVary: Origin\r\nx-accel-buffering: \
                        no\r\nTransfer-Encoding: chunked\r\n\r\n";

/// A parsed request and the connection to answer it on.
pub struct Request {
    pub method: String,
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

/// State shared by the connection threads and the scenario runner.
pub struct Shared {
    /// `user:password` a request must present, `None` without `OPENCODE_SERVER_PASSWORD`.
    credentials: Option<String>,
    /// `OPENCODE_SERVER_PASSWORD`, for `"$password"`.
    password: Option<String>,
    /// `allow` rules: route pattern and reply, newest last.
    pub allows: Mutex<Vec<(String, Value)>>,
    /// Open `/global/event` streams.
    subscribers: Mutex<Vec<TcpStream>>,
    /// The last `?directory=` (or `x-opencode-directory`) a request carried.
    directory: Mutex<Option<String>>,
    /// `FAKE_OPENCODE_STORE`: where sessions' permission rules are kept.
    store: Option<PathBuf>,
}

impl Shared {
    pub fn from_env() -> Self {
        let password = std::env::var("OPENCODE_SERVER_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty());
        let user = std::env::var("OPENCODE_SERVER_USERNAME").unwrap_or_else(|_| "opencode".to_owned());

        Self {
            credentials: password.as_ref().map(|password| format!("{user}:{password}")),
            password,
            allows: Mutex::default(),
            subscribers: Mutex::default(),
            directory: Mutex::default(),
            store: std::env::var_os("FAKE_OPENCODE_STORE").map(PathBuf::from),
        }
    }

    /// Replaces `"$directory"` with the directory of the last routed request (the working
    /// directory before any) and `"$password"` with the server password.
    pub fn substitute(&self, value: &Value) -> Value {
        let directory = lock(&self.directory).clone().unwrap_or_else(|| {
            std::env::current_dir()
                .map(|dir| dir.to_string_lossy().into_owned())
                .unwrap_or_default()
        });

        crate::substitute(value, &|name| match name {
            "directory" => Some(json!(directory)),
            _ => self.password_value(name),
        })
    }

    /// Replaces only `"$password"`, which never changes.
    pub fn substitute_password(&self, value: &Value) -> Value {
        crate::substitute(value, &|name| self.password_value(name))
    }

    fn password_value(&self, name: &str) -> Option<Value> {
        (name == "password").then(|| json!(self.password.clone().unwrap_or_default()))
    }

    /// The reply `{"status", "body", "headers"}` describes (`$directory` substituted). Without a
    /// `status`: 200 with a body, 204 without one. A `null` body means no body.
    pub fn reply(&self, spec: &Value) -> Reply {
        let spec = self.substitute(spec);
        let body = spec.get("body").filter(|body| !body.is_null()).cloned();
        let default_status = if body.is_some() { 200 } else { 204 };
        let status = spec
            .get("status")
            .and_then(Value::as_u64)
            .and_then(|status| u16::try_from(status).ok())
            .unwrap_or(default_status);

        let headers = spec
            .get("headers")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(name, value)| {
                let text = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
                (name.clone(), text)
            })
            .collect();

        Reply { status, body, headers }
    }

    /// [`Shared::reply`] to `request`, with the stored rules of the session it is about.
    pub fn reply_to(&self, request: &Request, spec: &Value) -> Reply {
        let mut reply = self.reply(spec);

        let Some(session) = session_route(request) else {
            return reply;
        };

        if let (Some(store), Some(Value::Object(body))) = (&self.store, reply.body.as_mut()) {
            body.remove("permission");

            if let Some(rules) = read_store(store).get(session) {
                body.insert("permission".to_owned(), rules.clone());
            }
        }

        reply
    }

    /// Appends the rules of a `PATCH /session/{id} {permission}` to the session's stored ones.
    fn store_rules(&self, request: &Request) {
        let (Some(store), Some(session)) = (&self.store, session_route(request)) else {
            return;
        };
        let Some(Value::Array(rules)) = request.body.get("permission").filter(|_| request.method == "PATCH") else {
            return;
        };

        let mut sessions = read_store(store);
        let stored = sessions.entry(session.to_owned()).or_insert_with(|| json!([]));

        if let Value::Array(stored) = stored {
            stored.extend(rules.iter().cloned());
        }

        let text = serde_json::to_string_pretty(&sessions).unwrap_or_default();
        std::fs::write(store, text).unwrap_or_else(|error| crate::fail(2, &format!("store: {error}")));
    }

    /// Ends every SSE stream (the last chunk, then the connection is shut).
    pub fn close_subscribers(&self) {
        for mut stream in lock(&self.subscribers).drain(..) {
            stream.write_all(b"0\r\n\r\n").ok();
            stream.shutdown(Shutdown::Both).ok();
        }
    }

    /// The number of open SSE streams.
    pub fn subscribers(&self) -> usize {
        lock(&self.subscribers).len()
    }

    /// Writes `event` to every open SSE stream, drops the closed ones and returns how many took it.
    pub fn broadcast(&self, event: &Value) -> usize {
        let mut subscribers = lock(&self.subscribers);
        subscribers.retain_mut(|stream| write_event(stream, event).is_ok());
        subscribers.len()
    }

    fn authorized(&self, request: &Request) -> bool {
        let Some(expected) = &self.credentials else {
            return true;
        };

        let basic = request
            .header("authorization")
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("basic"))
            .map(|(_, token)| token);

        [basic, request.query("auth_token")].into_iter().flatten().any(|token| {
            STANDARD
                .decode(token.trim())
                .is_ok_and(|decoded| decoded == expected.as_bytes())
        })
    }

    /// Sends the SSE head and `server.connected`, then keeps the stream for [`Shared::broadcast`].
    /// Holding the lock throughout keeps `server.connected` the first event.
    fn subscribe(&self, mut stream: TcpStream) {
        let connected =
            json!({ "payload": { "id": "evt_fake_connected", "type": "server.connected", "properties": {} } });
        let mut subscribers = lock(&self.subscribers);

        if stream.write_all(SSE_HEAD.as_bytes()).is_ok() && write_event(&mut stream, &connected).is_ok() {
            subscribers.push(stream);
        }
    }
}

/// The session id of a `/session/{id}` request.
fn session_route(request: &Request) -> Option<&str> {
    let session = request.path.strip_prefix("/session/")?;

    (!session.is_empty() && !session.contains('/')).then_some(session)
}

/// The stored rules by session; empty when the file is missing or empty.
fn read_store(path: &Path) -> Map<String, Value> {
    let text = std::fs::read_to_string(path).unwrap_or_default();

    serde_json::from_str(&text).unwrap_or_default()
}

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One SSE event as one chunk: `data: <json>\n\n`.
fn write_event(stream: &mut TcpStream, event: &Value) -> std::io::Result<()> {
    let data = format!("data: {event}\n\n");
    write!(stream, "{:x}\r\n{data}\r\n", data.len())?;
    stream.flush()
}

/// Binds `127.0.0.1:0` and serves connections on background threads; returns the port.
pub fn listen(shared: Arc<Shared>, runner: Sender<Request>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| crate::fail(2, &format!("bind: {error}")));
    let port = listener.local_addr().map(|address| address.port()).unwrap_or_default();

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

    if !shared.authorized(&request) {
        let challenge = ("www-authenticate".to_owned(), "Basic realm=\"Secure Area\"".to_owned());
        let reply = Reply {
            status: 401,
            body: None,
            headers: vec![challenge],
        };
        respond(request.stream, &reply);
        return;
    }

    let directory = request
        .query("directory")
        .or_else(|| request.header("x-opencode-directory"));
    if let Some(directory) = directory {
        *lock(&shared.directory) = Some(directory.to_owned());
    }

    if request.method == "GET" && request.path == "/global/event" {
        shared.subscribe(request.stream);
        return;
    }

    shared.store_rules(&request);

    let allowed = lock(&shared.allows)
        .iter()
        .rev()
        .find(|(pattern, _)| matches(pattern, &request.method, &request.path))
        .map(|(_, reply)| reply.clone());

    match allowed {
        Some(reply) => {
            let reply = shared.reply_to(&request, &reply);
            respond(request.stream, &reply);
        }
        None => {
            runner.send(request).ok();
        }
    }
}

/// Whether `METHOD /path` matches `pattern` (`METHOD /a/*/b`, `*` stands for one segment).
pub fn matches(pattern: &str, method: &str, path: &str) -> bool {
    let Some((wanted_method, wanted_path)) = pattern.split_once(' ') else {
        return false;
    };

    let wanted: Vec<&str> = wanted_path.split('/').collect();
    let actual: Vec<&str> = path.split('/').collect();

    wanted_method == method
        && wanted.len() == actual.len()
        && wanted
            .iter()
            .zip(&actual)
            .all(|(wanted, actual)| *wanted == "*" || wanted == actual)
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

    head.push_str(&format!(
        "Content-Length: {}\r\nVary: Origin\r\nConnection: close\r\n\r\n",
        body.len()
    ));

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
    let target = words.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));

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
