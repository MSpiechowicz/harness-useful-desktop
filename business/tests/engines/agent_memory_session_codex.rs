//! A Codex chat with a memory identity (plan decision 11): Codex starts its MCP servers with a
//! cleared environment, so the launch names `HARNESS_MEMORY_SESSION` among the variables of each
//! harness-memory server the chat's settings have (`-c mcp_servers.<name>.env_vars=[…]`, its own
//! variables kept), and of no other server. A server only an untrusted project names gets none
//! (unless the chat trusts the project once), nor does a chat without a memory identity. Every
//! Codex spawn drops a relative `CODEX_HOME`, which Codex would resolve in the project.

use crate::agent_support;
use crate::common;

use std::ffi::OsString;
use std::path::Path;

use agent_support::{Run, options, start_at};
use common::write_file;
use harness_business::ecosystem::SESSION_VAR;
use harness_business::env::Env;
use harness_business::model::{HarnessId, KindGuard, MemoryChatGuards, MemoryChatStart};
use harness_business::process::ExecSpec;
use pretty_assertions::assert_eq;

const KEY: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

/// What the test writes before the chat starts: the user's `config.toml`, the project's own
/// `.codex/config.toml`, and whether the chat trusts the project once and has a memory identity.
#[derive(Default)]
struct Case {
    user: &'static str,
    project: &'static str,
    trust_once: bool,
    no_memory: bool,
    /// `CODEX_HOME` relative (`codex`) or absolute (in the test home).
    relative_home: Option<bool>,
}

async fn start(case: Case) -> Run {
    let environment = |env: Env, project: &Path| {
        write_file(&env.home.join(".codex/config.toml"), case.user);

        if !case.project.is_empty() {
            write_file(&project.join(".codex/config.toml"), case.project);
        }

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

    start_at(
        HarnessId::Codex,
        "codex",
        "codex_engine_open.jsonl",
        options(),
        environment,
        |request| {
            request.trust_once = case.trust_once;
            request.memory_chat = (!case.no_memory).then(|| MemoryChatStart {
                key: KEY.to_owned(),
                guards: MemoryChatGuards {
                    paused: false,
                    notes: KindGuard { on: true, ask: true },
                    scripts: KindGuard { on: true, ask: false },
                },
            });
        },
    )
    .await
    .expect("the chat starts")
}

fn spawned(run: &Run) -> ExecSpec {
    run.spawner.requested()[0].clone()
}

/// The `-c` overrides of the launch that name a server's `env_vars`.
fn env_vars_overrides(run: &Run) -> Vec<String> {
    spawned(run)
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .filter(|arg| arg.starts_with("mcp_servers."))
        .collect()
}

async fn stop(run: Run) {
    run.manager.stop(run.key()).await.unwrap();
}

const MEMORY: &str = "[mcp_servers.harness-memory]\ncommand = \"/opt/bin/harness-memory\"\nargs = [\"mcp\"]\nenv_vars = [\"HOME_LANG\"]\n\n[mcp_servers.docs]\ncommand = \"docs-server\"\n";

#[tokio::test]
async fn the_memory_server_gets_the_chat_key_and_keeps_its_own_variables() {
    let run = start(Case {
        user: MEMORY,
        ..Case::default()
    })
    .await;

    assert_eq!(
        env_vars_overrides(&run),
        [format!(
            "mcp_servers.harness-memory.env_vars=[\"HOME_LANG\",\"{SESSION_VAR}\"]"
        )]
    );
    let spec = spawned(&run);
    assert!(spec.env.iter().any(|(name, value)| name == SESSION_VAR && value == KEY));

    stop(run).await;
}

#[tokio::test]
async fn a_server_running_harness_memory_under_another_name_gets_it_too() {
    let run = start(Case {
        user: "[mcp_servers.notes]\ncommand = 'C:\\Tools\\Harness-Memory.exe'\nargs = [\"mcp\"]\n",
        ..Case::default()
    })
    .await;

    assert_eq!(
        env_vars_overrides(&run),
        [format!("mcp_servers.notes.env_vars=[\"{SESSION_VAR}\"]")]
    );

    stop(run).await;
}

#[tokio::test]
async fn nothing_is_overridden_without_a_memory_server_or_a_memory_identity() {
    let none = start(Case {
        user: "[mcp_servers.docs]\ncommand = \"docs-server\"\n",
        ..Case::default()
    })
    .await;
    assert!(
        env_vars_overrides(&none).is_empty(),
        "a missing server would stop Codex"
    );
    stop(none).await;

    let no_memory = start(Case {
        user: MEMORY,
        no_memory: true,
        ..Case::default()
    })
    .await;
    assert!(env_vars_overrides(&no_memory).is_empty());
    stop(no_memory).await;

    let listed = start(Case {
        user: "[mcp_servers.harness-memory]\ncommand = \"harness-memory\"\nenv_vars = [\"HARNESS_MEMORY_SESSION\"]\n",
        ..Case::default()
    })
    .await;
    assert!(
        env_vars_overrides(&listed).is_empty(),
        "the server passes it on already"
    );
    stop(listed).await;
}

#[tokio::test]
async fn a_server_of_an_untrusted_project_counts_only_when_trusted_once() {
    let project = "[mcp_servers.harness-memory]\ncommand = \"harness-memory\"\nargs = [\"mcp\"]\n";

    let untrusted = start(Case {
        project,
        ..Case::default()
    })
    .await;
    assert!(
        env_vars_overrides(&untrusted).is_empty(),
        "Codex leaves that server out"
    );
    stop(untrusted).await;

    let trusted_once = start(Case {
        project,
        trust_once: true,
        ..Case::default()
    })
    .await;
    assert_eq!(
        env_vars_overrides(&trusted_once),
        [format!("mcp_servers.harness-memory.env_vars=[\"{SESSION_VAR}\"]")]
    );
    stop(trusted_once).await;
}

#[tokio::test]
async fn a_relative_codex_home_is_not_passed_on() {
    let relative = start(Case {
        relative_home: Some(true),
        no_memory: true,
        ..Case::default()
    })
    .await;
    assert!(
        spawned(&relative)
            .env_remove_prefixes
            .contains(&OsString::from("CODEX_HOME"))
    );
    stop(relative).await;

    let absolute = start(Case {
        relative_home: Some(false),
        no_memory: true,
        ..Case::default()
    })
    .await;
    assert!(
        !spawned(&absolute)
            .env_remove_prefixes
            .contains(&OsString::from("CODEX_HOME"))
    );
    stop(absolute).await;
}
