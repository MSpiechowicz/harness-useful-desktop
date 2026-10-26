//! Approval modes and helper chats of the OpenCode 2 chat engine against the scripted OpenCode 2
//! server: a first start without the mode reads the agents, the chat's server then starts with the
//! mode as `OPENCODE_CONFIG_CONTENT` (the fake applies it to `GET /api/agent` like OpenCode), and
//! nothing is ever set on the session. Agents OpenCode reloads while the chat runs are checked
//! again, before a subagent ask for an agent the probe did not list shows. A helper chat's session is created with rules that deny every tool and deleted when the
//! chat stops.

use crate::agent_support;
use crate::opencode2_engine_support;
use crate::opencode2_recorded;

use std::time::Duration;

use agent_support::{WAIT, options};
use harness_business::BusinessError;
use harness_business::agent::SessionOptions;
use harness_business::model::{
    AgentAction, AgentEvent, ApprovalMode, IpcErrorCode, NoticeLevel, UiOutcome, UiResponse,
};
use harness_business::opencode_server_v2::mode_config::{configured_rules, listed_agents};
use harness_business::opencode_server_v2::modes::{Rule, decide};
use harness_business::process::ExecSpec;
use opencode2_engine_support::{
    Chat, Launch, SESSION, env, start, start_mode, start_through, terminated, until_marker,
};
use pretty_assertions::assert_eq;
use serde_json::Value;

const CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

fn launch<'a>(name: &'a str, approval: Option<ApprovalMode>, env: &'a [(&'a str, &'a str)]) -> Launch<'a> {
    Launch {
        name,
        resume: None,
        approval,
        utility: false,
        options: options(),
        env,
    }
}

async fn start_launch(launch: Launch<'_>) -> Chat {
    start_through(launch, |spawner| spawner, Box::pin(std::future::pending())).await
}

fn content(spec: &ExecSpec) -> Value {
    serde_json::from_str(&env(spec, CONTENT).expect("the mode's configuration")).expect("JSON")
}

/// The two spawns of a chat with a mode: the probe, then the chat's server.
fn two_spawns(chat: &Chat) -> (ExecSpec, ExecSpec) {
    let mut spawns = chat.spawner.requested();
    assert_eq!(spawns.len(), 2, "the probe, then the chat's server");
    let server = spawns.pop().expect("server");

    (spawns.pop().expect("probe"), server)
}

/// What the recorded agent `id` decides for `action` on `resource` with the rules of `content`.
fn decision(content: &Value, id: &str, action: &str, resource: &str) -> String {
    let recorded = opencode2_recorded::find("GET", "/api/agent")[0].response["data"].clone();
    let listed = listed_agents(&recorded).expect("agents");
    let (_, mut rules): (String, Vec<Rule>) = listed.into_iter().find(|(agent, _)| agent == id).expect("listed");
    rules.extend(configured_rules(content, id));

    decide(&rules, action, resource).to_owned()
}

/// Panics when the fake reported a request the scenario did not expect (a `fake_agent:` stderr line).
fn assert_matched(events: &[AgentEvent]) {
    let mismatches: Vec<&AgentEvent> = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Stderr { line } if line.starts_with("fake_agent:")))
        .collect();

    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// The error a start ended with.
fn refused(chat: &Chat) -> &BusinessError {
    chat.session.as_ref().err().expect("the start fails")
}

