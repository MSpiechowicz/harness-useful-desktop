//! A Codex chat with a memory identity (plan decision 11): Codex starts its MCP servers with a
//! cleared environment, so after the handshake the chat asks Codex which servers it runs the
//! project with (`config/read`) and names `HARNESS_MEMORY_SESSION` among the variables of each
//! harness-memory server there, in the `config` of its threads (the scenarios check it), never on
//! the command line. The chat's own guards count as applied (`memoryGuards`) only when a server
//! gets the variable or passes it on already. A chat without a memory identity asks nothing. Every
//! Codex spawn drops a relative `CODEX_HOME`, which Codex would resolve in the project.

use crate::agent_support;
use crate::common;

use std::ffi::OsString;
use std::path::Path;

use agent_support::{Run, options, start_at};
use common::write_file;
use harness_business::ecosystem::SESSION_VAR;
use harness_business::env::Env;
use harness_business::model::{AgentEvent, HarnessId, KindGuard, MemoryChatGuards, MemoryChatStart, NoticeLevel};
use harness_business::process::ExecSpec;
use pretty_assertions::assert_eq;

const KEY: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

/// How the chat starts besides its scenario: whether it has a memory identity, and whether
/// `CODEX_HOME` is relative (`codex`) or absolute (in the test home).
#[derive(Default)]
struct Case {
    no_memory: bool,
    relative_home: Option<bool>,
}

async fn start(scenario: &str, case: Case) -> Run {
    let environment = |env: Env, _project: &Path| {
        // The app's own reading of the user's settings is not what counts: Codex's answer is.
        write_file(
            &env.home.join(".codex/config.toml"),
            "[mcp_servers.harness-memory]\ncommand = \"harness-memory\"\n",
        );

        let local = env.home.to_string_lossy().into_owned();
        let env = env.with_var("LOCALAPPDATA", local);

        match case.relative_home {
            Some(true) => env.with_var("CODEX_HOME", "codex"),
            Some(false) => {
                let home = env.home.join(".codex").to_string_lossy().into_owned();
                env.with_var("CODEX_HOME", home)
            }
            None => env,
        }
    };

    start_at(HarnessId::Codex, "codex", scenario, options(), environment, |request| {
        request.memory_chat = (!case.no_memory).then(|| MemoryChatStart {
            key: KEY.to_owned(),
            guards: MemoryChatGuards {
                paused: false,
                notes: KindGuard { on: true, ask: true },
                scripts: KindGuard { on: true, ask: false },
            },
        });
    })
    .await
    .expect("the chat starts")
}

fn spawned(run: &Run) -> ExecSpec {
    run.spawner.requested()[0].clone()
}

/// The launch names no MCP server on the command line.
fn assert_no_server_args(run: &Run) {
    let named: Vec<String> = spawned(run)
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .filter(|arg| arg.contains("mcp_servers"))
        .collect();

    assert!(named.is_empty(), "{named:?}");
}

/// The warnings the chat showed about its memory settings.
fn warnings(run: &Run) -> Vec<String> {
    run.events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Notice {
                level: NoticeLevel::Warning,
                message,
            } if message.contains("memory") => Some(message),
            _ => None,
        })
        .collect()
}

/// Stops the chat: the scripted Codex exits 0 only when every expected frame matched.
async fn stop(run: &Run) {
    run.manager.stop(run.key()).await.unwrap();

    let exited = run
        .wait_until("exited", |events| {
            events.iter().any(|event| matches!(event, AgentEvent::Exited { .. }))
        })
        .await;
    let clean = exited
        .iter()
        .any(|event| matches!(event, AgentEvent::Exited { code: Some(0), .. }));
    assert!(clean, "the scripted Codex saw what it expected");
}

#[tokio::test]
async fn the_threads_pass_the_chat_key_to_the_memory_servers_codex_lists() {
    let run = start("codex_memory_guard.jsonl", Case::default()).await;

    assert!(run.info.memory_guards, "the chat's own guards apply");
    assert_no_server_args(&run);
    let spec = spawned(&run);
    assert!(spec.env.iter().any(|(name, value)| name == SESSION_VAR && value == KEY));
    stop(&run).await;
    assert!(warnings(&run).is_empty(), "{:?}", warnings(&run));
}

#[tokio::test]
async fn a_server_that_passes_the_key_on_already_needs_nothing() {
    let run = start("codex_memory_listed.jsonl", Case::default()).await;

    assert!(run.info.memory_guards);
    assert_no_server_args(&run);

    stop(&run).await;
}

#[tokio::test]
async fn without_a_memory_server_codex_lists_the_guards_are_not_offered() {
    let none = start("codex_memory_none.jsonl", Case::default()).await;
    assert!(!none.info.memory_guards, "no server gets the key");
    stop(&none).await;
    assert!(warnings(&none).is_empty(), "nothing to tell without a memory server");

    let unread = start("codex_memory_unread.jsonl", Case::default()).await;
    assert!(!unread.info.memory_guards, "Codex did not say which servers it runs");
    stop(&unread).await;
}

#[tokio::test]
async fn a_memory_server_that_cannot_be_told_gets_a_notice() {
    let run = start("codex_memory_missed.jsonl", Case::default()).await;

    assert!(!run.info.memory_guards);
    run.wait_until("the notice", |events| {
        events.iter().any(|event| {
            matches!(event, AgentEvent::Notice { level: NoticeLevel::Warning, message } if message.contains("memory"))
        })
    })
    .await;
    let shown = warnings(&run);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert!(
        shown[0].contains("team.memory") && shown[0].contains("harness-memory") && !shown[0].contains(';'),
        "{}",
        shown[0]
    );

    stop(&run).await;
}

#[tokio::test]
async fn a_chat_without_a_memory_identity_asks_codex_nothing() {
    // The scenario answers no config/read: a chat that asked would not start in time.
    let run = start(
        "codex_engine_open.jsonl",
        Case {
            no_memory: true,
            ..Case::default()
        },
    )
    .await;

    assert!(!run.info.memory_guards);
    assert_no_server_args(&run);

    stop(&run).await;
}

#[tokio::test]
async fn a_relative_codex_home_is_not_passed_on() {
    let relative = start(
        "codex_engine_open.jsonl",
        Case {
            relative_home: Some(true),
            no_memory: true,
        },
    )
    .await;
    assert!(
        spawned(&relative)
            .env_remove_prefixes
            .contains(&OsString::from("CODEX_HOME"))
    );
    stop(&relative).await;

    let absolute = start(
        "codex_engine_open.jsonl",
        Case {
            relative_home: Some(false),
            no_memory: true,
        },
    )
    .await;
    assert!(
        !spawned(&absolute)
            .env_remove_prefixes
            .contains(&OsString::from("CODEX_HOME"))
    );
    stop(&absolute).await;
}
