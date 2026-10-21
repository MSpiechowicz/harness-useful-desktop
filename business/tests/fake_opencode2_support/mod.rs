//! A client for the scripted OpenCode 2 `opencode serve` (`fake_agent --dialect opencode2`) over
//! raw HTTP: [`Fake`] starts it and sends requests with Basic auth, [`Events`] reads the chunked
//! `/api/event` stream frame by frame, and [`play_to_marker`] plays a recorded turn.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::agent_support::{WAIT, scenario};
use crate::opencode2_engine_support::{LoggedRequest, SESSION, read_log};

pub const PASSWORD: &str = "fake-server-password";
pub const CREDENTIALS: &str = "opencode:fake-server-password";
pub const LOCATION: &str = "location%5Bdirectory%5D=%2Fwork%2Fmy%20project";

/// A running fake server; killed on drop.
pub struct Fake {
    child: Child,
    pub port: u16,
    log: PathBuf,
    _stdout: BufReader<ChildStdout>,
    _log_dir: TempDir,
}

impl Fake {
    /// Starts `scenario` with [`PASSWORD`] in `cwd` and reads the listening line.
    pub fn start(scenario: &Path, cwd: &Path) -> Self {
        Self::start_env(scenario, cwd, &[])
    }

    pub fn start_env(scenario: &Path, cwd: &Path, env: &[(&str, &str)]) -> Self {
        let log_dir = tempfile::tempdir().expect("temp dir");
        let log = log_dir.path().join("requests.jsonl");
        let mut command = fake_command(scenario, cwd, &["serve", "--hostname", "127.0.0.1", "--port", "0"]);
        command.env("FAKE_OPENCODE_REQUEST_LOG", &log).envs(env.iter().copied());

        let mut child = command.spawn().expect("fake_agent starts");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut line = String::new();
        stdout.read_line(&mut line).expect("listening line");

        Self {
            child,
            port: port_of(&line),
            log,
            _stdout: stdout,
            _log_dir: log_dir,
        }
    }

    pub fn send(&self, method: &str, target: &str, body: Option<&Value>) -> Response {
        self.send_as(method, target, body, Some(CREDENTIALS))
    }

    pub fn send_as(&self, method: &str, target: &str, body: Option<&Value>, credentials: Option<&str>) -> Response {
        let mut stream = self.connect(method, target, body, credentials);
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("response");

        let (head, body) = raw.split_once("\r\n\r\n").expect("response head");
        let status = head
            .split(' ')
            .nth(1)
            .and_then(|status| status.parse().ok())
            .expect("status");

        Response {
            status,
            head: head.to_owned(),
            body: body.to_owned(),
        }
    }