/// Waits until every server the chat spawned was reaped.
async fn all_reaped(chat: &Chat) {
    let deadline = tokio::time::Instant::now() + WAIT;

    while chat.spawner.exited() < chat.spawner.requested().len() {
        assert!(tokio::time::Instant::now() < deadline, "not every server was reaped");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_mode_configures_every_agent_of_the_chats_server() {
    for (mode, shell, read) in [
        (ApprovalMode::AlwaysAsk, "ask", "ask"),
        (ApprovalMode::Write, "ask", "allow"),
        (ApprovalMode::Yolo, "allow", "allow"),
    ] {
        let chat = start_mode("opencode2_modes.jsonl", Some(mode), false).await;
        assert!(chat.session.is_ok(), "{mode:?}: {:?}", chat.session.as_ref().err());
        assert_matched(&until_marker(&chat).await);

        let (probe, server) = two_spawns(&chat);
        assert_eq!(env(&probe, CONTENT), None, "the probe runs as launched");
        assert_ne!(
            env(&probe, "OPENCODE_SERVER_PASSWORD"),
            env(&server, "OPENCODE_SERVER_PASSWORD")
        );
        assert_eq!(chat.spawner.exited(), 1, "the probe was stopped");

        let content = content(&server);
        let configured = content["agents"].as_object().expect("agents");
        assert_eq!(configured.len(), 7, "every agent, subagents and hidden ones too");
        assert_eq!(decision(&content, "build", "shell", "ls"), shell, "{mode:?}");
        assert_eq!(decision(&content, "build", "read", "a.txt"), read, "{mode:?}");
        assert_eq!(decision(&content, "plan", "edit", "src/main.rs"), "deny", "{mode:?}");

        let created = chat.requests_to("POST /api/session");
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].body, serde_json::json!({}), "nothing set on the session");
        assert!(chat.requests_to("PATCH /api/session/*").is_empty());
        assert_eq!(chat.requests_to("GET /api/permission/saved").len(), 1);

        chat.session().stop().await;
    }
}

#[tokio::test]
async fn without_a_mode_the_server_starts_once_as_launched() {
    let chat = start("opencode2_hang.jsonl").await;
    chat.session();

    let spawns = chat.spawner.requested();
    assert_eq!(spawns.len(), 1);
    assert_eq!(env(&spawns[0], CONTENT), None);
    assert!(chat.requests_to("GET /api/permission/saved").is_empty());

    chat.session().stop().await;
}

#[tokio::test]
async fn an_inherited_configuration_keeps_its_rules_before_the_modes() {
    let mine = "{ // mine\n \"share\": \"disabled\", \"agents\": { \"build\": { \"permissions\": [{ \"action\": \"shell\", \
                \"resource\": \"git push*\", \"effect\": \"deny\" }] } } }";
    let inherited = [(CONTENT, mine)];
    let chat = start_launch(launch("opencode2_modes.jsonl", Some(ApprovalMode::Yolo), &inherited)).await;
    assert!(chat.session.is_ok(), "{:?}", chat.session.as_ref().err());
    assert_matched(&until_marker(&chat).await);

    let (probe, server) = two_spawns(&chat);
    assert_eq!(
        env(&probe, CONTENT).as_deref(),
        Some(mine),
        "the probe sees the user's content"
    );

    let content = content(&server);
    assert_eq!(content["share"], "disabled");
    let build: Vec<Rule> = serde_json::from_value(content["agents"]["build"]["permissions"].clone()).expect("rules");
    assert_eq!(
        build[0],
        Rule::new("shell", "git push*", "deny"),
        "the user's rule first"
    );
    assert_eq!(
        decide(&build, "shell", "git push origin"),
        "deny",
        "the probe saw it: Yolo keeps it"
    );
    assert_eq!(decide(&build, "shell", "ls"), "allow");
    assert!(chat.session().is_running());

    chat.session().stop().await;
}

#[tokio::test]
async fn an_inherited_configuration_that_cannot_take_the_mode_refuses_it() {
    let unreadable = [(CONTENT, "{ \"model\": {env:MODEL} }")];
    let chat = start_launch(launch("opencode2_modes.jsonl", Some(ApprovalMode::Write), &unreadable)).await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(error.to_string().contains(CONTENT), "{error}");
    assert!(chat.spawner.requested().is_empty(), "nothing started");

    let legacy = [(
        CONTENT,
        "{ \"agent\": { \"build\": { \"permission\": { \"bash\": \"allow\" } } } }",
    )];
    let chat = start_launch(launch("opencode2_modes.jsonl", Some(ApprovalMode::Write), &legacy)).await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(
        error.to_string().contains("the OpenCode 1 way (`agent.build`)"),
        "{error}"
    );
    assert_eq!(chat.spawner.requested().len(), 1, "only the probe ran");
    all_reaped(&chat).await;
}

#[tokio::test]
async fn a_mode_the_server_does_not_show_refuses_the_chat() {
    let chat = start_mode("opencode2_modes_unapplied.jsonl", Some(ApprovalMode::Yolo), false).await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(error.to_string().contains("approval mode"), "{error}");
    assert!(
        error.to_string().contains("a managed setting or a plugin changes"),
        "{error}"
    );
    assert_eq!(chat.spawner.requested().len(), 2);
    all_reaped(&chat).await;
}

