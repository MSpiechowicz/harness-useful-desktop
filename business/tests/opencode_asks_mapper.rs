//! OpenCode's permission asks and questions as chat dialogs (`Mapper`), with the recorded shapes of
//! `fixtures/opencode/server/global-event.sse`, and the replies the answers make.

mod opencode_mapper_support;

use harness_business::model::{AgentEvent, UiOutcome, UiRequestKind, UiResponse};
use harness_business::opencode_server::Mapper;
use harness_business::opencode_server::mapper::{Answer, PERMISSION_OPTIONS, Reply, ReplyKind};
use opencode_mapper_support::{SESSION, events, feed, mapper, status};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CHILD: &str = "ses_eaa55fce6ffeMnl5ZAKxvDBT61";

fn bash_ask(id: &str, session: &str) -> Value {
    json!({ "type": "permission.asked", "properties": { "id": id, "sessionID": session,
        "permission": "bash", "patterns": ["echo recorded"], "metadata": { "command": "echo recorded" },
        "always": ["echo *"], "tool": { "messageID": "msg_155a9eeb80018zbVAUXYL6o8VQ", "callID": "call_1" } } })
}

fn edit_ask(id: &str) -> Value {
    json!({ "type": "permission.asked", "properties": { "id": id, "sessionID": SESSION,
        "permission": "edit", "patterns": ["note.txt"],
        "metadata": { "filepath": "/work/project/note.txt", "diff": "+hello" }, "always": ["*"] } })
}

fn replied(kind: &str, id: &str) -> Value {
    json!({ "type": kind, "properties": { "sessionID": SESSION, "requestID": id, "reply": "reject" } })
}

fn question(id: &str, questions: Value) -> Value {
    json!({ "type": "question.asked", "properties": { "id": id, "sessionID": SESSION, "questions": questions } })
}

fn value(id: &str, value: &str) -> UiResponse {
    UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Value {
            value: value.to_owned(),
        },
    }
}

fn cancelled(id: &str) -> UiResponse {
    UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Cancelled,
    }
}

fn sent(mapper: &mut Mapper, response: &UiResponse) -> Reply {
    match mapper.answer(response).expect("an open dialog") {
        Answer::Send(reply) => reply,
        Answer::Next(next) => panic!("expected a reply, got {next:?}"),
    }
}

fn select(event: &AgentEvent) -> (String, String, Vec<String>, Vec<Option<String>>) {
    match event {
        AgentEvent::UiRequest { request } => match &request.kind {
            UiRequestKind::Select {
                title,
                options,
                details,
                ..
            } => (request.id.clone(), title.clone(), options.clone(), details.clone()),
            other => panic!("not a select: {other:?}"),
        },
        other => panic!("not a dialog: {other:?}"),
    }
}

fn input(event: &AgentEvent) -> (String, String, Option<String>) {
    match event {
        AgentEvent::UiRequest { request } => match &request.kind {
            UiRequestKind::Input { title, placeholder, .. } => (request.id.clone(), title.clone(), placeholder.clone()),
            other => panic!("not an input: {other:?}"),
        },
        other => panic!("not a dialog: {other:?}"),
    }
}

fn cancel(id: &str) -> AgentEvent {
    AgentEvent::UiCancel {
        request_id: id.to_owned(),
    }
}

#[test]
fn a_permission_ask_is_an_approval_with_three_answers() {
    let mut mapper = mapper();
    let out = events(feed(&mut mapper, &[bash_ask("per_1", SESSION), edit_ask("per_2")]));

    let (id, title, options, details) = select(&out[0]);
    assert_eq!(id, "per_1");
    assert_eq!(title, "Allow bash: echo recorded?");
    assert_eq!(options, PERMISSION_OPTIONS.map(str::to_owned));
    assert_eq!(
        details,
        [None, Some("Also allows echo * until this chat ends".to_owned()), None]
    );

    let (_, title, _, details) = select(&out[1]);
    assert_eq!(
        title, "Allow edit: note.txt?",
        "a path relative to the session's directory"
    );
    assert_eq!(
        details[1].as_deref(),
        Some("Allows every edit call until this chat ends")
    );

    let once = sent(&mut mapper, &value("per_1", "Allow once"));
    assert_eq!(
        once,
        Reply {
            session: SESSION.to_owned(),
            kind: ReplyKind::Permission {
                id: "per_1".to_owned(),
                reply: "once"
            }
        }
    );
    assert!(mapper.answer(&value("per_1", "Allow once")).is_err(), "answered once");

    let dismissed = sent(&mut mapper, &cancelled("per_2"));
    assert!(matches!(dismissed.kind, ReplyKind::Permission { reply: "reject", .. }));
}

#[test]
fn asks_answered_elsewhere_or_left_by_the_turn_close_their_dialogs() {
    let mut mapper = mapper();
    let asked = feed(
        &mut mapper,
        &[
            status("busy"),
            bash_ask("per_1", SESSION),
            edit_ask("per_2"),
            bash_ask("per_3", SESSION),
        ],
    );
    assert_eq!(asked.len(), 4);

    let always = sent(&mut mapper, &value("per_1", "Always allow"));
    assert!(matches!(always.kind, ReplyKind::Permission { reply: "always", .. }));

    let cascade = events(feed(
        &mut mapper,
        &[
            replied("permission.replied", "per_1"),
            replied("permission.replied", "per_2"),
        ],
    ));
    assert_eq!(cascade, [cancel("per_2")], "the answered ask needs no cancel");

    let ended = events(feed(&mut mapper, &[status("idle")]));
    assert_eq!(
        ended[0],
        cancel("per_3"),
        "OpenCode drops open asks when the turn stops"
    );
}

