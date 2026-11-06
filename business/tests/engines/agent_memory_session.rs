//! A chat with a memory identity: its guards are written to Harness Memory's data folder before
//! the harness starts, and the harness (OMP, PI, OpenCode, Claude Code) gets its key as
//! `HARNESS_MEMORY_SESSION`, which its MCP servers inherit. Any other launch has the variable
//! removed, and a key that is not a lowercase UUID starts nothing. The chat file names the project
//! root of the chat (its git root, else the folder, as a turned-off folder is listed); a chat
//! whose file cannot be written (no data folder) gets no variable. The test home holds the data
//! folder on every platform (on Windows through `LOCALAPPDATA`).

use crate::agent_support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use agent_support::{Run, options, start_in};
use harness_business::BusinessError;
use harness_business::ecosystem::{
    KNOWLEDGE_SESSION_VAR, Pin, SESSION_VAR, memory_data_dir, read_chat, read_chat_repo, update_global,
};
use harness_business::env::{Env, OsKind};
use harness_business::model::{
    AgentStartRequest, HarnessId, KindGuard, MemoryChatGuards, MemoryChatStart, MemoryGuardKind, MemoryGuardsPatch,
};
use harness_business::process::ExecSpec;
use pretty_assertions::assert_eq;

pub(crate) const KEY: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

fn guards() -> MemoryChatGuards {
    MemoryChatGuards {
        paused: false,
        notes: KindGuard { on: false, ask: false },
        scripts: KindGuard { on: true, ask: true },
    }
}

pub(crate) fn memory_chat(key: &str) -> MemoryChatStart {
    MemoryChatStart {
        key: key.to_owned(),
        guards: guards(),
    }
}

/// `env` with Windows' local data folder in its home, so Harness Memory's data folder is known on
/// every platform (`LOCALAPPDATA` counts only on Windows).
pub(crate) fn with_data_folder(env: Env) -> Env {
    let local = env.home.to_string_lossy().into_owned();

    env.with_var("LOCALAPPDATA", local)
}

/// [`with_data_folder`], and the folder exists: Harness Memory is in use on this computer.
pub(crate) fn with_memory(env: Env) -> Env {
    let env = with_data_folder(env);
    std::fs::create_dir_all(memory_data_dir(&env).unwrap()).unwrap();

    env
}

/// Harness Memory's data folder of `run`.
pub(crate) fn data_folder(run: &Run) -> PathBuf {
    memory_data_dir(&with_data_folder(Env::for_tests(run.home.path()))).expect("a data folder")
}