#[tokio::test]
async fn saved_always_allow_rules_refuse_the_modes_that_ask() {
    for (mode, actions) in [
        (ApprovalMode::AlwaysAsk, "(for shell, read)"),
        (ApprovalMode::Write, "(for shell)"),
    ] {
        let chat = start_mode("opencode2_modes_saved.jsonl", Some(mode), false).await;

        let error = refused(&chat);
        assert_eq!(error.code(), IpcErrorCode::Unsupported);
        assert!(error.to_string().contains("saved \"Always allow\" rules"), "{error}");
        assert!(error.to_string().contains(actions), "{mode:?}: {error}");
        all_reaped(&chat).await;
    }

    let chat = start_mode("opencode2_modes_saved.jsonl", Some(ApprovalMode::Yolo), false).await;
    assert!(
        chat.session.is_ok(),
        "Yolo asks for nothing: {:?}",
        chat.session.as_ref().err()
    );
    chat.session().stop().await;
}

#[tokio::test]
async fn a_resumed_session_with_rules_of_its_own_refuses_a_mode() {
    let resumed = Launch {
        resume: Some(SESSION),
        ..launch("opencode2_modes_resume_stored.jsonl", Some(ApprovalMode::Write), &[])
    };
    let chat = start_launch(resumed).await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(error.to_string().contains("rules of its own"), "{error}");
    assert_eq!(chat.spawner.requested().len(), 1, "only the probe ran");
    all_reaped(&chat).await;
}

