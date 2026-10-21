//! The OpenCode 2 mapper's asks: permission requests as approval cards and forms (the `question`
//! tool's among them) as question dialogs, the answers sent back, and subagent asks. Against the
//! recorded runs (`fixtures/opencode2/recorded`) and crafted events.

mod opencode2_mapper_support;
mod opencode2_recorded;

use std::collections::BTreeMap;

use harness_business::model::{AgentEvent, UiOutcome, UiPurpose, UiRequestKind, UiResponse};
use harness_business::opencode_server_v2::mapper::{Answer, Mapper, PERMISSION_OPTIONS, Reply, ReplyKind};
use harness_business::opencode_server_v2::wire::{Decision, FormValue};
use opencode2_mapper_support::{
    OWN_ANSWER, SESSION, dismissed, event, events, feed, mapper, ours, replay, request, requests, send, value,
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

fn input_title(kind: &UiRequestKind) -> &str {
    match kind {
        UiRequestKind::Input { title, .. } => title,
        other => panic!("not an input: {other:?}"),
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
            Some("Allows every edit call in this project, in later chats too (OpenCode saves the rule)".to_owned()),
            None
        ]
    );

    let shell = &dialogs[1];
    assert_eq!(shell.tool_call_id.as_deref(), Some("call_01a15d7c540c72e7b51933da"));
    let (title, _, details) = select(&shell.kind);
    assert_eq!(title, "Allow shell: ls?");
    assert_eq!(
        details[1].as_deref(),
        Some("Also allows ls * in this project, in later chats too (OpenCode saves the rule)")
    );

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
        Some(
            "Also allows ls *, echo *, cat *, echo *, ls * in this project, in later chats too (OpenCode saves the rule)"
        )
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
    let (title, options, details) = select(&question.kind);
    assert_eq!(title, "Which color do you prefer, red or blue?");
    assert_eq!(options, ["Red", "Blue", OWN_ANSWER]);
    assert_eq!(
        details,
        [
            Some("Pick red as your preferred color.".to_owned()),
            Some("Pick blue as your preferred color.".to_owned()),
            None
        ]
    );

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

    let reply = send(mapper.answer(&dismissed("per_1")).expect("answer"));
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
    let reply = send(mapper.answer(&confirmed).expect("answer"));
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
    send(mapper.answer(&value("per_1", "Allow once")).expect("answer"));

    let again = mapper.reopen("per_1");
    assert_eq!(again.len(), 1);
    assert_eq!(request(&again[0]).id, "per_1");
    send(mapper.answer(&value("per_1", "Allow once")).expect("answered again"));

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

    let reply = send(mapper.answer(&value("per_s", "Allow once")).expect("answer"));
    assert_eq!(reply.session, "ses_grandchild");
}

/// A `form.created` of [`SESSION`] with `fields`.
fn form(id: &str, metadata: Value, fields: Value) -> harness_business::opencode_server_v2::events::Event {
    event(
        "form.created",
        json!({ "form": { "id": id, "sessionID": SESSION, "title": "Setup", "metadata": metadata, "fields": fields } }),
    )
}

#[test]
fn a_form_asks_each_field_in_turn_and_sends_the_answers_together() {
    let mut mapper = mapper();
    let fields = json!([
        { "key": "color", "type": "string", "title": "Color", "description": "Pick one",
          "options": [{ "value": "r", "label": "Red" }, { "value": "g", "label": "Green" }] },
        { "key": "tags", "type": "multiselect", "title": "Tags",
          "options": [{ "value": "a", "label": "Alpha" }, { "value": "b", "label": "Beta" }] },
        { "key": "ok", "type": "boolean", "title": "Proceed?" },
        { "key": "count", "type": "integer", "title": "How many?" },
        { "key": "note", "type": "string", "title": "Anything else?" }
    ]);

    let out = events(feed(&mut mapper, &[form("frm_1", json!({}), fields)]));
    let first = request(&out[0]);
    assert_eq!(first.id, "frm_1");
    let (title, options, _) = select(&first.kind);
    assert_eq!(title, "Question 1 of 5: Color");
    assert_eq!(options, ["Red", "Green"], "no typed answer without custom");

    let next = |answer: Answer| match answer {
        Answer::Next(events) => request(&events[0]).clone(),
        Answer::Send(reply) => panic!("sent early: {reply:?}"),
    };

    let tags = next(mapper.answer(&value("frm_1", "Red")).expect("color"));
    assert_eq!(tags.id, "frm_1:2");
    assert_eq!(
        input_title(&tags.kind),
        "Question 2 of 5: Tags (any of Alpha, Beta, separated by commas)"
    );

    let ok = next(mapper.answer(&value("frm_1:2", "beta, gamma")).expect("tags"));
    let (_, options, _) = select(&ok.kind);
    assert_eq!(options, ["Yes", "No"]);

    let count = next(mapper.answer(&value("frm_1:3", "No")).expect("ok"));
    assert_eq!(input_title(&count.kind), "Question 4 of 5: How many?");
    assert!(
        mapper.answer(&value("frm_1:4", "2.5")).is_err(),
        "an integer field takes whole numbers"
    );
    let note = next(mapper.answer(&value("frm_1:4", " 3 ")).expect("count"));
    assert_eq!(input_title(&note.kind), "Question 5 of 5: Anything else?");

    let reply = send(mapper.answer(&value("frm_1:5", "   ")).expect("note"));
    let ReplyKind::Form { id, body } = reply.kind else {
        panic!("a form reply");
    };
    assert_eq!(id, "frm_1");
    assert_eq!(
        serde_json::to_value(&body).expect("body"),
        json!({ "answer": { "color": "r", "count": 3.0, "ok": false, "tags": ["b", "gamma"] } }),
        "a blank answer leaves its field out"
    );
}

