//! What Codex asks the user (`codex_rpc::mapper::Ask`) from crafted requests the recordings do not
//! show (questions exist only in Plan mode, no MCP server elicited), the answers to them, and the
//! requests the app refuses.

use std::collections::BTreeMap;

use harness_business::codex_rpc::mapper::{Ask, AskKind, CodexMapper, Hint, Mapped};
use harness_business::codex_rpc::wire::answers::{
    Decision, ElicitationAction, GrantScope, elicitation_answer, file_change_decision, permissions_deny,
    permissions_grant, refusal, user_input_answers,
};
use harness_business::codex_rpc::wire::{RequestId, decode_value};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use crate::codex_recorded::PROJECT;

fn hints(request: Value) -> Vec<Hint> {
    let mut mapper = CodexMapper::new(std::path::Path::new(PROJECT));
    let frame = decode_value(request).expect("a request");

    mapper
        .on_frame(&frame)
        .into_iter()
        .filter_map(|mapped| match mapped {
            Mapped::Hint(hint) => Some(hint),
            Mapped::Event(_) => None,
        })
        .collect()
}

fn ask(request: Value) -> Ask {
    match hints(request).as_slice() {
        [Hint::Ask(ask)] => ask.clone(),
        other => panic!("one ask: {other:?}"),
    }
}

fn request(id: Value, method: &str, params: Value) -> Value {
    json!({ "id": id, "method": method, "params": params })
}

