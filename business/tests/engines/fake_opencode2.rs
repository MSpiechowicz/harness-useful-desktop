//! The scripted OpenCode 2 `opencode serve` (`fake_agent --dialect opencode2`) driven over raw
//! HTTP: the command line, the listening line, Basic auth, `{data}` replies and default replies,
//! the SSE stream with heartbeats, `allow` / `on` / `expect` steps, the request log and exit codes.

use crate::agent_support;
use crate::fake_opencode2_support;
use crate::opencode2_engine_support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Stdio;

use agent_support::scenario;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use fake_opencode2_support::{
    CREDENTIALS, Fake, Frame, LOCATION, PASSWORD, fake_command, play_to_marker, port_of, script, session_target,
};
use harness_business::process::ProcessSpawner;
use opencode2_engine_support::{FakeServerSpawner, SESSION, serve_spec};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;

#[test]
fn info_needs_the_basic_password() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let fake = Fake::start(&scenario("opencode2_startup.jsonl"), cwd.path());

    let anonymous = fake.send_as("GET", "/api/info", None, None);
    assert_eq!(anonymous.status, 401);
    assert!(
        anonymous.head.contains("www-authenticate: Basic realm=\"Secure Area\""),
        "{}",
        anonymous.head
    );
    assert_eq!(
        fake.send_as("GET", "/api/info", None, Some("opencode:guess")).status,
        401
    );
    assert_eq!(fake.send_as("GET", "/api/event", None, None).status, 401);

    let info = fake.send("GET", "/api/info", None);
    assert_eq!(info.status, 200);
    assert_eq!(info.json()["version"], "2.1.2");
    assert_eq!(info.json()["urls"], json!([format!("http://127.0.0.1:{}", fake.port)]));
    assert_eq!(info.json()["capabilities"], json!({ "persistentPty": true }));

    let logged: Vec<(String, bool)> = fake
        .requests()
        .into_iter()
        .map(|request| (request.target, request.authorized))
        .collect();
    assert_eq!(
        logged,
        [
            ("/api/info".to_owned(), false),
            ("/api/info".to_owned(), false),
            ("/api/event".to_owned(), false),
            ("/api/info".to_owned(), true),
        ]
    );
}

#[test]
fn the_command_line_refuses_mdns_and_other_addresses() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let refused: [&[&str]; 4] = [
        &["serve", "--hostname", "127.0.0.1", "--port", "0", "--mdns=false"],
        &["serve", "--mdns"],
        &["serve", "--hostname", "0.0.0.0", "--port", "0"],
        &["run", "hello"],
    ];

    for args in refused {
        let output = fake_command(&scenario("opencode2_startup.jsonl"), cwd.path(), args)
            .stdin(Stdio::null())
            .output()
            .expect("fake_agent runs");

        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert_eq!(output.stdout, b"", "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("opencode serve"),
            "{args:?}"
        );
    }
}

#[test]
fn info_answers_503_as_often_as_scripted() {
    let (dir, path) = script(&[json!({ "allow": "GET /api/info", "reply": { "status": 503 }, "times": 2 })]);
    let fake = Fake::start(&path, dir.path());

    let statuses: Vec<u16> = (0..3).map(|_| fake.send("GET", "/api/info", None).status).collect();
    assert_eq!(statuses, [503, 503, 200]);
}