#[test]
fn a_dismissed_form_is_cancelled_and_one_answered_elsewhere_closes() {
    let mut mapper = mapper();
    let fields = json!([{ "key": "q0", "type": "string", "title": "Name?" }]);

    feed(&mut mapper, &[form("frm_1", json!({}), fields.clone())]);
    let reply = send(mapper.answer(&dismissed("frm_1")).expect("dismissed"));
    assert_eq!(reply.kind, ReplyKind::FormCancel { id: "frm_1".to_owned() });

    feed(&mut mapper, &[form("frm_2", json!({}), fields)]);
    let out = events(feed(
        &mut mapper,
        &[event("form.cancelled", json!({ "id": "frm_2", "sessionID": SESSION }))],
    ));
    assert_eq!(
        out,
        [AgentEvent::UiCancel {
            request_id: "frm_2".to_owned()
        }]
    );
}

#[test]
fn a_form_the_chat_cannot_ask_is_not_shown() {
    let mut mapper = mapper();
    let fields = json!([
        { "key": "q0", "type": "string", "title": "Name?" },
        { "key": "login", "type": "external", "url": "https://example.com/login" }
    ]);

    assert!(feed(&mut mapper, &[form("frm_1", json!({}), fields)]).is_empty());

    let global = event(
        "form.created",
        json!({ "form": { "id": "frm_g", "sessionID": "global", "title": "MCP",
            "fields": [{ "key": "q0", "type": "string" }] } }),
    );
    assert!(
        feed(&mut mapper, &[global]).is_empty(),
        "forms no session of the chat owns"
    );
}

#[test]
fn a_question_form_asks_its_question_text_and_starts_over_when_reopened() {
    let mut mapper: Mapper = mapper();
    let fields = json!([
        { "key": "q0", "type": "string", "title": "Color", "description": "Which color?", "custom": true,
          "options": [{ "value": "Red", "label": "Red" }] },
        { "key": "q1", "type": "multiselect", "title": "Sizes", "description": "Which sizes?", "custom": true,
          "options": [{ "value": "S", "label": "S" }] }
    ]);

    let out = events(feed(
        &mut mapper,
        &[form("frm_q", json!({ "kind": "question" }), fields)],
    ));
    let (title, options, _) = select(&request(&out[0]).kind);
    assert_eq!(title, "Question 1 of 2: Which color?");
    assert_eq!(options, ["Red", OWN_ANSWER]);

    let typed = match mapper.answer(&value("frm_q", OWN_ANSWER)).expect("own") {
        Answer::Next(events) => request(&events[0]).clone(),
        Answer::Send(_) => panic!("the typed answer comes next"),
    };
    assert_eq!(typed.id, "frm_q:1:own");
    assert_eq!(input_title(&typed.kind), "Question 1 of 2: Which color?");

    let sizes = match mapper.answer(&value("frm_q:1:own", "teal")).expect("typed") {
        Answer::Next(events) => request(&events[0]).clone(),
        Answer::Send(_) => panic!("the second question comes next"),
    };
    assert_eq!(
        input_title(&sizes.kind),
        "Question 2 of 2: Which sizes? (any of S, separated by commas)"
    );

    let reply = send(mapper.answer(&value("frm_q:2", "s, xl")).expect("sizes"));
    let ReplyKind::Form { body, .. } = &reply.kind else {
        panic!("a form reply");
    };
    assert_eq!(
        serde_json::to_value(body).expect("body"),
        json!({ "answer": { "q0": "teal", "q1": ["S", "xl"] } })
    );

    // The reply did not reach OpenCode: the form is asked again from its first question.
    let again = mapper.reopen("frm_q");
    assert_eq!(request(&again[0]).id, "frm_q");
    assert_eq!(select(&request(&again[0]).kind).0, "Question 1 of 2: Which color?");
}
