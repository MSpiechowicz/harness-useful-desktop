//! The OpenCode server password and the subagent panel: the password is masked before any text is
//! cut to its limit, so a cut never leaves a piece of it. Also what the panel trusts: the agent the
//! server reports (not the one a model wrote), the chat's own session (never a subagent) and the
//! size of ids and labels.

use crate::opencode_mapper_support;
use crate::opencode_subagent_support;

use harness_business::model::{
    MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_BYTES, MAX_STEP_TEXT_CHARS, MAX_SUBAGENT_TEXT_CHARS, SubagentStatus,
};
use opencode_mapper_support::{SESSION, events, feed, mapper};
use opencode_subagent_support::crafted::{child_status, created, prose, reply, task, tool};
use opencode_subagent_support::{fake_password, latest, leaked, shown, snapshots, steps, straddling, straddling_tail};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CHILD: &str = "ses_child1";

/// A reply of subagent `CHILD` that names `agent`, `provider` and `model`.
fn reply_by(agent: &str, provider: &str, model: &str) -> Value {
    json!({ "type": "message.updated", "properties": { "sessionID": CHILD, "info": {
        "id": "msg_c1", "role": "assistant", "agent": agent, "providerID": provider, "modelID": model,
        "cost": 0, "tokens": {}, "time": { "created": 1 } } } })
}

/// The `task` call `call` with the given input, in state `state`.
fn call_of(call: &str, input: &Value, state: Value) -> Value {
    let mut full = json!({ "status": "running", "input": input });

    for (key, value) in state.as_object().cloned().unwrap_or_default() {
        full[key] = value;
    }

    opencode_mapper_support::part(
        SESSION,
        json!({ "id": "prt_t", "messageID": "msg_parent", "type": "tool", "tool": "task", "callID": call,
            "state": full }),
    )
}

#[test]
fn a_password_cut_by_any_limit_leaves_no_piece_of_it_in_the_panel() {
    let password = fake_password();
    let mut mapper = mapper();
    mapper.set_secret(Some(&password));

    let input = json!({
        "description": straddling(MAX_STEP_DETAIL_CHARS, &password),
        "prompt": straddling(MAX_SUBAGENT_TEXT_CHARS, &password),
        "subagent_type": straddling(MAX_STEP_DETAIL_CHARS, &password),
    });
    let command = straddling(MAX_STEP_DETAIL_CHARS, &password);
    let report = format!(
        "<task_result>\n{}\n</task_result>",
        straddling(MAX_SUBAGENT_TEXT_CHARS, &password)
    );

    let out = events(feed(
        &mut mapper,
        &[
            call_of("call_t", &input, json!({})),
            created(CHILD, SESSION),
            reply_by(
                &straddling(MAX_STEP_DETAIL_CHARS, &password),
                &straddling(MAX_STEP_DETAIL_CHARS, &password),
                &straddling(MAX_STEP_DETAIL_CHARS, &password),
            ),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "running", "input": { "command": command } }),
            ),
            tool(
                CHILD,
                "call_a",
                "bash",
                json!({ "status": "completed", "input": { "command": command },
                    "output": straddling_tail(MAX_STEP_OUTPUT_BYTES, &password) }),
            ),
            tool(
                CHILD,
                "call_b",
                &straddling(MAX_STEP_DETAIL_CHARS, &password),
                json!({ "status": "error", "input": {}, "error": straddling_tail(MAX_STEP_OUTPUT_BYTES, &password) }),
            ),
            prose(
                CHILD,
                "prt_x",
                "msg_c1",
                &straddling(MAX_STEP_TEXT_CHARS, &password),
                true,
            ),
            call_of(
                "call_t",
                &input,
                json!({ "status": "completed", "metadata": { "sessionId": CHILD },
                "output": report }),
            ),
        ],
    ));

    assert_eq!(
        leaked(&shown(&out), &password),
        None,
        "no panel event shows a piece of the password"
    );

    // The cuts happened: the texts are at their limits, so the test reaches every one of them.
    let all = steps(&out, CHILD);
    assert!(
        all.iter()
            .any(|step| step.detail.chars().count() == MAX_STEP_DETAIL_CHARS)
    );
    assert!(
        all.iter()
            .any(|step| step.detail.chars().count() == MAX_STEP_TEXT_CHARS)
    );
    assert!(all.iter().filter_map(|step| step.output.as_ref()).count() >= 2);
    assert_eq!(
        latest(&out, CHILD).report.map(|report| report.chars().count()),
        Some(MAX_SUBAGENT_TEXT_CHARS)
    );
}

