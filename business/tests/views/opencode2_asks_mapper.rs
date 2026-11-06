//! The OpenCode 2 mapper's asks: permission requests as approval cards and forms (the `question`
//! tool's among them) as question dialogs, the answers sent back, and subagent asks. Against the
//! recorded runs (`fixtures/opencode2/recorded`) and crafted events.

use crate::ask_support::{labels, questions_of};
use crate::opencode2_mapper_support;

use std::collections::BTreeMap;

use harness_business::model::{AgentEvent, UiOutcome, UiPurpose, UiRequestKind, UiResponse};
use harness_business::opencode_server_v2::mapper::{PERMISSION_OPTIONS, Reply, ReplyKind};
use harness_business::opencode_server_v2::wire::{Decision, FormValue};
use opencode2_mapper_support::{
    SESSION, dismissed, event, events, feed, mapper, ours, replay, request, requests, value,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const EDIT_SESSION: &str = "ses_ea283f985ffeX5BRVy9GzbD4gY";

fn select(kind: &UiRequestKind) -> (&str, Vec<String>, Vec<Option<String>>) {
    match kind {
        UiRequestKind::Select {
            title,
            options,
            details,
            ..
        } => (title.as_str(), options.clone(), details.clone()),
        other => panic!("not a select: {other:?}"),
    }
}

/// A `permission.asked` of `session`.
fn asked(id: &str, session: &str, action: &str, extra: Value) -> harness_business::opencode_server_v2::events::Event {
    let mut data = json!({ "id": id, "sessionID": session, "action": action });
    data.as_object_mut()
        .expect("object")
        .extend(extra.as_object().cloned().unwrap_or_default());

    event("permission.asked", data)
}

#[test]
fn recorded_permissions_are_approval_cards_bound_to_their_calls() {
    let replay = replay("edit_once.jsonl");
    let dialogs = requests(&replay.events());

    assert_eq!(dialogs.len(), 2);

    let edit = &dialogs[0];
    assert_eq!(edit.id, "per_15d7c3a33001lsSJ9gGeiFzCh8");
    assert_eq!(edit.purpose, Some(UiPurpose::Approval));
    assert_eq!(edit.tool_call_id.as_deref(), Some("call_01a15d7c388975ddb41ea7f1"));
    let (title, options, details) = select(&edit.kind);
    assert_eq!(title, "Allow edit: notes.txt?");
    assert_eq!(options, PERMISSION_OPTIONS.map(str::to_owned).to_vec());
    assert_eq!(
        details,
        [
            None,
            Some("Allows every edit call until this chat ends".to_owned()),
            None
        ]
    );

    let shell = &dialogs[1];
    assert_eq!(shell.tool_call_id.as_deref(), Some("call_01a15d7c540c72e7b51933da"));
    let (title, _, details) = select(&shell.kind);
    assert_eq!(title, "Allow shell: ls?");
    assert_eq!(details[1].as_deref(), Some("Also allows ls * until this chat ends"));

    let decisions: Vec<(String, ReplyKind)> = replay
        .replies
        .iter()
        .map(|reply| (reply.session.clone(), reply.kind.clone()))
        .collect();
    assert_eq!(
        decisions,
        [
            (
                EDIT_SESSION.to_owned(),
                ReplyKind::Permission {
                    id: "per_15d7c3a33001lsSJ9gGeiFzCh8".to_owned(),
                    decision: Decision::Once
                }
            ),
            (
                EDIT_SESSION.to_owned(),
                ReplyKind::Permission {
                    id: "per_15d7c55e7001c0D7XksJRFT88J".to_owned(),
                    decision: Decision::Always
                }
            )
        ]
    );
}

#[test]
fn a_shell_ask_shows_the_whole_command_of_its_call() {
    let replay = replay("edit_reject.jsonl");
    let dialogs = requests(&replay.events());

    let (title, _, details) = select(&dialogs[1].kind);
    assert_eq!(
        title,
        "Allow shell: ls -la; echo \"---\"; cat notes.txt 2>/dev/null; echo \"---\"; ls -la .opencode 2>/dev/null?"
    );
    assert_eq!(
        details[1].as_deref(),
        Some("Also allows ls *, echo *, cat *, echo *, ls * until this chat ends")
    );

    assert_eq!(
        replay.replies[0].kind,
        ReplyKind::Permission {
            id: "per_15d7c8eeb001fukvNGTNsK07QQ".to_owned(),
            decision: Decision::Reject
        }
    );
}

#[test]
fn the_recorded_question_is_a_dialog_answered_with_a_typed_answer() {
    let replay = replay("question.jsonl");
    let dialogs = requests(&replay.events());

    let question = &dialogs[0];
    assert_eq!(question.id, "frm_15d80582d001AwxHnvC4VOQ9RP");
    assert_eq!(question.purpose, Some(UiPurpose::Question));
    assert_eq!(question.tool_call_id, None);
    let [asked] = questions_of(question) else {
        panic!("one question");
    };
    assert_eq!(asked.id, "0");
    assert_eq!(asked.question, "Which color do you prefer, red or blue?");
    assert_eq!(labels(asked), ["Red", "Blue"]);
    assert_eq!(
        asked
            .options
            .iter()
            .map(|option| option.description.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("Pick red as your preferred color."),
            Some("Pick blue as your preferred color.")
        ]
    );
    assert!(asked.custom && !asked.multi);

    let answer = BTreeMap::from([("q0".to_owned(), FormValue::Text("blue".to_owned()))]);
    let ReplyKind::Form { id, body } = &replay.replies[0].kind else {
        panic!("a form reply");
    };
    assert_eq!(id, "frm_15d80582d001AwxHnvC4VOQ9RP");
    assert_eq!(body.answer, answer);
    assert_eq!(
        serde_json::to_value(body).expect("body"),
        json!({ "answer": { "q0": "blue" } })
    );
}

#[test]
fn a_dismissed_permission_rejects_and_a_wrong_option_fails() {
    let mut mapper = mapper();
    feed(
        &mut mapper,
        &[asked(
            "per_1",
            SESSION,
            "read",
            json!({ "resources": ["/work/proj/a.txt"] }),
        )],
    );

    assert!(mapper.answer(&value("per_1", "Maybe")).is_err());
    assert!(
        mapper.answer(&value("per_2", "Allow once")).is_err(),
        "not an open dialog"
    );

    let reply = mapper.answer(&dismissed("per_1")).expect("answer");
    assert_eq!(
        reply,
        Reply {
            session: SESSION.to_owned(),
            kind: ReplyKind::Permission {
                id: "per_1".to_owned(),
                decision: Decision::Reject
            }
        }
    );

    feed(&mut mapper, &[asked("per_3", SESSION, "read", json!({}))]);
    let confirmed = UiResponse {
        id: "per_3".to_owned(),
        outcome: UiOutcome::Confirmed { confirmed: true },
    };
    let reply = mapper.answer(&confirmed).expect("answer");
    assert!(matches!(
        reply.kind,
        ReplyKind::Permission {
            decision: Decision::Once,
            ..
        }
    ));
}

#[test]
fn a_permission_title_names_its_resources_relative_to_the_session() {
    let mut mapper = mapper();
    let out = events(feed(
        &mut mapper,
        &[
            asked(
                "per_1",
                SESSION,
                "read",
                json!({ "resources": ["/work/proj/src/a.rs", "/etc/hosts"] }),
            ),
            asked("per_2", SESSION, "question", json!({ "resources": ["*"] })),
        ],
    ));

    let titles: Vec<String> = requests(&out)
        .iter()
        .map(|dialog| select(&dialog.kind).0.to_owned())
        .collect();
    assert_eq!(titles, ["Allow read: src/a.rs, /etc/hosts?", "Allow question?"]);
}

#[test]
fn asks_answered_elsewhere_and_open_asks_at_the_turn_end_close() {
    let mut mapper = mapper();
    mapper.prompt_sent();
    feed(&mut mapper, &[ours("session.execution.started", json!({}))]);

    feed(
        &mut mapper,
        &[
            asked("per_1", SESSION, "edit", json!({})),
            asked("per_2", SESSION, "edit", json!({})),
            asked("per_3", SESSION, "edit", json!({})),
        ],
    );

    let replied = ours("permission.replied", json!({ "requestID": "per_1", "reply": "reject" }));
    assert_eq!(
        events(feed(&mut mapper, &[replied])),
        [AgentEvent::UiCancel {
            request_id: "per_1".to_owned()
        }]
    );

    let out = events(feed(&mut mapper, &[ours("session.execution.succeeded", json!({}))]));
    let closed: Vec<&AgentEvent> = out
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiCancel { .. }))
        .collect();
    assert_eq!(closed.len(), 2);
    assert!(mapper.reopen("per_2").is_empty(), "the turn's end dropped the ask");
}

