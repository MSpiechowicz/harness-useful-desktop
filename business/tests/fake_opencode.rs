//! The scripted `opencode serve` (`fake_agent --dialect opencode`) driven over raw HTTP: the
//! listening line, Basic auth, `allow` / `expect` / `hold` steps, the SSE stream and exit codes.

mod agent_support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use agent_support::{WAIT, scenario};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

const PASSWORD: &str = "fake-server-password";
const CREDENTIALS: &str = "opencode:fake-server-password";
const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

/// A running fake server; killed on drop.
struct Fake {
    child: Child,
    port: u16,
    _stdout: BufReader<ChildStdout>,
}

impl Fake {
    /// Starts `scenario` with [`PASSWORD`] in `cwd` and reads the listening line.
    fn start(scenario: &Path, cwd: &Path) -> Self {
        let mut child = spawn("opencode", scenario, cwd);
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut line = String::new();
        stdout.read_line(&mut line).expect("listening line");

        let port = line
            .strip_prefix("opencode server listening on http://127.0.0.1:")
            .and_then(|rest| rest.strip_suffix('\n'))
            .and_then(|port| port.parse().ok())
            .unwrap_or_else(|| panic!("not a listening line: {line:?}"));

        Self {
            child,
            port,
            _stdout: stdout,
        }
    }

    fn send(&self, method: &str, target: &str, body: Option<&Value>) -> Response {
        self.send_as(method, target, body, Some(CREDENTIALS))
    }

    fn send_as(&self, method: &str, target: &str, body: Option<&Value>, credentials: Option<&str>) -> Response {
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

    /// Opens `GET /global/event` and checks the SSE response head.
    fn subscribe(&self) -> Events {
        let stream = self.connect("GET", "/global/event", None, Some(CREDENTIALS));
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

        Events { reader }
    }

    fn close_stdin(&mut self) {
        drop(self.child.stdin.take());
    }

    /// Whether the fake still runs.
    fn running(&mut self) -> bool {
        self.child.try_wait().expect("try_wait").is_none()
    }

    /// The exit code and stderr, once the fake exited (panics after [`WAIT`]).
    fn finish(mut self) -> (i32, String) {
        let code = wait_exit(&mut self.child);
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .expect("piped stderr")
            .read_to_string(&mut stderr)
            .expect("stderr");

        (code, stderr)
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn spawn(dialect: &str, scenario: &Path, cwd: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_fake_agent"))
        .args(["--dialect", dialect, "--scenario"])
        .arg(scenario)
        .current_dir(cwd)
        .env("OPENCODE_SERVER_PASSWORD", PASSWORD)
        .env_remove("OPENCODE_SERVER_USERNAME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("fake_agent starts")
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

struct Response {
    status: u16,
    head: String,
    body: String,
}

impl Response {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {:?}", self.body))
    }
}

/// An open `/global/event` stream.
struct Events {
    reader: BufReader<TcpStream>,
}

impl Events {
    /// The next event: one chunk holding `data: <json>\n\n`.
    fn next(&mut self) -> Value {
        let mut size = String::new();
        self.reader.read_line(&mut size).expect("chunk size");
        let size = usize::from_str_radix(size.trim_end(), 16).expect("hex chunk size");

        let mut chunk = vec![0; size + 2];
        self.reader.read_exact(&mut chunk).expect("chunk");
        assert_eq!(&chunk[size..], b"\r\n");

        let text = String::from_utf8(chunk[..size].to_vec()).expect("UTF-8 event");
        let data = text
            .strip_prefix("data: ")
            .and_then(|data| data.strip_suffix("\n\n"))
            .unwrap_or_else(|| panic!("not an SSE event: {text:?}"));

        serde_json::from_str(data).expect("JSON event")
    }
}

/// A scenario file with `lines` in a fresh directory (also the fake's working directory).
fn script(lines: &[Value]) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("scenario.jsonl");
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    std::fs::write(&path, text.join("\n")).expect("write scenario");

    (dir, path)
}

fn recorded_body(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/opencode/server")
        .join(name);
    let recording: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("recording")).expect("JSON");
    recording["body"].clone()
}

#[test]
fn allowed_routes_answer_any_number_of_times() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let fake = Fake::start(&scenario("opencode_smoke.jsonl"), cwd.path());

    for _ in 0..3 {
        let health = fake.send("GET", "/global/health", None);
        assert_eq!(health.status, 200);
        assert_eq!(health.json(), json!({ "healthy": true, "version": "1.19.3" }));
        assert!(
            health.head.contains("Content-Type: application/json"),
            "{}",
            health.head
        );
    }

    let agents = fake.send("GET", "/agent?directory=%2Fwork%2Fproject", None);
    assert_eq!(agents.status, 200);
    assert_eq!(agents.json(), recorded_body("agent-list.json"));
    assert_eq!(fake.send("GET", "/session/status", None).json(), json!({}));
}

#[test]
fn requests_without_the_password_get_401() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let fake = Fake::start(&scenario("opencode_smoke.jsonl"), cwd.path());

