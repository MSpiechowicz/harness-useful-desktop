//! Approval modes of the OpenCode chat engine against the scripted `opencode serve`: a first start
//! without the mode reads the agents, the chat's server then starts with the mode as
//! `OPENCODE_CONFIG_CONTENT` (the fake applies it to `GET /agent` like OpenCode), and nothing is
//! ever set on the session. Without a mode the server starts once, as launched.

mod agent_support;
mod opencode_engine_support;

use agent_support::options;
use harness_business::model::{AgentAction, ApprovalMode, IpcErrorCode};
use harness_business::opencode_server::mode_config::config_rules;
use harness_business::opencode_server::modes::{Rule, agent_rules, evaluate};
use harness_business::process::ExecSpec;
use opencode_engine_support::{Chat, env, start, start_env, start_mode, text_prompt, until_marker};
use pretty_assertions::assert_eq;
use serde_json::Value;

const CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

fn recorded_agents() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/opencode/server/agent-list.json"
    );
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("recording")).expect("JSON");

    recorded["body"].clone()
}

fn content(spec: &ExecSpec) -> Value {
    serde_json::from_str(&env(spec, CONTENT).expect("the mode's configuration")).expect("JSON")
}

/// What the agent `name` decides for `permission` on `pattern` once the server applies `content`.
fn decide(content: &Value, name: &str, permission: &str, pattern: &str) -> String {
    let mut rules: Vec<Rule> = agent_rules(&recorded_agents(), name);
    rules.extend(config_rules(&content["agent"][name]["permission"], None));

    evaluate(&rules, permission, pattern).map_or_else(|| "ask".to_owned(), |rule| rule.action.clone())
}

/// The two spawns of a chat with a mode: the probe, then the chat's server.
fn two_spawns(chat: &Chat) -> (ExecSpec, ExecSpec) {
    let mut spawns = chat.spawner.requested();
    assert_eq!(spawns.len(), 2, "the probe, then the chat's server");
    let server = spawns.pop().expect("server");

    (spawns.pop().expect("probe"), server)
}

#[tokio::test]
async fn a_mode_configures_every_agent_of_the_chats_server() {
    for (scenario, mode, webfetch) in [
        ("opencode_mode_always_ask.jsonl", ApprovalMode::AlwaysAsk, "ask"),
        ("opencode_mode_write.jsonl", ApprovalMode::Write, "ask"),
        ("opencode_mode_yolo.jsonl", ApprovalMode::Yolo, "allow"),
    ] {
        let chat = start_mode(scenario, Some(mode), false).await;
        assert!(chat.session.is_ok(), "{scenario}: {:?}", chat.session.as_ref().err());

        until_marker(&chat).await;
        let (probe, server) = two_spawns(&chat);
        assert_eq!(env(&probe, CONTENT), None, "the probe runs as launched");
        assert_ne!(
            env(&probe, "OPENCODE_SERVER_PASSWORD"),
            env(&server, "OPENCODE_SERVER_PASSWORD")
        );
        assert_eq!(chat.spawner.exited(), 1, "the probe was stopped");

        let content = content(&server);
        let configured: Vec<&String> = content["agent"].as_object().expect("agents").keys().collect();
        assert_eq!(configured.len(), 7, "every agent, subagents and hidden ones too");
        assert_eq!(decide(&content, "build", "webfetch", "https://x"), webfetch, "{mode:?}");
        assert_eq!(decide(&content, "plan", "edit", "src/main.rs"), "deny", "{mode:?}");
        assert!(chat.session().is_running(), "{scenario}: nothing set on the session");

        chat.session().stop().await;
    }
}

#[tokio::test]
async fn without_a_mode_the_server_starts_once_as_launched() {
    let chat = start("opencode_hang.jsonl").await;
    chat.session();

    let spawns = chat.spawner.requested();
    assert_eq!(spawns.len(), 1);
    assert_eq!(env(&spawns[0], CONTENT), None);

    chat.session().stop().await;
}

#[tokio::test]
async fn an_inherited_configuration_is_kept_and_ours_wins() {
    let mine =
        "{ // mine\n \"share\": \"disabled\", \"agent\": { \"build\": { \"permission\": { \"bash\": \"allow\" } } }, }";
    let chat = start_env(
        "opencode_mode_write.jsonl",
        None,
        Some(ApprovalMode::Write),
        options(),
        &[(CONTENT, mine)],
    )
    .await;
    until_marker(&chat).await;

    let (probe, server) = two_spawns(&chat);
    assert_eq!(
        env(&probe, CONTENT).as_deref(),
        Some(mine),
        "the probe sees the user's content"
    );

    let content = content(&server);
    assert_eq!(content["share"], "disabled");
    let build: Vec<&String> = content["agent"]["build"]["permission"]
        .as_object()
        .expect("build")
        .keys()
        .collect();
    assert_eq!(build[0], "bash", "the user's key stays first");

    let mut rules: Vec<Rule> = agent_rules(&recorded_agents(), "build");
    rules.extend(config_rules(&content["agent"]["build"]["permission"], None));
    let bash = evaluate(&rules, "bash", "ls").map(|rule| rule.action.clone());
    assert_eq!(bash.as_deref(), Some("ask"), "Write asks before bash all the same");
    assert!(chat.session().is_running());

    chat.session().stop().await;
}

#[tokio::test]
async fn an_inherited_configuration_that_is_not_an_object_refuses_the_mode() {
    let chat = start_env(
        "opencode_mode_write.jsonl",
        None,
        Some(ApprovalMode::Write),
        options(),
        &[(CONTENT, "{ \"model\": {env:MODEL} }")],
    )
    .await;

    let refused = chat.session.as_ref().err().expect("refused");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(refused.to_string().contains(CONTENT), "{refused}");
    assert!(chat.spawner.requested().is_empty(), "nothing started");
}

#[tokio::test]
async fn a_mode_the_server_does_not_show_refuses_the_chat() {
    let chat = start_mode("opencode_mode_unapplied.jsonl", Some(ApprovalMode::Yolo), false).await;

    let refused = chat.session.as_ref().err().expect("refused");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(refused.to_string().contains("approval mode"), "{refused}");
    assert!(refused.to_string().contains("a managed setting"), "{refused}");
    chat.wait_reaped().await;
}

#[tokio::test]
async fn switching_agents_sets_nothing_on_the_session() {
    let chat = start_mode("opencode_mode_switch.jsonl", Some(ApprovalMode::Write), false).await;
    let session = chat.session();

    session
        .send(AgentAction::SetAgent {
            agent: "plan".to_owned(),
        })
        .await
        .expect("agent set");
    session.send(text_prompt("Make a plan")).await.expect("the prompt");

    assert!(session.is_running(), "only the prompt reached the server");
    session.stop().await;
}