#[test]
fn a_password_cut_in_the_last_words_used_as_the_report_is_masked_too() {
    let password = fake_password();
    let mut mapper = mapper();
    mapper.set_secret(Some(&password));

    let words = straddling(MAX_SUBAGENT_TEXT_CHARS, &password);
    let out = events(feed(
        &mut mapper,
        &[
            task("call_t", "Check", "general", json!({})),
            created(CHILD, SESSION),
            reply(CHILD, "msg_c1", json!({}), 0.0),
            prose(CHILD, "prt_x", "msg_c1", &words, true),
            child_status(CHILD, "idle"),
        ],
    ));

    let report = latest(&out, CHILD).report.expect("the last words are the report");

    assert_eq!(report.chars().count(), MAX_SUBAGENT_TEXT_CHARS);
    assert_eq!(leaked(&shown(&out), &password), None);
}

#[test]
fn the_chats_own_session_is_never_a_subagent() {
    let mut mapper = mapper();

    let out = events(feed(
        &mut mapper,
        &[
            task("call_t", "Check", "general", json!({})),
            created(SESSION, SESSION),
            tool(
                SESSION,
                "call_a",
                "bash",
                json!({ "status": "running", "input": { "command": "ls" } }),
            ),
        ],
    ));

    assert!(snapshots(&out, SESSION).is_empty());
    assert_eq!(
        mapper.subagent_active(SESSION),
        None,
        "stopping it would abort the whole chat"
    );
}

#[test]
fn a_message_runs_as_the_agent_the_server_reported_never_the_one_a_model_wrote() {
    let mut mapper = mapper();

    let before = events(feed(
        &mut mapper,
        &[task("call_t", "Check", "docs", json!({})), created(CHILD, SESSION)],
    ));

    assert_eq!(
        latest(&before, CHILD).agent,
        "docs",
        "the panel shows what the call asked for"
    );
    assert_eq!(
        mapper.subagent_settings(CHILD).map(|settings| settings.agent),
        Some(None),
        "but a message needs the server's word for it"
    );

    let after = events(feed(&mut mapper, &[reply(CHILD, "msg_c1", json!({}), 0.0)]));

    assert_eq!(latest(&after, CHILD).agent, "general");
    assert_eq!(
        mapper
            .subagent_settings(CHILD)
            .and_then(|settings| settings.agent)
            .as_deref(),
        Some("general")
    );

    // A later task update does not bring the model's agent back.
    let later = events(feed(&mut mapper, &[task("call_t", "Check", "docs", json!({}))]));
    assert!(
        snapshots(&later, CHILD)
            .iter()
            .all(|snapshot| snapshot.agent == "general")
    );
}