#[test]
fn a_subagent_asks_with_its_own_directory_and_other_sessions_are_ignored() {
    let mut mapper = mapper();
    let created = json!({ "type": "session.created", "properties": { "sessionID": CHILD,
        "info": { "id": CHILD, "parentID": SESSION, "directory": "/work/project/sub" } } });

    let out = events(feed(
        &mut mapper,
        &[bash_ask("per_0", "ses_unknown"), created, bash_ask("per_1", CHILD)],
    ));

    assert_eq!(out.len(), 1);
    let (id, title, ..) = select(&out[0]);
    assert_eq!(
        (id.as_str(), title.as_str()),
        ("per_1", "Allow bash for a subagent: echo recorded?")
    );
    assert_eq!(mapper.subagent_directory(CHILD), Some("/work/project/sub"));
    assert_eq!(mapper.subagent_directory(SESSION), None);

    let reply = sent(&mut mapper, &value("per_1", "Reject"));
    assert_eq!(reply.session, CHILD);

    let cancelled = events(feed(&mut mapper, &[bash_ask("per_2", CHILD)]));
    assert_eq!(cancelled.len(), 1);
    assert_eq!(mapper.cancel_asks(), [cancel("per_2")]);
}

#[test]
fn a_single_choice_question_is_a_select_that_never_reads_as_an_approval() {
    let mut mapper = mapper();
    let recorded = json!([{ "question": "Pick one", "header": "Pick", "options": [
        { "label": "Alpha", "description": "first" }, { "label": "Beta", "description": "second" }] }]);
    let yes_no = json!([{ "question": "Commit the change?", "header": "Commit", "options": [
        { "label": "Yes", "description": "" }, { "label": "No", "description": "keep it staged" }] }]);

    let out = events(feed(
        &mut mapper,
        &[question("que_1", recorded), question("que_2", yes_no)],
    ));

    let (id, title, options, details) = select(&out[0]);
    assert_eq!((id.as_str(), title.as_str()), ("que_1", "Pick one"));
    assert_eq!(options, ["Alpha", "Beta"]);
    assert_eq!(details, [Some("first".to_owned()), Some("second".to_owned())]);

    let (_, _, options, _) = select(&out[1]);
    assert_eq!(
        options,
        ["1. Yes", "2. No"],
        "numbered, so the UI does not show an approval card"
    );

    let beta = sent(&mut mapper, &value("que_1", "Beta"));
    assert_eq!(
        beta.kind,
        ReplyKind::Question {
            id: "que_1".to_owned(),
            answers: vec![vec!["Beta".to_owned()]]
        }
    );

    let no = sent(&mut mapper, &value("que_2", "2. No"));
    assert!(matches!(no.kind, ReplyKind::Question { answers, .. } if answers == [["No"]]));
}

#[test]
fn several_or_open_questions_are_inputs_answered_together() {
    let mut mapper = mapper();
    let questions = json!([
        { "question": "Which tools?", "header": "Tools", "multiple": true, "options": [
            { "label": "Alpha", "description": "" }, { "label": "Beta", "description": "" }] },
        { "question": "Anything else?", "header": "More", "options": [] },
    ]);

    let out = events(feed(&mut mapper, &[question("que_1", questions)]));
    let (id, title, placeholder) = input(&out[0]);
    assert_eq!(id, "que_1");
    assert_eq!(
        title,
        "Question 1 of 2: Which tools? (any of Alpha, Beta, separated by commas)"
    );
    assert_eq!(placeholder.as_deref(), Some("Alpha, Beta"));

    let Answer::Next(next) = mapper.answer(&value("que_1", "alpha, Gamma")).expect("open") else {
        panic!("the second question comes next");
    };
    let (id, title, placeholder) = input(&next[0]);
    assert_eq!(
        (id.as_str(), title.as_str(), placeholder),
        ("que_1:2", "Question 2 of 2: Anything else?", None)
    );

    let reply = sent(&mut mapper, &value("que_1:2", "  no  "));
    assert_eq!(
        reply.kind,
        ReplyKind::Question {
            id: "que_1".to_owned(),
            answers: vec![vec!["Alpha".to_owned(), "Gamma".to_owned()], vec!["no".to_owned()]]
        }
    );
}

#[test]
fn a_dismissed_question_is_rejected_and_one_settled_elsewhere_closes() {
    let mut mapper = mapper();
    let one = json!([{ "question": "Pick one", "header": "Pick", "options": [] }]);

    feed(&mut mapper, &[question("que_1", one.clone()), question("que_2", one)]);

    let rejected = sent(&mut mapper, &cancelled("que_1"));
    assert_eq!(rejected.kind, ReplyKind::QuestionReject { id: "que_1".to_owned() });

    let closed = events(feed(&mut mapper, &[replied("question.rejected", "que_2")]));
    assert_eq!(closed, [cancel("que_2")]);
}