/// The value `spec` sets for `name`.
fn env_value(spec: &ExecSpec, name: &str) -> Option<String> {
    spec.env
        .iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

/// The one spawn of `run`.
fn spawned(run: &Run) -> ExecSpec {
    let requested = run.spawner.requested();
    assert_eq!(requested.len(), 1);

    requested[0].clone()
}

/// `run` was spawned without the session variable, an inherited one removed.
fn assert_no_session_variable(run: &Run) {
    let spec = spawned(run);

    assert_eq!(env_value(&spec, SESSION_VAR), None);
    assert!(spec.env_remove_prefixes.contains(&OsString::from(SESSION_VAR)));
    assert_eq!(env_value(&spec, KNOWLEDGE_SESSION_VAR), None);
    assert!(
        spec.env_remove_prefixes
            .contains(&OsString::from(KNOWLEDGE_SESSION_VAR))
    );
}

/// Starts `scenario` with Harness Memory's data folder known ([`with_data_folder`]).
pub(crate) async fn start_with(
    harness: HarnessId,
    dialect: &'static str,
    scenario: &str,
    customize: impl FnOnce(&mut AgentStartRequest),
) -> harness_business::Result<Run> {
    start_in(harness, dialect, scenario, options(), with_data_folder, customize).await
}

async fn start(harness: HarnessId, dialect: &'static str, scenario: &str, key: Option<&str>) -> Run {
    start_with(harness, dialect, scenario, |request| {
        request.memory_chat = key.map(memory_chat);
    })
    .await
    .expect("the chat starts")
}

#[tokio::test]
async fn every_chat_harness_gets_the_chat_key_and_its_guards_first() {
    for (harness, dialect, scenario) in [
        (HarnessId::Omp, "omp", "omp_basic.jsonl"),
        (HarnessId::Pi, "pi", "pi_basic.jsonl"),
        (HarnessId::OpenCode, "opencode", "opencode_hang.jsonl"),
        (HarnessId::Claude, "claude", "claude_init.jsonl"),
    ] {
        let run = start(harness, dialect, scenario, Some(KEY)).await;
        let spec = spawned(&run);

        assert_eq!(env_value(&spec, SESSION_VAR).as_deref(), Some(KEY), "{harness:?}");
        assert_eq!(
            env_value(&spec, KNOWLEDGE_SESSION_VAR).as_deref(),
            Some(KEY),
            "Harness Knowledge learns the chat too ({harness:?})"
        );
        assert!(
            spec.env_remove_prefixes.contains(&OsString::from(SESSION_VAR)),
            "an inherited value is replaced ({harness:?})"
        );

        let data = data_folder(&run);
        assert_eq!(read_chat(&data, KEY).unwrap(), Some(guards()), "{harness:?}");
        assert!(run.info.memory_guards, "the chat's own guards apply ({harness:?})");

        run.manager.stop(run.key()).await.unwrap();
    }
}

#[tokio::test]
async fn a_chat_without_a_memory_identity_never_passes_one_on() {
    let run = start(HarnessId::Omp, "omp", "omp_basic.jsonl", None).await;
    assert_no_session_variable(&run);
    assert!(!run.info.memory_guards);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_helper_chat_has_no_memory_identity() {
    let run = start_with(HarnessId::Omp, "omp", "omp_basic.jsonl", |request| {
        request.utility = true;
        request.memory_chat = Some(memory_chat(KEY));
    })
    .await
    .expect("the helper starts");

    assert_eq!(env_value(&spawned(&run), SESSION_VAR), None);
    assert!(!run.info.memory_guards);

    let data = data_folder(&run);
    assert_eq!(read_chat(&data, KEY).unwrap(), None, "no guards written");

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_key_that_is_not_a_lowercase_uuid_starts_nothing() {
    for key in ["3B2F6C1E-8A4D-4E5F-9A0B-1C2D3E4F5A6B", "../../escape", ""] {
        let error = start_with(HarnessId::Pi, "pi", "pi_basic.jsonl", |request| {
            request.memory_chat = Some(memory_chat(key));
        })
        .await
        .err()
        .expect("refused");

        assert!(matches!(error, BusinessError::InvalidInput(_)), "{key:?}: {error:?}");
    }
}

#[tokio::test]
async fn the_chat_file_names_the_git_root_of_the_project() {
    let run = start_with(HarnessId::Omp, "omp", "omp_basic.jsonl", |request| {
        std::fs::create_dir(Path::new(&request.project).join(".git")).unwrap();
        request.memory_chat = Some(memory_chat(KEY));
    })
    .await
    .expect("the chat starts");

    let data = data_folder(&run);
    let root = dunce::canonicalize(run.project.path()).unwrap();

    assert_eq!(
        read_chat_repo(&data, KEY).unwrap(),
        Some(root.to_string_lossy().into_owned())
    );

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_chat_outside_a_repository_names_its_folder_as_turned_on() {
    let run = start(HarnessId::Omp, "omp", "omp_basic.jsonl", Some(KEY)).await;
    let data = data_folder(&run);

    let turn_on = MemoryGuardsPatch {
        kind: MemoryGuardKind::Notes,
        paused: None,
        every_project: None,
        add_repo: Some(run.project.path().to_string_lossy().into_owned()),
        remove_repo: None,
    };
    let on_repos = update_global(&data, &turn_on, Some(Pin(None))).unwrap().notes.on_repos;

    assert_eq!(read_chat(&data, KEY).unwrap(), Some(guards()));
    assert_eq!(on_repos.len(), 1);
    assert_eq!(read_chat_repo(&data, KEY).unwrap().as_ref(), Some(&on_repos[0]));

    run.manager.stop(run.key()).await.unwrap();
}

pub(crate) fn global_file(data: &Path) -> PathBuf {
    data.join("guards").join("global.json")
}

/// `with_data_folder` and a `guards/global.json` holding `text` before the chat starts.
fn with_global(text: &'static str) -> impl FnOnce(Env) -> Env {
    move |env| {
        let env = with_data_folder(env);
        let data = memory_data_dir(&env).unwrap();

        std::fs::create_dir_all(data.join("guards")).unwrap();
        std::fs::write(global_file(&data), text).unwrap();

        env
    }
}

#[tokio::test]
async fn a_chat_start_creates_the_opt_in_guards_file_on_a_fresh_data_folder() {
    let run = start(HarnessId::Omp, "omp", "omp_basic.jsonl", Some(KEY)).await;
    let data = data_folder(&run);

    let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(global_file(&data)).unwrap()).unwrap();
    assert_eq!(
        file,
        serde_json::json!({
            "version": 2,
            "notes": { "paused": false, "everyProject": false, "onRepos": [], "repoKeys": {} },
            "scripts": { "paused": false, "everyProject": false, "onRepos": [], "repoKeys": {} },
        })
    );

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_chat_start_moves_a_version_one_file_and_leaves_an_unreadable_one() {
    let version_one =
        r#"{"version":1,"notes":{"paused":true,"offRepos":["/x"]},"scripts":{"paused":false,"offRepos":[]}}"#;
    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        with_global(version_one),
        |request| request.memory_chat = Some(memory_chat(KEY)),
    )
    .await
    .expect("the chat starts");

    let file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(global_file(&data_folder(&run))).unwrap()).unwrap();
    assert_eq!(file["version"], 2);
    assert_eq!(file["notes"]["paused"], true);
    assert_eq!(file["notes"]["onRepos"], serde_json::json!([]));
    run.manager.stop(run.key()).await.unwrap();

    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        with_global("not json"),
        |request| request.memory_chat = Some(memory_chat(KEY)),
    )
    .await
    .expect("the chat starts: the server refuses saves against that file");

    assert_eq!(
        std::fs::read_to_string(global_file(&data_folder(&run))).unwrap(),
        "not json"
    );
    run.manager.stop(run.key()).await.unwrap();
}

pub(crate) fn marker_file(data: &Path) -> PathBuf {
    data.join("guards").join("opt-in")
}

#[tokio::test]
async fn any_agent_start_keeps_the_guards_current_a_routine_run_included() {
    let version_one =
        r#"{"version":1,"notes":{"paused":true,"offRepos":["/x"]},"scripts":{"paused":false,"offRepos":[]}}"#;

    // No memory identity, as in a routine's run.
    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        with_global(version_one),
        |request| request.memory_chat = None,
    )
    .await
    .expect("the agent starts");

    let data = data_folder(&run);
    let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(global_file(&data)).unwrap()).unwrap();
    assert_eq!(file["version"], 2);
    assert_eq!(file["notes"]["paused"], true);
    assert_eq!(std::fs::read_to_string(marker_file(&data)).unwrap(), r#"{"version":1}"#);
    assert_no_session_variable(&run);
    run.manager.stop(run.key()).await.unwrap();

    // A helper agent too.
    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        with_memory,
        |request| {
            request.utility = true;
        },
    )
    .await
    .expect("the helper starts");
    assert!(marker_file(&data_folder(&run)).exists());
    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_chat_without_a_written_chat_file_gets_no_session_variable() {
    // Windows without `LOCALAPPDATA`: Harness Memory's data folder is unknown.
    let windows = |mut env: Env| {
        env.os = OsKind::Windows;
        env
    };

    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        windows,
        |request| {
            request.memory_chat = Some(memory_chat(KEY));
        },
    )
    .await
    .expect("the chat starts");

    assert_no_session_variable(&run);

    run.manager.stop(run.key()).await.unwrap();
}