#[test]
fn ids_agents_and_models_from_the_server_are_bounded() {
    let long = "l".repeat(1000);
    let mut mapper = mapper();

    let out = events(feed(
        &mut mapper,
        &[
            call_of(
                "call_t",
                &json!({ "description": "Check", "subagent_type": "general" }),
                json!({}),
            ),
            created(&"s".repeat(1000), SESSION),
            created(CHILD, SESSION),
            reply_by(&long, &long, &long),
            tool(
                CHILD,
                &long,
                "bash",
                json!({ "status": "running", "input": { "command": "ls" } }),
            ),
        ],
    ));

    let child = latest(&out, CHILD);

    assert_eq!(child.status, SubagentStatus::Running);
    assert!(child.agent.chars().count() <= MAX_STEP_DETAIL_CHARS);
    assert!(
        child
            .model
            .as_deref()
            .is_some_and(|model| model.chars().count() <= 2 * MAX_STEP_DETAIL_CHARS + 1)
    );
    assert!(
        steps(&out, CHILD)
            .iter()
            .all(|step| step.id.chars().count() <= MAX_STEP_DETAIL_CHARS)
    );
    assert!(
        out.iter().all(|event| match event {
            harness_business::model::AgentEvent::Subagent { subagent } => subagent.id.chars().count() <= 240,
            _ => true,
        }),
        "a child session with an absurd id is not tracked"
    );
}

#[test]
fn a_repeated_step_is_not_sent_again_and_a_changed_one_is() {
    let mut mapper = mapper();
    let running = |command: &str| {
        tool(
            CHILD,
            "call_a",
            "bash",
            json!({ "status": "running", "input": { "command": command } }),
        )
    };

    let out = events(feed(
        &mut mapper,
        &[
            task("call_t", "Check", "general", json!({})),
            created(CHILD, SESSION),
            running("ls"),
            running("ls"),
            running("ls -la"),
        ],
    ));

    let details: Vec<String> = steps(&out, CHILD).into_iter().map(|step| step.detail).collect();

    assert_eq!(details, ["ls", "ls -la"]);
}

#[test]
fn a_run_reports_at_most_a_bounded_number_of_steps() {
    let mut mapper = mapper();
    let mut frames = vec![task("call_t", "Check", "general", json!({})), created(CHILD, SESSION)];

    frames.extend((0..2_500).map(|number| {
        tool(
            CHILD,
            &format!("call_{number}"),
            "bash",
            json!({ "status": "running", "input": { "command": "ls" } }),
        )
    }));

    let out = events(feed(&mut mapper, &frames));

    assert_eq!(steps(&out, CHILD).len(), 2_000);
}

#[test]
fn a_message_routes_with_the_exact_agent_and_never_a_cut_or_masked_copy() {
    let mut mapper = mapper();
    let long = "a".repeat(300);
    let route = |mapper: &harness_business::opencode_server::Mapper| {
        mapper
            .subagent_settings(CHILD)
            .map(|settings| (settings.agent, settings.model.map(|model| (model.provider, model.id))))
    };

    feed(
        &mut mapper,
        &[task("call_t", "Check", "docs", json!({})), created(CHILD, SESSION)],
    );
    let shown = events(feed(&mut mapper, &[reply_by(&long, "mock", "m1")]));

    assert!(
        latest(&shown, CHILD).agent.chars().count() <= MAX_STEP_DETAIL_CHARS,
        "the panel shows a cut copy"
    );
    assert_eq!(route(&mapper), Some((None, None)), "a cut agent is not routed with");

    feed(&mut mapper, &[reply_by("general", "mock", &long)]);
    assert_eq!(
        route(&mapper),
        Some((None, None)),
        "a cut model is not routed with either"
    );

    feed(&mut mapper, &[reply_by("general", "mock", "m1")]);
    assert_eq!(
        route(&mapper),
        Some((Some("general".to_owned()), Some(("mock".to_owned(), "m1".to_owned()))))
    );
}

#[test]
fn a_reported_agent_the_password_touches_is_not_kept_for_routing() {
    let password = fake_password();
    let mut mapper = mapper();
    mapper.set_secret(Some(&password));

    feed(
        &mut mapper,
        &[task("call_t", "Check", "docs", json!({})), created(CHILD, SESSION)],
    );
    feed(&mut mapper, &[reply_by(&format!("agent-{password}"), "mock", "m1")]);

    let settings = mapper.subagent_settings(CHILD).expect("settings");
    assert_eq!((settings.agent, settings.model), (None, None));
}
