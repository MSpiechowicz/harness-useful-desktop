//! An OpenCode 2 subagent's permission ask shows what it covers exactly as sent: its call is not in the transcript.

use crate::opencode2_mapper_support;

use harness_business::model::UiRequestKind;
use opencode2_mapper_support::{SESSION, event, events, feed, mapper, requests};
use serde_json::{Value, json};

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

fn asked(id: &str, session: &str, action: &str, extra: Value) -> harness_business::opencode_server_v2::events::Event {
    let mut data = json!({ "id": id, "sessionID": session, "action": action });
    data.as_object_mut()
        .expect("object")
        .extend(extra.as_object().cloned().unwrap_or_default());

    event("permission.asked", data)
}

/// The title of the ask `action` of a subagent with `extra`, in a chat whose directory is `/work/proj/sub`.
fn subagent_title(action: &str, extra: Value) -> String {
    let mut mapper = mapper();
    let envelope = json!({ "id": "evt_c", "type": "session.created",
        "data": { "sessionID": "ses_child", "parentID": SESSION },
        "location": { "directory": "/work/proj/sub" } });
    let child = harness_business::opencode_server_v2::events::parse(&envelope.to_string()).expect("event");

    let out = events(feed(&mut mapper, &[child, asked("per_s", "ses_child", action, extra)]));
    let dialogs = requests(&out);

    select(&dialogs[0].kind).0.to_owned()
}

#[test]
fn a_subagent_shell_ask_shows_its_command_as_sent() {
    for command in [
        "echo -----BEGIN X-----; touch /tmp/marker",
        "/work/proj/sub/clean.sh /",
        "/work/proj/sub/run a\\ b",
    ] {
        let title = subagent_title("shell", json!({ "resources": [command] }));

        assert!(title.contains(command), "{title}");
    }
}

#[test]
fn a_subagent_edit_ask_shows_what_it_changes() {
    let extra =
        json!({ "resources": ["notes.txt"], "metadata": { "files": [{ "file": "notes.txt", "patch": "+new line" }] } });
    let title = subagent_title("edit", extra);

    assert!(title.contains("notes.txt") && title.contains("+new line"), "{title}");
}