#[test]
fn an_ask_whose_answer_did_not_arrive_opens_again() {
    let mut mapper = mapper();
    feed(
        &mut mapper,
        &[asked("per_1", SESSION, "edit", json!({ "resources": ["a.txt"] }))],
    );
    mapper.answer(&value("per_1", "Allow once")).expect("answer");

    let again = mapper.reopen("per_1");
    assert_eq!(again.len(), 1);
    assert_eq!(request(&again[0]).id, "per_1");
    mapper.answer(&value("per_1", "Allow once")).expect("answered again");

    feed(
        &mut mapper,
        &[ours(
            "permission.replied",
            json!({ "requestID": "per_1", "reply": "once" }),
        )],
    );
    assert!(mapper.reopen("per_1").is_empty());
}

#[test]
fn subagent_asks_reach_the_chat_unbound_and_reply_to_their_session() {
    let mut mapper = mapper();
    mapper.set_mode_applied(true);

    let child = json!({ "sessionID": "ses_child", "parentID": SESSION });
    let grandchild = json!({ "sessionID": "ses_grandchild", "parentID": "ses_child" });
    let created = |data: Value| {
        let envelope = json!({ "id": "evt_c", "type": "session.created", "data": data,
            "location": { "directory": "/work/proj/sub" } });
        harness_business::opencode_server_v2::events::parse(&envelope.to_string()).expect("event")
    };

    let out = events(feed(
        &mut mapper,
        &[
            created(child),
            created(grandchild),
            asked(
                "per_s",
                "ses_grandchild",
                "edit",
                json!({ "resources": ["x.txt"], "source": { "type": "tool", "messageID": "msg_x", "id": "call_x" } }),
            ),
            asked("per_t", SESSION, "subagent", json!({ "resources": ["explore"] })),
            asked("per_u", "ses_stranger", "edit", json!({})),
        ],
    ));

    let dialogs = requests(&out);
    assert_eq!(dialogs.len(), 2, "a stranger session's asks are not this chat's");

    let (title, _, _) = select(&dialogs[0].kind);
    assert_eq!(title, "Allow edit for a subagent: x.txt?");
    assert_eq!(dialogs[0].tool_call_id, None);

    let (title, _, details) = select(&dialogs[1].kind);
    assert_eq!(title, "Allow subagent: explore?");
    assert_eq!(
        details[0].as_deref(),
        Some("The subagent's own tools then follow this chat's approval mode.")
    );

    assert_eq!(mapper.subagent_directory("ses_grandchild"), Some("/work/proj/sub"));
    assert_eq!(mapper.subagent_directory(SESSION), None);

    let reply = mapper.answer(&value("per_s", "Allow once")).expect("answer");
    assert_eq!(reply.session, "ses_grandchild");
}