    let anonymous = fake.send_as("GET", "/global/health", None, None);
    assert_eq!(anonymous.status, 401);
    assert!(
        anonymous.head.contains("www-authenticate: Basic realm=\"Secure Area\""),
        "{}",
        anonymous.head
    );
    assert_eq!(anonymous.body, "");

    let wrong = fake.send_as("GET", "/global/health", None, Some("opencode:guess"));
    assert_eq!(wrong.status, 401);
    assert_eq!(fake.send_as("GET", "/global/event", None, None).status, 401);

    let token = format!("/global/health?auth_token={}", STANDARD.encode(CREDENTIALS));
    assert_eq!(fake.send_as("GET", &token, None, None).status, 200);
}

#[test]
fn the_smoke_turn_streams_the_recorded_events_in_order() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let mut fake = Fake::start(&scenario("opencode_smoke.jsonl"), cwd.path());
    let mut events = fake.subscribe();
    assert_eq!(events.next()["payload"]["type"], "server.connected");

    let created = fake.send("POST", "/session?directory=%2Fwork%2Fmy%20project", Some(&json!({})));
    assert_eq!(created.status, 200);
    assert_eq!(created.json()["id"], SESSION);
    assert_eq!(created.json()["directory"], "/work/my project");

    let prompt = json!({ "parts": [{ "type": "text", "text": "Say hello" }] });
    let accepted = fake.send("POST", &format!("/session/{SESSION}/prompt_async"), Some(&prompt));
    assert_eq!((accepted.status, accepted.body.as_str()), (204, ""));

    let received: Vec<Value> = (0..16).map(|_| events.next()).collect();
    let kinds: Vec<&str> = received
        .iter()
        .map(|event| event["payload"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "message.updated",
            "message.part.updated",
            "session.status",
            "message.updated",
            "message.part.updated",
            "message.part.updated",
            "message.part.delta",
            "message.part.delta",
            "message.part.delta",
            "message.part.delta",
            "message.part.delta",
            "message.part.updated",
            "message.part.updated",
            "message.updated",
            "session.status",
            "session.idle",
        ]
    );

    let reply: String = received
        .iter()
        .filter_map(|event| event["payload"]["properties"]["delta"].as_str())
        .collect();
    assert_eq!(reply, "Hello from the mock model.");
    assert!(received.iter().all(|event| event["directory"] == "/work/my project"));
    assert_eq!(received[14]["payload"]["properties"]["status"]["type"], "idle");

    fake.close_stdin();
    assert_eq!(fake.finish().0, 0);
}

#[test]
fn a_mismatched_field_gets_500_and_exit_4_at_the_end() {
    let (dir, path) = script(&[
        json!({ "expect": "POST /session", "match": { "/title": "Plan", "query.directory": "/p" } }),
        json!({ "expect": "GET /session/*", "reply": { "body": { "id": "ses_1" } } }),
    ]);
    let fake = Fake::start(&path, dir.path());

    let rejected = fake.send("POST", "/session?directory=/p", Some(&json!({ "title": "Other" })));
    assert_eq!(rejected.status, 500);
    assert!(rejected.body.contains("/title"), "{}", rejected.body);

    // The failed step is consumed: the next request meets the next step.
    assert_eq!(
        fake.send("GET", "/session/ses_1", None).json(),
        json!({ "id": "ses_1" })
    );

    let (code, stderr) = fake.finish();
    assert_eq!(code, 4);
    assert!(stderr.contains("expected POST /session: /title was"), "{stderr}");
}