    fn connect(&self, method: &str, target: &str, body: Option<&Value>, credentials: Option<&str>) -> TcpStream {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream.set_read_timeout(Some(WAIT)).expect("read timeout");

        let body = body.map(Value::to_string).unwrap_or_default();
        let mut head = format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        if let Some(credentials) = credentials {
            head.push_str(&format!("Authorization: Basic {}\r\n", STANDARD.encode(credentials)));
        }
        head.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));

        stream.write_all(head.as_bytes()).expect("send request");
        stream
    }

    /// Opens `GET /api/event`, checks the SSE head and the opening `server.connected` and heartbeat.
    pub fn subscribe(&self) -> Events {
        let stream = self.connect("GET", "/api/event", None, Some(CREDENTIALS));
        let mut reader = BufReader::new(stream);
        let mut head = Vec::new();

        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("SSE head");
            if line == "\r\n" {
                break;
            }
            head.push(line.trim_end().to_owned());
        }

        assert_eq!(head[0], "HTTP/1.1 200 OK");
        assert!(head.contains(&"Content-Type: text/event-stream".to_owned()), "{head:?}");
        assert!(head.contains(&"Transfer-Encoding: chunked".to_owned()), "{head:?}");

        let mut events = Events { reader };
        assert_eq!(
            events.frame(),
            Frame::Event(json!({ "id": "evt_fakeconnected", "type": "server.connected", "data": {} }))
        );
        assert_eq!(events.frame(), Frame::Comment(": heartbeat".to_owned()));

        events
    }

    pub fn close_stdin(&mut self) {
        drop(self.child.stdin.take());
    }

    pub fn requests(&self) -> Vec<LoggedRequest> {
        read_log(&self.log)
    }

    /// The exit code and stderr, once the fake exited (panics after [`WAIT`]).
    pub fn finish(mut self) -> (i32, String) {
        let code = wait_exit(&mut self.child);
        let mut stderr = String::new();
        let mut pipe = self.child.stderr.take().expect("piped stderr");
        pipe.read_to_string(&mut stderr).expect("stderr");

        (code, stderr)
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

pub fn fake_command(scenario: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fake_agent"));
    command
        .args(["--dialect", "opencode2", "--scenario"])
        .arg(scenario)
        .args(args)
        .current_dir(cwd)
        .env("OPENCODE_SERVER_PASSWORD", PASSWORD)
        .env_remove("OPENCODE_SERVER_USERNAME")
        .env_remove("OPENCODE_CONFIG_CONTENT")
        .env_remove("FAKE_OPENCODE_REQUEST_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub fn port_of(line: &str) -> u16 {
    line.strip_prefix("server listening on http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix('\n'))
        .and_then(|port| port.parse().ok())
        .unwrap_or_else(|| panic!("not a listening line: {line:?}"))
}

fn wait_exit(child: &mut Child) -> i32 {
    let deadline = Instant::now() + WAIT;

    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status.code().expect("exit code");
        }

        assert!(Instant::now() < deadline, "fake_agent did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub struct Response {
    pub status: u16,
    pub head: String,
    pub body: String,
}

impl Response {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {:?}", self.body))
    }
}

#[derive(Debug, PartialEq)]
pub enum Frame {
    Event(Value),
    Comment(String),
    /// `data:` that is not JSON.
    Raw(String),
    /// The last chunk of a closed stream.
    End,
}

/// An open `/api/event` stream.
pub struct Events {
    reader: BufReader<TcpStream>,
}

impl Events {
    /// The next chunk: one `data: <json>\n\n` event or one `: comment\n\n`.
    pub fn frame(&mut self) -> Frame {
        let mut size = String::new();
        self.reader.read_line(&mut size).expect("chunk size");
        let size = usize::from_str_radix(size.trim_end(), 16).expect("hex chunk size");

        let mut chunk = vec![0; size + 2];
        self.reader.read_exact(&mut chunk).expect("chunk");
        assert_eq!(&chunk[size..], b"\r\n");

        if size == 0 {
            return Frame::End;
        }

        let text = String::from_utf8(chunk[..size].to_vec()).expect("UTF-8 frame");
        let text = text
            .strip_suffix("\n\n")
            .unwrap_or_else(|| panic!("not an SSE frame: {text:?}"));

        match text.strip_prefix("data: ") {
            Some(data) => serde_json::from_str(data).map_or_else(|_| Frame::Raw(data.to_owned()), Frame::Event),
            None => Frame::Comment(text.to_owned()),
        }
    }

    /// The next event, skipping comments.
    pub fn next(&mut self) -> Value {
        loop {
            match self.frame() {
                Frame::Event(event) => return event,
                Frame::Comment(_) => {}
                other => panic!("expected an event, got {other:?}"),
            }
        }
    }
}

/// A scenario file with `lines` in a fresh directory (also the fake's working directory).
pub fn script(lines: &[Value]) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("scenario.jsonl");
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    std::fs::write(&path, text.join("\n")).expect("write scenario");

    (dir, path)
}

pub fn session_target(rest: &str) -> String {
    format!("/api/session/{SESSION}/{rest}?{LOCATION}")
}

/// Plays `name` up to its `Marker` title after a new session and `prompt`; the events after the
/// startup ones and the admitted inbox entry.
pub fn play_to_marker(name: &str, prompt: &str) -> (Vec<Value>, Value) {
    let cwd = tempfile::tempdir().expect("temp dir");
    let fake = Fake::start(&scenario(name), cwd.path());
    let mut events = fake.subscribe();

    fake.send("POST", &format!("/api/session?{LOCATION}"), Some(&json!({})));
    for _ in 0..11 {
        events.next();
    }

    let admitted = fake.send("POST", &session_target("prompt"), Some(&json!({ "text": prompt })));
    assert_eq!(admitted.status, 200, "{}", admitted.body);

    let mut received = Vec::new();
    loop {
        let event = events.next();
        let marker = event["type"] == "session.renamed" && event["data"]["title"] == "Marker";
        received.push(event);

        if marker {
            return (received, admitted.json()["data"].clone());
        }
    }
}
