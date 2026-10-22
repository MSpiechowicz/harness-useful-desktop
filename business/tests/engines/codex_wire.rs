//! Codex app-server frames (`codex_rpc::wire`): every recorded frame decodes, crafted frames that
//! do not fit are tolerated or reported, the results of the app's requests read as recorded, and
//! the app's requests and answers are written as recorded.

use harness_business::codex_rpc::wire::answers::{Decision, command_decision, file_change_decision, initialized};
use harness_business::codex_rpc::wire::items::{ThreadItem, TurnStatus, UserInput};
use harness_business::codex_rpc::wire::notifications::Notification;
use harness_business::codex_rpc::wire::outbound::{
    ApprovalPolicy, Initialize, Paged, Request, SandboxMode, ThreadFork, ThreadList, ThreadSettings, ThreadStart,
    TurnStart, TurnSteer,
};
use harness_business::codex_rpc::wire::results::{
    Account, AccountRead, InitializeResult, ItemEntry, McpServerStatus, Model, Page, SkillsList, Thread, ThreadSession,
    ThreadStatus, TurnStarted,
};
use harness_business::codex_rpc::wire::{
    INVALID_REQUEST, Inbound, RequestId, Response, ServerRequestKind, decode, decode_value,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use crate::codex_recorded::{Dir, FILES, PROJECT, THREAD, frame, frames, result};

fn response(file: &str, id: i64) -> Response {
    Response {
        id: RequestId::Number(id),
        outcome: Ok(result(file, id)),
    }
}

#[test]
fn every_recorded_frame_decodes_and_known_methods_are_known() {
    for file in FILES {
        for frame in frames(file, Dir::In) {
            let method = frame["method"].as_str().unwrap_or("response").to_owned();
            let decoded = decode(&frame.to_string()).unwrap_or_else(|error| panic!("{file} {method}: {error:?}"));

            if let Inbound::Notification(Notification::Unknown { method }) = &decoded {
                panic!("{file}: an unknown notification {method}");
            }

            if let Inbound::Request(request) = &decoded {
                assert!(!matches!(request.kind, ServerRequestKind::Unknown), "{file}: {method}");
            }
        }
    }
}

#[test]
fn frames_are_told_apart_by_their_id_and_method() {
    let approval = frame(
        "02_turn_approvals_accept.jsonl",
        Dir::In,
        "item/commandExecution/requestApproval",
        0,
    );
    let Ok(Inbound::Request(request)) = decode_value(approval) else {
        panic!("a request");
    };
    assert_eq!(request.id, RequestId::Number(0));
    assert!(matches!(request.kind, ServerRequestKind::CommandApproval(_)));

    let steer_error = json!({ "id": "13", "error": { "code": -32600, "message": "no active turn to steer" } });
    let Ok(Inbound::Response(refused)) = decode_value(steer_error) else {
        panic!("a response");
    };
    assert_eq!(refused.id, RequestId::Text("13".to_owned()));
    let error = refused.outcome.unwrap_err();
    assert_eq!(
        (error.code, error.message.as_str()),
        (INVALID_REQUEST, "no active turn to steer")
    );

    let folded = json!({ "method": "turn/moderationMetadata", "params": { "metadata": { "prompt": {} } } });
    assert!(matches!(
        decode_value(folded),
        Ok(Inbound::Notification(Notification::Folded { .. }))
    ));

    let unknown = json!({ "method": "thread/realtime/started", "params": null });
    assert!(matches!(
        decode_value(unknown),
        Ok(Inbound::Notification(Notification::Unknown { .. }))
    ));
}

#[test]
fn frames_that_do_not_fit_are_errors_without_their_content() {
    let cases = [
        ("[1, 2]", None, "The frame is not a JSON object."),
        (
            r#"{"jsonrpc": "2.0"}"#,
            None,
            "The frame is neither a request, a response nor a notification.",
        ),
        (
            r#"{"id": [1], "result": {}}"#,
            None,
            "The frame's id is neither a number nor a string.",
        ),
        (
            r#"{"method": "turn/completed", "params": {"turn": "secret text"}}"#,
            Some("turn/completed"),
            "The notification does not have the expected shape.",
        ),
    ];

    for (line, method, error) in cases {
        let decoded = decode(line).unwrap_err();
        assert_eq!(
            (decoded.method.as_deref(), decoded.error.as_str()),
            (method, error),
            "{line}"
        );
    }
}

#[test]
fn lenient_fields_default_and_unknown_items_keep_their_type() {
    let started = json!({
        "method": "item/started",
        "params": {
            "item": { "type": "commandExecution", "id": "exec-1", "command": null, "status": "queued", "extra": 1 },
            "threadId": THREAD, "turnId": "turn-1", "startedAtMs": 5,
        },
    });
    let Ok(Inbound::Notification(Notification::ItemStarted(item))) = decode_value(started) else {
        panic!("an item");
    };
    let ThreadItem::CommandExecution(command) = &item.item else {
        panic!("a command: {item:?}");
    };
    assert_eq!(
        (command.command.as_str(), command.aggregated_output.as_ref()),
        ("", None)
    );
    assert_eq!(item.at_ms, Some(5));

    let plan = json!({ "type": "plan", "id": "plan-1", "text": "1." });
    let item: ThreadItem = serde_json::from_value(plan).unwrap();
    assert_eq!(
        item,
        ThreadItem::Unknown {
            kind: "plan".to_owned(),
            id: "plan-1".to_owned()
        }
    );
}

#[test]
fn results_read_as_recorded() {
    let handshake = "01_handshake.jsonl";
    let init: InitializeResult = response(handshake, 1).result().unwrap();
    assert_eq!(init.version(), Some("0.154.0"));
    assert_eq!(init.codex_home, "/home/user/.codex");

    let account: AccountRead = response(handshake, 2).result().unwrap();
    assert_eq!(
        account,
        AccountRead {
            account: Some(Account::Chatgpt {
                plan_type: Some("free".to_owned())
            }),
            requires_openai_auth: true,
        }
    );

    let models: Page<Model> = response(handshake, 3).result().unwrap();
    let default = models.data.iter().find(|model| model.is_default).unwrap();
    assert_eq!(default.model, "gpt-5.6-terra");
    assert_eq!(default.input_modalities, ["text", "image"]);
    assert_eq!(default.default_reasoning_effort.as_deref(), Some("medium"));

    let skills: SkillsList = response(handshake, 4).result().unwrap();
    assert_eq!(skills.data[0].skills[0].name, "brandkit");

    let servers: Page<McpServerStatus> = response(handshake, 6).result().unwrap();
    assert!(servers.data[0].tools.contains_key("fork_repository"));

    let session: ThreadSession = response("02_turn_approvals_accept.jsonl", 8).result().unwrap();
    assert_eq!(session.thread.id, THREAD);
    assert_eq!(session.thread.status, ThreadStatus::Idle);
    assert_eq!(session.runtime_workspace_roots, [PROJECT, "/work/extra-root"]);
    assert_eq!(session.approval_policy, Some(json!("untrusted")));

    let turn: TurnStarted = response("02_turn_approvals_accept.jsonl", 9).result().unwrap();
    assert_eq!(turn.turn.status, TurnStatus::InProgress);

    let fork: ThreadSession = response("07_fork_list.jsonl", 19).result().unwrap();
    assert_eq!(fork.thread.forked_from_id.as_deref(), Some(THREAD));
    assert_eq!(fork.thread.name.as_deref(), Some("S0 recording"));

    let threads: Page<Thread> = response("07_fork_list.jsonl", 21).result().unwrap();
    assert_eq!(threads.data[0].id, THREAD);
    assert!(threads.data.iter().all(|thread| !thread.preview.is_empty()));

    let items: Page<ItemEntry> = response("07_fork_list.jsonl", 23).result().unwrap();
    let kinds: Vec<&str> = items
        .data
        .iter()
        .map(|entry| match entry.item {
            ThreadItem::UserMessage(_) => "user",
            ThreadItem::AgentMessage(_) => "agent",
            ThreadItem::CommandExecution(_) => "command",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["user", "agent", "command"]);

    let turns: Page<harness_business::codex_rpc::wire::items::Turn> =
        response("02_turn_approvals_accept.jsonl", 10).result().unwrap();
    assert_eq!(turns.data[0].duration_ms, Some(13204));
    assert!(turns.next_cursor.is_none() && turns.backwards_cursor.is_some());
}

#[test]
fn requests_are_written_as_recorded() {
    let thread = || THREAD.to_owned();
    let sent = |file: &str, method: &str| frame(file, Dir::Out, method, 0);
    let id = |frame: &Value| serde_json::from_value::<RequestId>(frame["id"].clone()).unwrap();

    let start = Request::ThreadStart(ThreadStart {
        settings: ThreadSettings {
            cwd: Some(PROJECT.to_owned()),
            model: Some("gpt-5.6-terra".to_owned()),
            approval_policy: Some(ApprovalPolicy::Untrusted),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            approvals_reviewer: None,
            config: None,
        },
        ..ThreadStart::default()
    });
    let prompt = "Create a file hello.txt containing the single word hi, then run the shell command `ls -1` and tell me the output. Be brief.";
    let turn = Request::TurnStart(Box::new(TurnStart {
        thread_id: thread(),
        input: vec![UserInput::text(prompt)],
        ..TurnStart::default()
    }));
    let steer = Request::TurnSteer(TurnSteer {
        thread_id: thread(),
        expected_turn_id: "01a164a0-96a7-7243-8b57-70d95d87c471".to_owned(),
        client_user_message_id: None,
        input: vec![UserInput::text("Also mention the word STEERED in your answer.")],
    });
    let fork = Request::ThreadFork(ThreadFork {
        thread_id: thread(),
        last_turn_id: Some("01a164a0-4c86-7902-8de6-44d0f7bc8ac7".to_owned()),
        exclude_turns: true,
        ..ThreadFork::default()
    });
    let list = Request::ThreadList(ThreadList {
        cwd: Some(PROJECT.to_owned()),
        limit: Some(10),
        ..ThreadList::default()
    });

    let cases = [
        (start, sent("02_turn_approvals_accept.jsonl", "thread/start")),
        (turn, sent("02_turn_approvals_accept.jsonl", "turn/start")),
        (steer, sent("04_steer.jsonl", "turn/steer")),
        (
            Request::TurnInterrupt {
                thread_id: thread(),
                turn_id: "01a164a0-cb10-7c53-8db1-eb2cb396fb41".to_owned(),
            },
            sent("05_interrupt.jsonl", "turn/interrupt"),
        ),
        (
            Request::ThreadNameSet {
                thread_id: thread(),
                name: "S0 recording".to_owned(),
            },
            sent("06_ops.jsonl", "thread/name/set"),
        ),
        (
            Request::ThreadShellCommand {
                thread_id: thread(),
                command: "echo from-shell".to_owned(),
            },
            sent("06_ops.jsonl", "thread/shellCommand"),
        ),
        (
            Request::ThreadCompactStart { thread_id: thread() },
            sent("06_ops.jsonl", "thread/compact/start"),
        ),
        (fork, sent("07_fork_list.jsonl", "thread/fork")),
        (list, sent("07_fork_list.jsonl", "thread/list")),
        (
            Request::ThreadItemsList(Paged {
                thread_id: thread(),
                ..Paged::default()
            }),
            sent("07_fork_list.jsonl", "thread/items/list"),
        ),
        (
            Request::ThreadTurnsList(Paged {
                thread_id: thread(),
                sort_direction: Some("asc".to_owned()),
                ..Paged::default()
            }),
            sent("02_turn_approvals_accept.jsonl", "thread/turns/list"),
        ),
        (Request::AccountRead, sent("01_handshake.jsonl", "account/read")),
        (
            Request::SkillsList {
                cwds: vec![PROJECT.to_owned()],
            },
            sent("01_handshake.jsonl", "skills/list"),
        ),
    ];

    for (request, recorded) in cases {
        assert_eq!(request.frame(&id(&recorded)), recorded, "{}", request.method());
    }

    let init = Request::Initialize(Initialize::desktop("0.0.0-s0")).frame(&RequestId::Number(1));
    let recorded = sent("01_handshake.jsonl", "initialize");
    assert_eq!(init["params"]["clientInfo"], recorded["params"]["clientInfo"]);
    assert_eq!(init["params"]["capabilities"]["experimentalApi"], true);
    assert!(
        init["params"]["capabilities"]["optOutNotificationMethods"]
            .as_array()
            .unwrap()
            .contains(&json!("mcpServer/startupStatus/updated"))
    );
}

#[test]
fn answers_are_written_as_recorded() {
    let answered = |file: &str, id: i64| {
        frames(file, Dir::Out)
            .into_iter()
            .find(|frame| frame["id"] == id && frame.get("method").is_none())
            .unwrap()
    };

    assert_eq!(initialized(), frame("01_handshake.jsonl", Dir::Out, "initialized", 0));
    assert_eq!(
        command_decision(&RequestId::Number(0), Decision::Accept),
        answered("02_turn_approvals_accept.jsonl", 0)
    );
    assert_eq!(
        file_change_decision(&RequestId::Number(1), Decision::Accept),
        answered("02_turn_approvals_accept.jsonl", 1)
    );
    assert_eq!(
        command_decision(&RequestId::Number(3), Decision::Decline),
        answered("03_turn_decline.jsonl", 3)
    );
}