#[test]
fn the_smoke_turn_replays_the_recorded_run() {
    let cwd = tempfile::tempdir().expect("temp dir");
    let mut fake = Fake::start(&scenario("opencode2_smoke.jsonl"), cwd.path());
    let mut events = fake.subscribe();

    let agents = fake.send("GET", &format!("/api/agent?{LOCATION}"), None);
    assert_eq!(agents.json()["location"], json!({ "directory": "/work/my project" }));
    assert_eq!(agents.json()["data"][0]["id"], "build");
    let commands = fake.send("GET", &format!("/api/command?{LOCATION}"), None).json();
    assert_eq!(commands["data"][1]["name"], "review");
    let config = fake.send("GET", &format!("/api/config?{LOCATION}"), None).json();
    assert_eq!(config[0]["type"], "directory");

    let created = fake.send("POST", &format!("/api/session?{LOCATION}"), Some(&json!({})));
    assert_eq!(created.status, 200);
    assert_eq!(created.json()["data"]["id"], SESSION);
    assert_eq!(
        created.json()["data"]["location"],
        json!({ "directory": "/work/my project" })
    );

    let startup: Vec<String> = (0..11)
        .map(|_| events.next()["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(startup[0], "session.created");

    let prompt = json!({ "text": "Reply with exactly the word: hello" });
    let admitted = fake.send("POST", &session_target("prompt"), Some(&prompt));
    assert_eq!(admitted.status, 200);
    let inbox = admitted.json()["data"].clone();
    assert_eq!(inbox["id"], "msg_15d7bc2db001z7i7HF20ZowIPY");
    assert_eq!(inbox["sessionID"], SESSION);
    assert_eq!(inbox["type"], "user");
    assert_eq!(inbox["payload"], prompt);
    assert_eq!(inbox["delivery"], "steer");

    let received: Vec<Value> = (0..15).map(|_| events.next()).collect();
    let kinds: Vec<&str> = received.iter().map(|event| event["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        [
            "plugin.updated",
            "session.inbox.enqueued",
            "session.execution.started",
            "session.instructions.updated",
            "session.inbox.delivered",
            "session.step.started",
            "session.text.started",
            "session.text.delta",
            "session.text.ended",
            "session.step.streamed",
            "session.step.ended",
            "session.usage.updated",
            "session.execution.succeeded",
            "session.usage.updated",
            "session.renamed",
        ]
    );
    assert_eq!(received[1]["location"], json!({ "directory": "/work/my project" }));
    assert_eq!(received[7]["data"]["delta"], "hello");

    let page = fake.send(
        "GET",
        &format!("/api/session/{SESSION}/message?limit=50&{LOCATION}"),
        None,
    );
    assert_eq!(page.json()["data"][0]["type"], "idle");

    fake.close_stdin();
    assert_eq!(fake.finish().0, 0);
}

#[test]
fn events_get_ids_and_frames_follow_the_steps() {
    let (dir, path) = script(&[
        json!({ "expect": "POST /api/session/*/interrupt" }),
        json!({ "sse": { "type": "session.execution.interrupted", "data": { "sessionID": "$path/2", "reason": "user" } } }),
        json!({ "heartbeat": true }),
        json!({ "sseRaw": "not json" }),
        json!({ "sse": { "id": "evt_kept", "created": 7, "type": "x.y", "data": {} } }),
        json!({ "sseClose": true }),
    ]);
    let fake = Fake::start(&path, dir.path());
    let mut events = fake.subscribe();

    let interrupted = fake.send("POST", &format!("/api/session/{SESSION}/interrupt"), None);
    assert_eq!(
        (interrupted.status, interrupted.json()),
        (200, json!({ "interrupted": true }))
    );

    let Frame::Event(event) = events.frame() else {
        panic!("an event first");
    };
    assert!(event["id"].as_str().unwrap().starts_with("evt_fake"), "{event}");
    assert!(event["created"].as_u64().unwrap() > 1_700_000_000_000, "{event}");
    assert_eq!(event["data"], json!({ "sessionID": SESSION, "reason": "user" }));

    assert_eq!(events.frame(), Frame::Comment(": heartbeat".to_owned()));
    assert_eq!(events.frame(), Frame::Raw("not json".to_owned()));
    assert_eq!(
        events.frame(),
        Frame::Event(json!({ "id": "evt_kept", "created": 7, "type": "x.y", "data": {} }))
    );
    assert_eq!(events.frame(), Frame::End);
}

#[test]
fn triggers_answer_in_order_and_emit_events_about_their_request() {
    let (dir, path) = script(&[
        json!({ "on": "POST /api/session/*/prompt", "match": { "/delivery": "$absent" }, "reply": { "inboxID": "msg_first" },
                "emit": [{ "sse": { "type": "session.inbox.enqueued", "data": { "sessionID": "$path/2", "inboxID": "$inbox", "text": "$request/text" } } }] }),
        json!({ "on": "POST /api/session/*/prompt", "emit": [{ "sse": { "type": "second", "data": { "inboxID": "$inbox" } } }] }),
        json!({ "on": "POST /api/session/*/permission/*/reply", "match": { "decision": "once" },
                "emit": [{ "sse": { "type": "session.permission.replied", "data": { "requestID": "$path/4", "reply": "$request/decision" } } }] }),
    ]);
    let fake = Fake::start(&path, dir.path());
    let mut events = fake.subscribe();

    let first = fake.send("POST", &session_target("prompt"), Some(&json!({ "text": "one" })));
    assert_eq!(first.json()["data"]["id"], "msg_first");
    assert_eq!(
        events.next()["data"],
        json!({ "sessionID": SESSION, "inboxID": "msg_first", "text": "one" })
    );

    let second = fake.send(
        "POST",
        &session_target("prompt"),
        Some(&json!({ "text": "two", "delivery": "queue" })),
    );
    let inbox = second.json()["data"].clone();
    assert!(inbox["id"].as_str().unwrap().starts_with("msg_fakeinbox"), "{inbox}");
    assert_eq!(inbox["delivery"], "queue");
    let event = events.next();
    assert_eq!(
        (event["type"].clone(), event["data"]["inboxID"].clone()),
        (json!("second"), inbox["id"].clone())
    );

    let reply = fake.send(
        "POST",
        &session_target("permission/per_1/reply"),
        Some(&json!({ "decision": "once" })),
    );
    assert_eq!((reply.status, reply.body.as_str()), (204, ""));
    assert_eq!(events.next()["data"], json!({ "requestID": "per_1", "reply": "once" }));

    // Every trigger fired: a third prompt has nobody to answer it after the scenario.
    assert_eq!(
        fake.send("POST", &session_target("prompt"), Some(&json!({ "text": "three" })))
            .status,
        500
    );
    assert_eq!(fake.finish().0, 4);
}

#[test]
fn a_trigger_that_never_fired_fails_the_run() {
    let (dir, path) = script(&[json!({ "on": "POST /api/session/*/compact" })]);
    let mut fake = Fake::start(&path, dir.path());

    fake.close_stdin();
    let (code, stderr) = fake.finish();
    assert_eq!(code, 4);
    assert!(
        stderr.contains("the trigger on POST /api/session/*/compact never fired"),
        "{stderr}"
    );
}

#[test]
fn routes_answer_their_defaults_and_envelopes() {
    let (dir, path) = script(&[
        json!({ "allow": "GET /api/session/*/message", "match": { "query.cursor": "$absent" },
                "reply": { "data": [{ "id": "msg_2" }], "cursor": { "previous": null, "next": "c1" } } }),
        json!({ "allow": "GET /api/session/*/message", "match": { "query.cursor": "c1" },
                "reply": { "data": [{ "id": "msg_1" }], "cursor": { "previous": "c0", "next": null } } }),
        json!({ "expect": "POST /api/session/*/command", "match": { "name": "review", "text": "" } }),
        json!({ "expect": "POST /api/session/*/shell", "match": { "command": "ls" }, "as": "shell", "hold": true }),
        json!({ "sse": { "type": "session.shell.started", "data": { "sessionID": "$path/2" } } }),
        json!({ "respond": "shell" }),
        json!({ "expect": "POST /api/session/*/compact" }),
        json!({ "expect": "POST /api/session/*/fork", "match": { "before": "$absent" },
                "reply": { "data": { "id": "ses_fork", "fork": { "sessionID": "$path/2", "boundary": { "type": "through" } } } } }),
        json!({ "expect": "PATCH /api/session/*", "match": { "title": "Renamed" } }),
        json!({ "expect": "GET /api/config" }),
    ]);
    let content = r#"{"agents":{"build":{"permissions":[{"action":"edit","resource":"*","effect":"ask"}]}}}"#;
    let fake = Fake::start_env(&path, dir.path(), &[("OPENCODE_CONFIG_CONTENT", content)]);
    let mut events = fake.subscribe();

    let first = fake
        .send(
            "GET",
            &format!("/api/session/{SESSION}/message?limit=1&{LOCATION}"),
            None,
        )
        .json();
    assert_eq!(first["cursor"]["next"], "c1");
    let second = fake
        .send(
            "GET",
            &format!("/api/session/{SESSION}/message?limit=1&cursor=c1&{LOCATION}"),
            None,
        )
        .json();
    assert_eq!(
        (second["data"][0]["id"].clone(), second["cursor"]["next"].clone()),
        (json!("msg_1"), Value::Null)
    );

    let command = json!({ "name": "review", "text": "" });
    assert_eq!(
        fake.send("POST", &session_target("command"), Some(&command)).status,
        204
    );

    std::thread::scope(|scope| {
        let shell = scope.spawn(|| fake.send("POST", &session_target("shell"), Some(&json!({ "command": "ls" }))));
        assert_eq!(events.next()["type"], "session.shell.started");
        assert_eq!(shell.join().unwrap().status, 204);
    });

    let compact = fake.send("POST", &session_target("compact"), Some(&json!({})));
    assert_eq!(compact.status, 200);
    assert_eq!(compact.json()["data"]["type"], "compaction");
    assert_eq!(compact.json()["data"]["payload"], json!({}));

    let fork = fake.send("POST", &session_target("fork"), Some(&json!({}))).json();
    assert_eq!(fork["data"]["fork"]["sessionID"], SESSION);

    let renamed = fake.send(
        "PATCH",
        &format!("/api/session/{SESSION}?{LOCATION}"),
        Some(&json!({ "title": "Renamed" })),
    );
    assert_eq!(renamed.status, 204);

    let config = fake.send("GET", &format!("/api/config?{LOCATION}"), None).json();
    assert_eq!(config[1]["type"], "document");
    assert_eq!(config[1]["info"]["agents"]["build"]["permissions"][0]["effect"], "ask");

    let posted: Vec<(String, Value)> = fake
        .requests()
        .into_iter()
        .filter(|request| request.method != "GET")
        .map(|request| (request.path().to_owned(), request.body))
        .collect();
    assert_eq!(posted[0], (format!("/api/session/{SESSION}/command"), command));
    assert_eq!(posted.len(), 5);
}

#[test]
fn a_mismatched_request_gets_500_and_exit_4_at_the_end() {
    let (dir, path) = script(&[
        json!({ "expect": "POST /api/session/*/prompt", "match": { "/text": "hello", "query.location[directory]": "/p" } }),
        json!({ "expect": "DELETE /api/session/*" }),
    ]);
    let fake = Fake::start(&path, dir.path());

    let rejected = fake.send("POST", &session_target("prompt"), Some(&json!({ "text": "other" })));
    assert_eq!(rejected.status, 500);
    assert!(rejected.body.contains("/text"), "{}", rejected.body);

    assert_eq!(
        fake.send("DELETE", &format!("/api/session/{SESSION}?{LOCATION}"), None)
            .status,
        204
    );

    let (code, stderr) = fake.finish();
    assert_eq!(code, 4);
    assert!(
        stderr.contains("expected POST /api/session/*/prompt: /text was"),
        "{stderr}"
    );
}

#[test]
fn crash_and_no_listen_steps_end_the_server() {
    let (dir, path) = script(&[
        json!({ "expect": "POST /api/session" }),
        json!({ "crash": "panic: boom" }),
    ]);
    let fake = Fake::start(&path, dir.path());
    fake.send("POST", &format!("/api/session?{LOCATION}"), None);
    let (code, stderr) = fake.finish();
    assert_eq!((code, stderr.as_str()), (1, "panic: boom\n"));

    let (dir, path) = script(&[
        json!({ "listen": false }),
        json!({ "stderr": "database migration failed" }),
        json!({ "exit": 2 }),
    ]);
    let output = fake_command(&path, dir.path(), &[])
        .stdin(Stdio::null())
        .output()
        .expect("fake_agent runs");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"database migration failed\n");
}

#[tokio::test]
async fn the_support_spawner_passes_the_engine_command_line() {
    let project = tempfile::tempdir().expect("temp project");
    let log_dir = tempfile::tempdir().expect("temp log dir");
    let log = log_dir.path().join("requests.jsonl");
    let spawner = FakeServerSpawner::new("opencode2_startup.jsonl", log.clone());

    let spec = serve_spec(project.path(), &[("OPENCODE_SERVER_PASSWORD", PASSWORD)]);
    let mut process = spawner.spawn(&spec).expect("spawned");
    let mut stdout = tokio::io::BufReader::new(process.stdout);
    let mut line = String::new();
    stdout.read_line(&mut line).await.expect("listening line");
    let port = port_of(&line);

    let fake_port = port;
    let status = tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", fake_port)).expect("connect");
        let request = format!(
            "GET /api/info HTTP/1.1\r\nAuthorization: Basic {}\r\nConnection: close\r\n\r\n",
            STANDARD.encode(CREDENTIALS)
        );
        stream.write_all(request.as_bytes()).expect("send");
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("response");
        raw.split(' ').nth(1).unwrap_or_default().to_owned()
    })
    .await
    .expect("request");
    assert_eq!(status, "200");

    assert_eq!(spawner.requests()[0].target, "/api/info");
    assert!(spawner.requests()[0].authorized);
    process.child.start_kill().expect("kill");
    process.child.wait().await.expect("reaped");
    assert_eq!(spawner.exited(), 1);

    let mut refused = serve_spec(project.path(), &[]);
    refused.args.push("--mdns=false".into());
    let mut process = spawner.spawn(&refused).expect("spawned");
    let exit = process.child.wait().await.expect("exits");
    assert_eq!(exit.code, Some(1));
}