#[test]
fn a_request_for_another_route_or_after_the_end_fails() {
    let (dir, path) = script(&[json!({ "expect": "POST /session" })]);
    let wrong_route = Fake::start(&path, dir.path());
    assert_eq!(wrong_route.send("GET", "/agent", None).status, 500);
    let (code, stderr) = wrong_route.finish();
    assert_eq!(code, 4);
    assert!(stderr.contains("got GET /agent"), "{stderr}");

    let after_end = Fake::start(&path, dir.path());
    assert_eq!(after_end.send("POST", "/session", None).status, 204);
    assert_eq!(after_end.send("POST", "/session", None).status, 500);
    assert_eq!(after_end.finish().0, 4);
}

#[test]
fn a_held_request_is_answered_after_later_steps() {
    let (dir, path) = script(&[
        json!({ "expect": "POST /session/*/shell", "as": "shell", "hold": true }),
        json!({ "sse": { "type": "session.status", "properties": { "status": { "type": "busy" } } } }),
        json!({ "sleep": 300 }),
        json!({ "respond": "shell", "reply": { "body": { "id": "msg_1" } } }),
    ]);
    let fake = Fake::start(&path, dir.path());
    let mut events = fake.subscribe();
    events.next();

    std::thread::scope(|scope| {
        let shell = scope.spawn(|| fake.send("POST", "/session/ses_1/shell", Some(&json!({ "command": "ls" }))));

        // Without a `?directory=` yet, `$directory` is the fake's working directory.
        let busy = events.next();
        assert_eq!(busy["payload"]["type"], "session.status");
        let cwd = dunce::canonicalize(dir.path()).expect("canonical cwd");
        assert_eq!(busy["directory"], json!(cwd.to_string_lossy()));
        assert!(!shell.is_finished(), "the shell reply waits for `respond`");

        assert_eq!(shell.join().unwrap().json(), json!({ "id": "msg_1" }));
    });
}

#[test]
fn stdin_eof_before_the_last_step_is_ignored() {
    let (dir, path) = script(&[
        json!({ "allow": "GET /global/health", "reply": { "body": true } }),
        json!({ "expect": "POST /session" }),
    ]);
    let mut fake = Fake::start(&path, dir.path());

    fake.close_stdin();
    std::thread::sleep(Duration::from_millis(200));
    assert!(fake.running(), "the real server keeps running after stdin EOF");
    assert_eq!(fake.send("GET", "/global/health", None).status, 200);

    assert_eq!(fake.send("POST", "/session", None).status, 204);
    assert_eq!(fake.finish().0, 0);
}

#[test]
fn exit_steps_end_the_fake_with_their_code() {
    let (dir, path) = script(&[json!({ "expect": "POST /session" }), json!({ "exit": 3 })]);
    let fake = Fake::start(&path, dir.path());
    fake.send("POST", "/session", None);
    assert_eq!(fake.finish().0, 3);

    // Steps before `listen` run before the listening line: a server that dies while starting.
    let (dir, path) = script(&[
        json!({ "stderr": "boom" }),
        json!({ "exit": 1 }),
        json!({ "listen": true }),
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_fake_agent"))
        .args(["--dialect", "opencode", "--scenario"])
        .arg(&path)
        .current_dir(dir.path())
        .output()
        .expect("fake_agent runs");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"boom\n");
}

#[test]
fn an_unknown_dialect_exits_2() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let mut child = spawn("opencod", &scenario("opencode_smoke.jsonl"), cwd.path());

    assert_eq!(wait_exit(&mut child), 2);
    let mut stderr = String::new();
    child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    assert!(stderr.contains("unknown dialect: opencod"), "{stderr}");
}
