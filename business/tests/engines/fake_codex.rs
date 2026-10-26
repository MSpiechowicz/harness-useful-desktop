//! The scripted `codex app-server` (`fake_agent --dialect codex`) driven over stdio as the Codex
//! engine will drive it: the handshake, a thread, a turn whose three approvals the fake waits for,
//! its frames mapped by `CodexMapper`; error replies, held replies and a failed `match`.

use crate::agent_support;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use agent_support::{WAIT, scenario};
use harness_business::codex_rpc::mapper::{AskKind, CodexMapper, Hint, Mapped, StartKind};
use harness_business::codex_rpc::wire::answers::{Decision, command_decision, file_change_decision, initialized};
use harness_business::codex_rpc::wire::items::UserInput;
use harness_business::codex_rpc::wire::outbound::{Initialize, Request, ThreadSettings, ThreadStart, TurnStart};
use harness_business::codex_rpc::wire::results::{InitializeResult, ThreadSession};
use harness_business::codex_rpc::wire::{INVALID_REQUEST, Inbound, RequestId, Response, decode};
use harness_business::model::{AgentEvent, PromptStatus};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use crate::codex_recorded::THREAD;

/// A running fake Codex; killed on drop.
struct Fake {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: i64,
}

impl Fake {
    fn start(scenario: &Path, cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake_agent"))
            .args(["--dialect", "codex", "--scenario"])
            .arg(scenario)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("fake_agent starts");

        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in stdout.lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });

        Self {
            stdin: child.stdin.take(),
            child,
            lines,
            next_id: 1,
        }
    }

    fn send(&mut self, frame: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{frame}")
            .and_then(|()| stdin.flush())
            .expect("the fake reads");
    }

    /// The next frame Codex writes.
    fn next(&self) -> Inbound {
        let line = self.lines.recv_timeout(WAIT).expect("the fake writes a frame");
        decode(&line).unwrap_or_else(|error| panic!("{line}: {error:?}"))
    }

    /// Sends `request`, returns its id.
    fn request(&mut self, request: &Request) -> RequestId {
        let id = RequestId::Number(self.next_id);
        self.next_id += 1;
        self.send(&request.frame(&id));
        id
    }

    /// Sends `request` and reads its response (the next frame).
    fn call(&mut self, request: &Request) -> Response {
        let id = self.request(request);

        match self.next() {
            Inbound::Response(response) if response.id == id => response,
            other => panic!("the response to {}: {other:?}", request.method()),
        }
    }

    /// Closes stdin and waits for the exit code.
    fn finish(mut self) -> i32 {
        self.stdin.take();
        let deadline = Instant::now() + WAIT;

        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                return status.code().expect("an exit code");
            }

            assert!(Instant::now() < deadline, "fake_agent did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

#[test]
fn a_turn_waits_for_each_approval_and_maps_as_recorded() {
    let cwd = tempfile::tempdir().unwrap();
    let project = dunce::canonicalize(cwd.path()).unwrap();
    let mut fake = Fake::start(&scenario("codex_approvals.jsonl"), &project);

    let init: InitializeResult = fake
        .call(&Request::Initialize(Initialize::desktop("0.0.0-test")))
        .result()
        .unwrap();
    assert_eq!(init.version(), Some("0.154.0"));
    fake.send(&initialized());

    let start = Request::ThreadStart(ThreadStart {
        settings: ThreadSettings {
            cwd: Some(project.to_string_lossy().into_owned()),
            ..ThreadSettings::default()
        },
        ..ThreadStart::default()
    });
    let session: ThreadSession = fake.call(&start).result().unwrap();
    assert_eq!(session.thread.id, THREAD);
    assert_eq!(Path::new(&session.cwd), project);

    let mut mapper = CodexMapper::new(&project);
    mapper.follow_thread(&session.thread.id);
    mapper.sent(StartKind::Prompt);
    let turn = fake.request(&Request::TurnStart(Box::new(TurnStart {
        thread_id: session.thread.id.clone(),
        input: vec![UserInput::text("Create hello.txt, then run ls -1.")],
        ..TurnStart::default()
    })));

    let mut events = Vec::new();
    let mut answered = Vec::new();

    while !events.iter().any(|event| matches!(event, AgentEvent::Settled { .. })) {
        for mapped in mapper.on_frame(&fake.next()) {
            match mapped {
                Mapped::Event(event) => events.push(*event),
                Mapped::Hint(Hint::Response(response)) => assert_eq!(response.id, turn),
                Mapped::Hint(Hint::Ask(ask)) => {
                    let answer = match ask.kind {
                        AskKind::FileChange { .. } => file_change_decision(&ask.request_id, Decision::Accept),
                        _ => command_decision(&ask.request_id, Decision::Accept),
                    };
                    answered.push(ask.request_id.clone());
                    fake.send(&answer);
                }
                Mapped::Hint(_) => {}
            }
        }
    }

    assert_eq!(answered, [0, 1, 2].map(RequestId::Number));
    let touched: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd { touched_paths, .. } => Some(touched_paths.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(touched, ["hello.txt"]);
    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Completed,
        error: None,
        retryable: false,
    }));
    assert_eq!(fake.finish(), 0);
}

#[test]
fn replies_can_fail_wait_or_refuse_the_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scenario.jsonl");
    let steps = [
        json!({ "expect": "turn/steer", "error": "no active turn to steer" }),
        json!({ "expect": "thread/name/set", "as": "rename", "hold": true }),
        json!({ "emit": { "method": "thread/name/updated", "params": { "threadId": "t", "threadName": "Renamed" } } }),
        json!({ "respond": "rename", "reply": {} }),
        json!({ "expect": "turn/start", "match": { "/params/threadId": "other" } }),
    ];
    let text: Vec<String> = steps.iter().map(Value::to_string).collect();
    std::fs::write(&path, text.join("\n")).unwrap();

    let mut fake = Fake::start(&path, dir.path());

    let steer = Request::TurnSteer(Default::default());
    let refused = fake.call(&steer).outcome.unwrap_err();
    assert_eq!(
        (refused.code, refused.message.as_str()),
        (INVALID_REQUEST, "no active turn to steer")
    );

    let rename = fake.request(&Request::ThreadNameSet {
        thread_id: "t".to_owned(),
        name: "Renamed".to_owned(),
    });
    assert!(
        matches!(fake.next(), Inbound::Notification(_)),
        "the echo comes before the held reply"
    );
    assert!(matches!(fake.next(), Inbound::Response(response) if response.id == rename));

    fake.request(&Request::TurnStart(Box::new(TurnStart {
        thread_id: "t".to_owned(),
        ..TurnStart::default()
    })));
    assert_eq!(fake.finish(), 4, "a failed match exits 4");
}