#[test]
fn the_turn_and_tools_scenarios_play_to_their_marker() {
    let (turn, inbox) = play_to_marker("opencode2_turn.jsonl", "Say hello");
    assert_eq!(turn[0]["type"], "session.inbox.enqueued");
    assert_eq!(turn[0]["data"]["inboxID"], inbox["id"]);
    let reply: String = turn
        .iter()
        .filter(|event| event["type"] == "session.text.delta")
        .filter_map(|event| event["data"]["delta"].as_str())
        .collect();
    assert_eq!(reply, "Hello from the fake server.");
    let about_the_session = turn
        .iter()
        .filter(|event| event["type"].as_str().unwrap().starts_with("session."));
    assert!(about_the_session.clone().count() > 10);
    assert!(
        about_the_session
            .into_iter()
            .all(|event| event["data"]["sessionID"] == SESSION),
        "{turn:#?}"
    );

    let prompt = "Use the read tool to read notes.txt, then use the glob tool to list *.txt files. Then reply \
                  with one short sentence.";
    let (tools, inbox) = play_to_marker("opencode2_tools.jsonl", prompt);
    assert_eq!(inbox["id"], "msg_15d7be1e4001f83TAnUjlOmbv8");
    let called: Vec<&str> = tools
        .iter()
        .filter(|event| event["type"] == "session.tool.input.started")
        .filter_map(|event| event["data"]["name"].as_str())
        .collect();
    assert_eq!(called, ["read", "glob"]);
    let glob = tools
        .iter()
        .filter(|event| event["type"] == "session.tool.success")
        .nth(1)
        .expect("the glob result");
    assert_eq!(glob["data"]["content"][0]["text"], "/work/my project/notes.txt");
}