#[test]
fn questions_are_asked_and_answered_by_question_id() {
    let params = json!({
        "threadId": "thread-1", "turnId": "turn-1", "itemId": "call-1", "isBlocking": true, "autoResolutionMs": null,
        "questions": [{
            "id": "colour", "header": "Colour", "question": "Your favourite colour?", "isOther": true, "isSecret": false,
            "options": [{ "label": "red", "description": "warm" }, { "label": "blue", "description": "cool" }],
        }, {
            "id": "token", "header": "Token", "question": "Paste it", "isOther": false, "isSecret": true, "options": null,
        }],
    });
    let ask = ask(request(json!(7), "item/tool/requestUserInput", params));

    let AskKind::Questions(questions) = &ask.kind else {
        panic!("questions: {ask:?}");
    };
    assert_eq!(ask.request_id, RequestId::Number(7));
    assert_eq!(ask.item_id(), Some("call-1"));
    assert!(questions.is_blocking);

    let shown: Vec<(&str, Vec<&str>, bool, bool)> = questions
        .questions
        .iter()
        .map(|question| {
            let labels = question.options.iter().map(|option| option.label.as_str()).collect();
            (
                question.question.as_str(),
                labels,
                question.is_other,
                question.is_secret,
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("Your favourite colour?", vec!["red", "blue"], true, false),
            ("Paste it", vec![], false, true)
        ]
    );

    let answers = BTreeMap::from([("colour".to_owned(), vec!["blue".to_owned()])]);
    assert_eq!(
        user_input_answers(&ask.request_id, &answers),
        json!({ "id": 7, "result": { "answers": { "colour": { "answers": ["blue"] } } } })
    );
}

#[test]
fn an_elicitation_is_a_form_or_a_url() {
    let form = ask(request(
        json!("elicit-1"),
        "mcpServer/elicitation/request",
        json!({
            "threadId": "thread-1", "turnId": null, "serverName": "docs", "mode": "form", "_meta": null,
            "message": "Which index?",
            "requestedSchema": { "type": "object", "properties": { "index": { "type": "string" } } },
        }),
    ));
    let AskKind::Elicitation(elicitation) = &form.kind else {
        panic!("an elicitation: {form:?}");
    };
    assert!(!elicitation.is_url());
    assert_eq!(elicitation.server_name, "docs");
    assert_eq!(
        elicitation.requested_schema.as_ref().unwrap()["properties"]["index"]["type"],
        "string"
    );
    assert_eq!(form.item_id(), None);
    assert_eq!(
        elicitation_answer(
            &form.request_id,
            ElicitationAction::Accept,
            Some(json!({ "index": "main" }))
        ),
        json!({ "id": "elicit-1", "result": { "action": "accept", "content": { "index": "main" }, "_meta": null } })
    );

    let url = ask(request(
        json!(8),
        "mcpServer/elicitation/request",
        json!({
            "threadId": "thread-1", "turnId": "turn-1", "serverName": "auth", "mode": "url", "_meta": null,
            "message": "Sign in", "url": "https://example.invalid/login", "elicitationId": "e-1",
        }),
    ));
    let AskKind::Elicitation(elicitation) = &url.kind else {
        panic!("an elicitation: {url:?}");
    };
    assert!(elicitation.is_url());
    assert_eq!(elicitation.url.as_deref(), Some("https://example.invalid/login"));
    assert_eq!(
        elicitation_answer(&url.request_id, ElicitationAction::Decline, None),
        json!({ "id": 8, "result": { "action": "decline", "content": null, "_meta": null } })
    );
}

#[test]
fn extra_permissions_are_granted_as_asked_or_not_at_all() {
    let ask = ask(request(
        json!("perm-1"),
        "item/permissions/requestApproval",
        json!({
            "threadId": "thread-1", "turnId": "turn-1", "itemId": "exec-1", "environmentId": null, "startedAtMs": 1,
            "cwd": PROJECT, "reason": "needs the network",
            "permissions": { "network": { "enabled": true }, "fileSystem": { "read": null, "write": ["/work/out"] } },
        }),
    ));
    let AskKind::Permissions(approval) = &ask.kind else {
        panic!("permissions: {ask:?}");
    };
    assert!(approval.network());
    assert_eq!(approval.write_paths(), ["/work/out"]);
    assert!(approval.read_paths().is_empty());

    let shown = approval.shown_profile().expect("a profile the dialog shows");
    assert_eq!(
        permissions_grant(&ask.request_id, &shown, GrantScope::Session),
        json!({
            "id": "perm-1",
            "result": {
                "permissions": { "network": { "enabled": true }, "fileSystem": { "write": ["/work/out"] } },
                "scope": "session",
            },
        })
    );
    assert_eq!(
        permissions_deny(&ask.request_id),
        json!({ "id": "perm-1", "result": { "permissions": {}, "scope": "turn" } })
    );
}

#[test]
fn permissions_a_dialog_cannot_show_have_no_profile_to_grant() {
    let asked = |permissions: serde_json::Value| {
        let ask = ask(request(
            json!(5),
            "item/permissions/requestApproval",
            json!({ "threadId": "thread-1", "turnId": "turn-1", "itemId": "exec-1", "cwd": PROJECT, "permissions": permissions }),
        ));
        let AskKind::Permissions(approval) = ask.kind else {
            panic!("permissions: {ask:?}");
        };
        approval.shown_profile()
    };
    let root = json!({ "path": { "type": "special", "value": { "kind": "root" } }, "access": "write" });

    assert_eq!(
        asked(json!({ "fileSystem": { "read": null, "write": null, "entries": [root] } })),
        None
    );
    assert_eq!(
        asked(json!({ "network": { "enabled": true }, "macos": { "automation": true } })),
        None
    );
    assert_eq!(
        asked(json!({ "network": { "enabled": true, "domains": ["evil.example"] } })),
        None
    );
    assert_eq!(
        asked(json!({ "network": null, "fileSystem": { "read": ["/work/in"], "entries": [] } })),
        Some(json!({ "fileSystem": { "read": ["/work/in"] } }))
    );
}

#[test]
fn approval_answers_keep_the_request_id_as_sent() {
    let id = RequestId::Text("approval-9".to_owned());

    assert_eq!(
        file_change_decision(&id, Decision::AcceptForSession),
        json!({ "id": "approval-9", "result": { "decision": "acceptForSession" } })
    );
}

#[test]
fn requests_the_app_does_not_handle_are_refused_with_their_id() {
    let refused = [
        request(
            json!(11),
            "item/tool/call",
            json!({ "threadId": "t", "turnId": "u", "callId": "c", "tool": "x", "arguments": {} }),
        ),
        request(
            json!(12),
            "account/chatgptAuthTokens/refresh",
            json!({ "reason": "unauthorized" }),
        ),
        request(json!(13), "attestation/generate", json!({})),
        request(
            json!(14),
            "item/commandExecution/requestApproval",
            json!({ "itemId": 5 }),
        ),
    ];

    for frame in refused {
        let method = frame["method"].as_str().unwrap().to_owned();
        let id = frame["id"].clone();

        let hints = hints(frame);
        let [Hint::Unsupported(request)] = hints.as_slice() else {
            panic!("{method} is refused");
        };
        assert_eq!(
            refusal(request),
            json!({ "id": id, "error": { "code": -32601, "message": format!("the desktop app does not handle {method} requests") } })
        );
    }
}
