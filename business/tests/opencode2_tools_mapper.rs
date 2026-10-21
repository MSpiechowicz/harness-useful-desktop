//! Tool cards of the OpenCode 2 mapper: the output a card shows live matches what it shows after
//! resume (files, a failed call's partial output), a running `shell` call's output streams as
//! `toolUpdate` once the engine reads its shell, a session a `subagent` call names counts only once
//! the engine confirms it, and a prompt that runs a skill names it. Also the lenient reading of
//! session messages.

mod opencode2_mapper_support;
mod opencode2_recorded;

use harness_business::model::{AgentEvent, MessageRole};
use harness_business::opencode_server_v2::events::Event;
use harness_business::opencode_server_v2::mapper::{Mapper, ShellCall, history};
use harness_business::opencode_server_v2::messages::{SessionMessage, ToolState};
use opencode2_mapper_support::{SESSION, ended, event, events, feed, mapper, ours, requests, result_text, text_of};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Starts call `call_1` of tool `tool` in a running turn.
fn running_call(tool: &str, input: Value) -> Mapper {
    let mut mapper = mapper();
    mapper.prompt_sent();

    feed(
        &mut mapper,
        &[
            ours("session.execution.started", json!({})),
            ours(
                "session.tool.input.started",
                json!({ "assistantMessageID": "msg_a", "id": "call_1", "name": tool }),
            ),
            ours(
                "session.tool.called",
                json!({ "assistantMessageID": "msg_a", "id": "call_1", "input": input }),
            ),
        ],
    );

    mapper
}

/// The live `toolEnd` result text of the event `kind` with `data` for `call_1`.
fn live_result(mapper: &mut Mapper, kind: &str, mut data: Value) -> (String, bool) {
    data["assistantMessageID"] = json!("msg_a");
    data["id"] = json!("call_1");

    events(feed(mapper, &[ours(kind, data)]))
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnd { result, is_error, .. } => Some((result_text(result), *is_error)),
            _ => None,
        })
        .expect("the call's end")
}

/// The result text of the stored call `state` after resume.
fn resumed_result(state: Value) -> (String, bool) {
    let message = SessionMessage::parse(&json!({
        "type": "assistant", "id": "msg_a", "time": { "created": 1 },
        "content": [{ "type": "tool", "id": "call_1", "name": "read", "state": state, "time": { "created": 1 } }]
    }));

    history(&[message])
        .into_iter()
        .find(|message| message.role == MessageRole::ToolResult)
        .map(|message| (text_of(&message), message.is_error == Some(true)))
        .expect("the call's result")
}

#[test]
fn a_file_a_tool_read_shows_by_name_live_and_after_resume() {
    let content = json!([
        { "type": "text", "text": "Image read successfully" },
        { "type": "file", "uri": "data:image/png;base64,iVBORw0KGgo=", "mime": "image/png", "name": "/work/proj/dot.png" },
        { "type": "file", "uri": "data:image/png;base64,iVBORw0KGgo=", "mime": "image/png" }
    ]);
    let expected = ("Image read successfully\n@/work/proj/dot.png".to_owned(), false);

    let mut mapper = running_call("read", json!({ "path": "dot.png" }));
    let live = live_result(&mut mapper, "session.tool.success", json!({ "content": content }));
    let resumed = resumed_result(json!({ "status": "completed", "input": {}, "content": content }));

    assert_eq!(live, expected);
    assert_eq!(resumed, expected);
}

#[test]
fn a_failed_call_keeps_what_it_wrote_before_its_error() {
    let content = json!([{ "type": "text", "text": "partial line" }]);
    let error = json!({ "type": "tool", "message": "Command failed" });
    let expected = ("partial line\nCommand failed".to_owned(), true);

    let mut mapper = running_call("shell", json!({ "command": "make" }));
    let live = live_result(
        &mut mapper,
        "session.tool.failed",
        json!({ "error": error, "content": content }),
    );
    let resumed = resumed_result(json!({ "status": "error", "input": {}, "error": error, "content": content }));

    assert_eq!(live, expected);
    assert_eq!(resumed, expected);
}

#[test]
fn a_running_shell_calls_output_streams_until_it_ends() {
    let mut mapper = running_call("shell", json!({ "command": "make" }));

    // As recorded against 2.1.2: the progress names only the shell.
    let progress = ours(
        "session.tool.progress",
        json!({ "assistantMessageID": "msg_a", "id": "call_1", "metadata": { "shellID": "sh_15d82a82d001Fd4r5Yj4H78EvM" } }),
    );
    assert!(feed(&mut mapper, std::slice::from_ref(&progress)).is_empty());
    assert_eq!(
        mapper.take_shells(),
        [ShellCall {
            call: "call_1".to_owned(),
            shell: "sh_15d82a82d001Fd4r5Yj4H78EvM".to_owned()
        }]
    );

    feed(&mut mapper, &[progress]);
    assert!(mapper.take_shells().is_empty(), "each shell is read once");

    assert!(mapper.shell_wanted("call_1"));
    assert!(mapper.shell_output("call_1", "").is_empty());

    let update = mapper.shell_output("call_1", "line 1\n");
    assert!(matches!(
        &update[..],
        [AgentEvent::ToolUpdate { tool_call_id, partial }] if tool_call_id == "call_1" && result_text(partial) == "line 1\n"
    ));
    assert!(mapper.shell_output("call_1", "line 1\n").is_empty(), "nothing new");

    // Aborted without content: the card keeps what streamed, then the error.
    let ended = live_result(
        &mut mapper,
        "session.tool.failed",
        json!({ "error": { "type": "aborted", "message": "Tool execution interrupted" } }),
    );
    assert_eq!(ended, ("line 1\nTool execution interrupted".to_owned(), true));

    assert!(!mapper.shell_wanted("call_1"));
    assert!(mapper.shell_output("call_1", "line 1\nline 2\n").is_empty());
}

