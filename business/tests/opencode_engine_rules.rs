//! The permission rules an OpenCode chat leaves in OpenCode's database, against the scripted
//! `opencode serve` keeping them like OpenCode does (`FAKE_OPENCODE_STORE`: a `PATCH` appends):
//! released at stop, neutralised on resume, not sent twice, and released when the event stream
//! is lost.

mod agent_support;
mod opencode_engine_support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_support::options;
use harness_business::agent::SessionOptions;
use harness_business::model::{AgentEvent, ApprovalMode, NoticeLevel};
use harness_business::opencode_server::modes::{Rule, ends_with, evaluate, neutral, session_rules};
use opencode_engine_support::{Chat, SESSION, start_env, until_marker};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

/// The build agent of the `opencode_rules_store*` scenarios: the user's settings ask before `bash`.
fn build() -> Vec<Rule> {
    vec![Rule::new("*", "*", "allow"), Rule::new("bash", "*", "ask")]
}

fn yolo() -> Vec<Rule> {
    session_rules(Some(ApprovalMode::Yolo), &build()).expect("rules")
}

/// A store file for the fake, holding `stored` for the chat's session when given.
struct Store {
    _dir: TempDir,
    path: PathBuf,
}

impl Store {
    fn new(stored: Option<&[Rule]>) -> Self {
        let dir = tempfile::tempdir().expect("store folder");
        let path = dir.path().join("sessions.json");
        let sessions = match stored {
            Some(rules) => json!({ SESSION: rules }),
            None => json!({}),
        };
        std::fs::write(&path, sessions.to_string()).expect("seed the store");

        Self { _dir: dir, path }
    }

    /// The rules the fake keeps for the chat's session.
    fn rules(&self) -> Vec<Rule> {
        let sessions: Value = serde_json::from_str(&std::fs::read_to_string(&self.path).expect("store")).expect("JSON");

        serde_json::from_value(sessions[SESSION].clone()).unwrap_or_default()
    }

    async fn start(&self, scenario: &str, resume: bool, mode: Option<ApprovalMode>, options: SessionOptions) -> Chat {
        let resume = resume.then_some(SESSION);
        let env = [("FAKE_OPENCODE_STORE", path_text(&self.path))];

        start_env(scenario, resume, mode, options, &env).await
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}

/// What OpenCode decides for `bash`: the agent's rules, then the session's.
fn bash(stored: &[Rule]) -> String {
    let rules: Vec<Rule> = build().into_iter().chain(stored.iter().cloned()).collect();

    evaluate(&rules, "bash", "ls").map_or_else(|| "ask".to_owned(), |rule| rule.action.clone())
}

fn concat(lists: &[Vec<Rule>]) -> Vec<Rule> {
    lists.concat()
}

#[tokio::test]
async fn a_mode_is_released_at_stop_and_a_resume_without_override_leaves_it_alone() {
    let store = Store::new(None);

    let chat = store
        .start("opencode_rules_store.jsonl", false, Some(ApprovalMode::Yolo), options())
        .await;
    until_marker(&chat).await;
    assert_eq!(store.rules(), yolo());
    assert_eq!(bash(&store.rules()), "allow", "Yolo allows bash while the chat runs");

    chat.session().stop().await;
    let released = store.rules();
    assert_eq!(released, concat(&[yolo(), neutral(&build())]));
    assert!(ends_with(&released, &neutral(&build())));
    assert_eq!(bash(&released), "ask", "the user's settings decide after the chat");

    let resumed = store
        .start("opencode_rules_store_resume.jsonl", true, None, options())
        .await;
    until_marker(&resumed).await;
    assert!(resumed.session().is_running(), "no request the scenario did not expect");
    assert_eq!(
        store.rules(),
        released,
        "a session ending with the neutral rules gets nothing"
    );

    resumed.session().stop().await;
    assert_eq!(store.rules(), released, "nothing to release without an override");
}

#[tokio::test]
async fn rules_left_behind_are_neutralised_when_the_session_is_resumed_without_override() {
    let store = Store::new(Some(&yolo()));

    let chat = store
        .start("opencode_rules_store_resume.jsonl", true, None, options())
        .await;
    until_marker(&chat).await;

    assert_eq!(store.rules(), concat(&[yolo(), neutral(&build())]));
    assert_eq!(
        bash(&store.rules()),
        "ask",
        "the rules of a crashed Yolo chat no longer apply"
    );

    chat.session().stop().await;
}

#[tokio::test]
async fn a_session_that_ends_with_the_modes_rules_is_not_patched_again() {
    let store = Store::new(Some(&yolo()));

    let chat = store
        .start(
            "opencode_rules_store_resume.jsonl",
            true,
            Some(ApprovalMode::Yolo),
            options(),
        )
        .await;
    until_marker(&chat).await;
    assert_eq!(store.rules(), yolo(), "the stored rules already end with the mode's");

    chat.session().stop().await;
    assert_eq!(store.rules(), concat(&[yolo(), neutral(&build())]), "released at stop");
}

/// Waits for `exited`; the events then.
async fn exited(chat: &Chat) -> Vec<AgentEvent> {
    chat.wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;

    chat.events()
}

/// The position of the lost-connection notice, which must come before `exited`.
fn lost_notice(events: &[AgentEvent]) -> usize {
    let notice = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Notice { level: NoticeLevel::Error, message } if message.contains("connection to OpenCode was lost")))
        .unwrap_or_else(|| panic!("no lost-connection notice: {events:#?}"));
    let exit = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Exited { .. }))
        .expect("exited");

    assert!(notice < exit, "the notice comes first");
    assert_eq!(exit, events.len() - 1, "exited is the last event");
    notice
}

#[tokio::test]
async fn a_silent_event_stream_stops_the_chat_and_releases_its_rules() {
    let store = Store::new(None);
    let quick = SessionOptions {
        event_silence: Duration::from_millis(300),
        ..options()
    };

    let chat = store
        .start("opencode_rules_store.jsonl", false, Some(ApprovalMode::Yolo), quick)
        .await;
    let events = exited(&chat).await;

    lost_notice(&events);
    assert!(!chat.session().is_running());
    assert_eq!(
        store.rules(),
        concat(&[yolo(), neutral(&build())]),
        "released before the server ended"
    );
}

#[tokio::test]
async fn a_closed_event_stream_stops_the_chat() {
    let store = Store::new(None);

    let chat = store
        .start("opencode_rules_store_lost.jsonl", false, None, options())
        .await;
    let events = exited(&chat).await;

    lost_notice(&events);
    assert_eq!(
        events.last(),
        Some(&AgentEvent::Exited {
            code: None,
            signal: Some(15)
        }),
        "the server was ended"
    );
    assert!(store.rules().is_empty(), "no rules without an override");
}