#[tokio::test]
async fn an_agent_added_while_the_chat_runs_stops_a_chat_with_a_mode() {
    let chat = start_mode("opencode2_modes_agent_added.jsonl", Some(ApprovalMode::Write), false).await;
    assert_matched(&until_marker(&chat).await);
    assert!(
        chat.session().is_running(),
        "OpenCode's own agent.updated at startup changes nothing"
    );

    let rename = AgentAction::SetSessionName { name: "Go".to_owned() };
    chat.session().send(rename).await.expect("renamed");

    let notice = chat
        .wait("the mode notice", |event| {
            matches!(event, AgentEvent::Notice { level: NoticeLevel::Error, message } if message.contains("approval mode"))
        })
        .await;
    assert!(matches!(notice, AgentEvent::Notice { .. }));
    chat.wait("exited", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;
    assert_matched(&chat.events());
}

#[tokio::test]
async fn a_reload_with_the_same_agents_and_rules_keeps_a_chat_with_a_mode() {
    let chat = start_mode("opencode2_modes_reload_same.jsonl", Some(ApprovalMode::Write), false).await;
    assert_matched(&until_marker(&chat).await);
    let listed = chat.requests_to("GET /api/agent").len();

    let rename = AgentAction::SetSessionName { name: "Go".to_owned() };
    chat.session().send(rename).await.expect("renamed");

    chat.wait(
        "the second title",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Checked")),
    )
    .await;

    let events = chat.events();
    assert_matched(&events);
    assert!(
        chat.requests_to("GET /api/agent").len() >= listed + 2,
        "config.updated and agent.updated each check the mode again"
    );
    let error = |event: &&AgentEvent| {
        matches!(
            event,
            AgentEvent::Notice {
                level: NoticeLevel::Error,
                ..
            }
        )
    };
    assert_eq!(events.iter().find(error), None, "no error notice");
    assert!(chat.session().is_running(), "the mode still holds: the chat goes on");

    chat.session().stop().await;
}

#[tokio::test]
async fn always_allow_never_answers_a_subagent_ask_for_an_agent_the_probe_did_not_list() {
    let chat = start_mode(
        "opencode2_modes_unknown_subagent.jsonl",
        Some(ApprovalMode::Write),
        false,
    )
    .await;
    let message = opencode2_engine_support::text_prompt("Say hello");
    chat.session().send(message).await.expect("prompt taken");

    let shown = |id: &'static str| move |event: &AgentEvent| matches!(event, AgentEvent::UiRequest { request } if request.id == id);
    chat.wait("per_1", shown("per_1")).await;
    let always = UiResponse {
        id: "per_1".to_owned(),
        outcome: UiOutcome::Value {
            value: "Always allow".to_owned(),
        },
    };
    chat.session().respond(always).await.expect("always");

    chat.wait("per_2", shown("per_2")).await;
    let events = until_marker(&chat).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let replies = chat.requests_to("POST /api/session/*/permission/*/reply");
    assert_eq!(replies.len(), 1, "only per_1 was answered: {replies:#?}");
    assert!(
        chat.session().is_running(),
        "the agents did not change: the chat goes on"
    );
    assert_matched(&events);

    chat.session().stop().await;
}

#[tokio::test]
async fn a_subagent_ask_for_an_added_agent_stops_the_chat_before_it_shows_live_and_after_a_reconnect() {
    for name in [
        "opencode2_modes_unknown_subagent_added.jsonl",
        "opencode2_modes_unknown_subagent_catch_up.jsonl",
    ] {
        let chat = start_mode(name, Some(ApprovalMode::Write), false).await;
        assert_matched(&until_marker(&chat).await);

        // The scenario then adds an agent and raises a subagent ask for it.
        let rename = AgentAction::SetSessionName { name: "Go".to_owned() };
        chat.session().send(rename).await.expect("renamed");

        chat.wait("exited", |event| matches!(event, AgentEvent::Exited { .. }))
            .await;
        let events = chat.events();
        assert_matched(&events);

        let notice = |event: &AgentEvent| matches!(event, AgentEvent::Notice { level: NoticeLevel::Error, message } if message.contains("approval mode"));
        let dialogs = events
            .iter()
            .filter(|event| matches!(event, AgentEvent::UiRequest { .. }));
        assert!(events.iter().any(notice), "{name}: the mode notice");
        assert_eq!(dialogs.count(), 0, "{name}: no dialog for the ask");
    }
}

/// Where the `deleted` stderr line the helper scenarios write before answering the delete is.
fn deleted_at(events: &[AgentEvent]) -> Option<usize> {
    events
        .iter()
        .position(|event| matches!(event, AgentEvent::Stderr { line } if line == "deleted"))
}

#[tokio::test]
async fn a_helper_chat_denies_every_tool_and_deletes_its_session_at_stop() {
    let chat = start_mode("opencode2_helper.jsonl", Some(ApprovalMode::Yolo), true).await;
    until_marker(&chat).await;

    let spawns = chat.spawner.requested();
    assert_eq!(spawns.len(), 1, "a helper chat starts no probe");
    assert_eq!(env(&spawns[0], CONTENT), None, "nor a mode");

    chat.session().stop().await;

    let events = chat.events();
    assert_matched(&events);
    assert!(
        deleted_at(&events).is_some(),
        "the session was deleted at stop: {events:#?}"
    );
    assert_eq!(events.last(), Some(&terminated()));

    let resumed = Launch {
        resume: Some(SESSION),
        utility: true,
        ..launch("opencode2_helper.jsonl", None, &[])
    };
    let resumed = start_launch(resumed).await;
    let error = refused(&resumed);
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert!(resumed.spawner.requested().is_empty(), "nothing was started");
}

#[tokio::test]
async fn a_helper_chat_that_loses_its_server_deletes_its_session() {
    let quick = SessionOptions {
        event_silence: Duration::from_millis(200),
        ..options()
    };
    let helper = Launch {
        utility: true,
        options: quick,
        ..launch("opencode2_helper_lost.jsonl", None, &[])
    };
    let chat = start_launch(helper).await;
    until_marker(&chat).await;

    chat.wait("exited", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;

    let events = chat.events();
    assert_matched(&events);
    let notice = events.iter().position(
        |event| matches!(event, AgentEvent::Notice { level: NoticeLevel::Error, message } if message.contains("lost")),
    );
    let deleted = deleted_at(&events);
    assert!(notice.is_some() && deleted.is_some(), "{events:#?}");
    assert!(notice < deleted, "{events:#?}");
}

#[tokio::test]
async fn a_helper_session_without_its_rules_refuses_the_chat() {
    let chat = start_mode("opencode2_helper_unkept.jsonl", None, true).await;

    let error = refused(&chat);
    assert!(matches!(error, BusinessError::Protocol(_)), "{error:?}");
    assert!(error.to_string().contains("helper chat"), "{error}");
    all_reaped(&chat).await;
}