#[test]
fn only_a_shell_call_names_a_shell_the_engine_reads() {
    let mut mapper = running_call("read", json!({ "filePath": "a.txt" }));

    let progress = ours(
        "session.tool.progress",
        json!({ "assistantMessageID": "msg_a", "id": "call_1", "metadata": { "shellID": "sh_15d82a82d001Fd4r5Yj4H78EvM" } }),
    );
    feed(&mut mapper, &[progress]);

    assert!(mapper.take_shells().is_empty(), "a read call's shellID is ignored");
}

#[test]
fn a_prompt_that_runs_a_skill_names_it_live_and_after_resume() {
    let skill =
        json!({ "id": "greet", "name": "greet", "text": "<skill_content name=\"greet\">Say hello.</skill_content>" });

    let mut mapper = mapper();
    mapper.prompt_sent();

    let mut texts = Vec::new();

    for (inbox, text) in [("msg_bare", ""), ("msg_args", "be brief")] {
        let out = feed(
            &mut mapper,
            &[
                ours(
                    "session.inbox.enqueued",
                    json!({ "inboxID": inbox, "item": { "type": "user", "payload": { "text": text, "skills": [skill] } } }),
                ),
                ours("session.execution.started", json!({})),
                ours("session.inbox.delivered", json!({ "inboxID": inbox })),
            ],
        );
        texts.extend(ended(&events(out), MessageRole::User).iter().map(text_of));
    }

    assert_eq!(texts, ["/greet", "/greet be brief"]);

    let resumed = |skills: Value| {
        let message = SessionMessage::parse(&json!({
            "type": "user", "id": "msg_u", "time": { "created": 1 }, "text": "", "skills": skills
        }));
        history(&[message]).iter().map(text_of).collect::<Vec<_>>()
    };

    assert_eq!(resumed(json!([skill])), ["/greet"]);
    assert!(
        resumed(json!([{ "id": "greet", "name": "greet", "mention": { "start": 0, "end": 6 } }])).is_empty(),
        "a skill the text mentions is not repeated"
    );
}

#[test]
fn unknown_and_unreadable_messages_are_other() {
    for value in [
        json!({ "type": "future-kind", "id": "msg_1" }),
        json!({ "type": "user", "text": 42 }),
        json!("not a message"),
    ] {
        assert!(
            matches!(SessionMessage::parse(&value), SessionMessage::Other),
            "{value}"
        );
    }
}

#[test]
fn a_streaming_call_has_no_arguments_or_result_yet() {
    let value = json!({
        "type": "assistant", "id": "msg_1", "time": { "created": 1 },
        "content": [
            { "type": "tool", "id": "call_1", "name": "write", "state": { "status": "streaming", "input": "{\"pa" }, "time": { "created": 1 } },
            { "type": "future-part" }
        ]
    });

    let message = SessionMessage::parse(&value);
    let call = message.tool_calls().next().expect("the call");

    assert!(matches!(call.state, ToolState::Streaming));
    assert!(call.input().is_null());
    assert_eq!(call.outcome(), None);
    assert_eq!(message.chat_messages().len(), 1);
}

/// A `permission.asked` of `session` to edit a file.
fn asked(id: &str, session: &str) -> Event {
    event(
        "permission.asked",
        json!({ "id": id, "sessionID": session, "action": "edit", "resources": ["x.txt"] }),
    )
}

/// The input start and the progress of call `id` of tool `name`, whose metadata names `session`.
fn call_naming(id: &str, name: &str, session: &str) -> Vec<Event> {
    vec![
        ours(
            "session.tool.input.started",
            json!({ "assistantMessageID": "msg_a", "id": id, "name": name }),
        ),
        ours(
            "session.tool.progress",
            json!({ "assistantMessageID": "msg_a", "id": id, "metadata": { "sessionID": session, "status": "running" } }),
        ),
    ]
}

#[test]
fn a_session_a_call_names_counts_only_once_the_engine_confirms_it() {
    let mut mapper = mapper();

    feed(&mut mapper, &call_naming("call_m", "mcp_lookup", "ses_foreign"));
    assert!(
        mapper.take_unlisted().is_empty(),
        "another tool's metadata names no subagent session"
    );

    feed(&mut mapper, &call_naming("call_s", "subagent", "ses_named"));
    feed(&mut mapper, &call_naming("call_t", "subagent", "ses_named"));
    assert_eq!(mapper.take_unlisted(), ["ses_named"], "named once, handed out once");
    assert!(mapper.take_unlisted().is_empty());

    let before = events(feed(
        &mut mapper,
        &[asked("per_f", "ses_foreign"), asked("per_n", "ses_named")],
    ));
    assert!(
        requests(&before).is_empty(),
        "a named session's asks wait for the confirmation: {before:#?}"
    );
    assert!(!mapper.is_chat_session("ses_foreign"));
    assert!(!mapper.is_chat_session("ses_named"));

    mapper.adopt("ses_named", SESSION, None);
    let after = events(feed(&mut mapper, &[asked("per_m", "ses_named")]));

    let ids: Vec<String> = requests(&after).into_iter().map(|request| request.id).collect();
    assert_eq!(ids, ["per_m"]);
}
