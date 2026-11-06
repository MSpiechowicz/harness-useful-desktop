//! The OpenCode 2 mapper's forms (`form.created`, the `question` tool's among them) as one question
//! dialog holding all their fields, and the reply the answers make. Crafted events.

use crate::ask_support;
use crate::opencode2_mapper_support;

use ask_support::{answers, labels, pick, pick_and_type, questions_of, typed};
use harness_business::model::{AgentEvent, AskInput, UiPurpose};
use harness_business::opencode_server_v2::mapper::ReplyKind;
use opencode2_mapper_support::{SESSION, dismissed, event, events, feed, mapper, request};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A `form.created` of [`SESSION`] with `fields`.
fn form(id: &str, metadata: Value, fields: Value) -> harness_business::opencode_server_v2::events::Event {
    event(
        "form.created",
        json!({ "form": { "id": id, "sessionID": SESSION, "title": "Setup", "metadata": metadata, "fields": fields } }),
    )
}

#[test]
fn a_form_is_one_dialog_of_its_fields_and_sends_the_answers_together() {
    let mut mapper = mapper();
    let fields = json!([
        { "key": "color", "type": "string", "title": "Color", "description": "Pick one",
          "options": [{ "value": "r", "label": "Red" }, { "value": "g", "label": "Green" }] },
        { "key": "tags", "type": "multiselect", "title": "Tags", "custom": true,
          "options": [{ "value": "a", "label": "Alpha" }, { "value": "b", "label": "Beta" }] },
        { "key": "ok", "type": "boolean", "title": "Proceed?" },
        { "key": "count", "type": "integer", "title": "How many?" },
        { "key": "note", "type": "string", "title": "Anything else?" }
    ]);

    let out = events(feed(&mut mapper, &[form("frm_1", json!({}), fields)]));
    assert_eq!(out.len(), 1, "one dialog for the whole form");
    let first = request(&out[0]);
    assert_eq!(first.id, "frm_1");
    assert_eq!(first.purpose, Some(UiPurpose::Question));
    let asked = questions_of(first);

    let shown = asked
        .iter()
        .map(|question| {
            (
                question.id.as_str(),
                question.question.as_str(),
                labels(question),
                question.multi,
                question.custom,
                question.input,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        shown,
        [
            ("0", "Color", vec!["Red", "Green"], false, false, AskInput::Text),
            ("1", "Tags", vec!["Alpha", "Beta"], true, true, AskInput::Text),
            ("2", "Proceed?", vec!["Yes", "No"], false, false, AskInput::Text),
            ("3", "How many?", vec![], false, false, AskInput::Integer),
            ("4", "Anything else?", vec![], false, false, AskInput::Text),
        ],
        "no typed answer without custom, a boolean is Yes or No"
    );

    let wrong = [
        answers("frm_1", vec![pick(0, &["Red"])]),
        answers(
            "frm_1",
            vec![
                pick(0, &["Blue"]),
                pick(1, &[]),
                pick(2, &[]),
                typed(3, ""),
                typed(4, ""),
            ],
        ),
        answers(
            "frm_1",
            vec![
                pick(0, &["Red"]),
                pick(1, &[]),
                pick(2, &[]),
                typed(3, "2.5"),
                typed(4, ""),
            ],
        ),
        answers(
            "frm_1",
            vec![
                pick(1, &[]),
                pick(0, &["Red"]),
                pick(2, &[]),
                typed(3, ""),
                typed(4, ""),
            ],
        ),
    ];
    for response in &wrong {
        assert!(mapper.answer(response).is_err(), "{response:?}");
    }

    let reply = mapper
        .answer(&answers(
            "frm_1",
            vec![
                pick(0, &["Red"]),
                pick_and_type(1, &["Beta"], "gamma"),
                pick(2, &["No"]),
                typed(3, " 3 "),
                typed(4, "   "),
            ],
        ))
        .expect("the dialog stayed open after the wrong answers");
    let ReplyKind::Form { id, body } = reply.kind else {
        panic!("a form reply");
    };
    assert_eq!(id, "frm_1");
    assert_eq!(
        serde_json::to_value(&body).expect("body"),
        json!({ "answer": { "color": "r", "count": 3.0, "ok": false, "tags": ["b", "gamma"] } }),
        "options answer with their values, a blank answer leaves its field out"
    );
}

#[test]
fn two_options_with_one_label_each_answer_with_their_own_value() {
    let mut mapper = mapper();
    let fields = json!([{ "key": "act", "type": "string", "title": "Sure?", "options": [
        { "label": "Yes", "value": "keep" }, { "label": "Yes", "value": "delete" }] }]);

    let out = events(feed(&mut mapper, &[form("frm_1", json!({}), fields)]));
    let asked = questions_of(request(&out[0]));
    assert_eq!(labels(&asked[0]), ["Yes", "Yes (delete)"]);

    let reply = mapper
        .answer(&answers("frm_1", vec![pick(0, &["Yes (delete)"])]))
        .expect("an answer");
    let ReplyKind::Form { body, .. } = reply.kind else {
        panic!("a form is answered with a form reply");
    };

    assert_eq!(
        serde_json::to_value(&body).expect("a body"),
        json!({ "answer": { "act": "delete" } })
    );
}

#[test]
fn a_dismissed_form_is_cancelled_and_one_answered_elsewhere_closes() {
    let mut mapper = mapper();
    let fields = json!([{ "key": "q0", "type": "string", "title": "Name?" }]);

    feed(&mut mapper, &[form("frm_1", json!({}), fields.clone())]);
    let reply = mapper.answer(&dismissed("frm_1")).expect("dismissed");
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
        { "key": "sso", "type": "external", "url": "https://example.com/login" }
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
fn a_question_form_asks_its_question_text_and_opens_again_when_reopened() {
    let mut mapper = mapper();
    let fields = json!([
        { "key": "q0", "type": "string", "title": "Color", "description": "Which color?", "custom": true,
          "options": [{ "value": "Red", "label": "Red" }] },
        { "key": "q1", "type": "multiselect", "title": "Sizes", "description": "Which sizes?", "custom": true,
          "options": [{ "value": "S", "label": "S" }, { "value": "L, big", "label": "Large, big" }] }
    ]);

    let out = events(feed(
        &mut mapper,
        &[form("frm_q", json!({ "kind": "question" }), fields)],
    ));
    let asked = questions_of(request(&out[0]));
    assert_eq!(
        asked
            .iter()
            .map(|question| question.question.as_str())
            .collect::<Vec<_>>(),
        ["Which color?", "Which sizes?"]
    );
    assert_eq!(labels(&asked[0]), ["Red"]);
    assert!(asked.iter().all(|question| question.custom));

    let reply = mapper
        .answer(&answers(
            "frm_q",
            vec![typed(0, "  teal "), pick_and_type(1, &["S", "Large, big"], "xl")],
        ))
        .expect("answered");
    let ReplyKind::Form { body, .. } = &reply.kind else {
        panic!("a form reply");
    };
    assert_eq!(
        serde_json::to_value(body).expect("body"),
        json!({ "answer": { "q0": "teal", "q1": ["S", "L, big", "xl"] } }),
        "a label with a comma stays one answer"
    );

    // The reply did not reach OpenCode: the form is asked again, with all its questions.
    let again = mapper.reopen("frm_q");
    assert_eq!(request(&again[0]).id, "frm_q");
    assert_eq!(questions_of(request(&again[0])).len(), 2);
}
