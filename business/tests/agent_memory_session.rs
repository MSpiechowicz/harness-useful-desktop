//! A chat with a memory identity: its guards are written to Harness Memory's data folder before
//! the harness starts, and the harness (OMP, PI, OpenCode) gets its key as
//! `HARNESS_MEMORY_SESSION`, which its MCP servers inherit. Any other launch has the variable
//! removed, and a key that is not a lowercase UUID starts nothing. The chat file names the project
//! root of the chat (its git root, else the folder, as a turned-off folder is listed); a chat
//! whose file cannot be written (no data folder) gets no variable.

mod agent_support;

use std::ffi::OsString;
use std::path::Path;

use agent_support::{Run, options, start_in, start_with};
use harness_business::BusinessError;
use harness_business::ecosystem::{SESSION_VAR, memory_data_dir, read_chat, read_chat_repo, update_global};
use harness_business::env::{Env, OsKind};
use harness_business::model::{
    HarnessId, KindGuard, MemoryChatGuards, MemoryChatStart, MemoryGuardKind, MemoryGuardsPatch,
};
use harness_business::process::ExecSpec;
use pretty_assertions::assert_eq;

const KEY: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

fn guards() -> MemoryChatGuards {
    MemoryChatGuards {
        paused: false,
        notes: KindGuard { on: false, ask: false },
        scripts: KindGuard { on: true, ask: true },
    }
}

fn memory_chat(key: &str) -> MemoryChatStart {
    MemoryChatStart {
        key: key.to_owned(),
        guards: guards(),
    }
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
}

async fn start(harness: HarnessId, dialect: &'static str, scenario: &str, key: Option<&str>) -> Run {
    start_with(harness, dialect, scenario, options(), |request| {
        request.memory_chat = key.map(memory_chat);
    })
    .await
    .expect("the chat starts")
}

#[tokio::test]
async fn omp_pi_and_opencode_get_the_chat_key_and_its_guards_first() {
    for (harness, dialect, scenario) in [
        (HarnessId::Omp, "omp", "omp_basic.jsonl"),
        (HarnessId::Pi, "pi", "pi_basic.jsonl"),
        (HarnessId::OpenCode, "opencode", "opencode_hang.jsonl"),
    ] {
        let run = start(harness, dialect, scenario, Some(KEY)).await;
        let spec = spawned(&run);

        assert_eq!(env_value(&spec, SESSION_VAR).as_deref(), Some(KEY), "{harness:?}");
        assert!(
            spec.env_remove_prefixes.contains(&OsString::from(SESSION_VAR)),
            "an inherited value is replaced ({harness:?})"
        );

        let data = memory_data_dir(&Env::for_tests(run.home.path())).unwrap();
        assert_eq!(read_chat(&data, KEY).unwrap(), Some(guards()), "{harness:?}");

        run.manager.stop(run.key()).await.unwrap();
    }
}

#[tokio::test]
async fn a_chat_without_a_memory_identity_never_passes_one_on() {
    let run = start(HarnessId::Omp, "omp", "omp_basic.jsonl", None).await;
    assert_no_session_variable(&run);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_helper_chat_has_no_memory_identity() {
    let run = start_with(HarnessId::Omp, "omp", "omp_basic.jsonl", options(), |request| {
        request.utility = true;
        request.memory_chat = Some(memory_chat(KEY));
    })
    .await
    .expect("the helper starts");

    assert_eq!(env_value(&spawned(&run), SESSION_VAR), None);

    let data = memory_data_dir(&Env::for_tests(run.home.path())).unwrap();
    assert_eq!(read_chat(&data, KEY).unwrap(), None, "no guards written");

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_key_that_is_not_a_lowercase_uuid_starts_nothing() {
    for key in ["3B2F6C1E-8A4D-4E5F-9A0B-1C2D3E4F5A6B", "../../escape", ""] {
        let error = start_with(HarnessId::Pi, "pi", "pi_basic.jsonl", options(), |request| {
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
    let run = start_with(HarnessId::Omp, "omp", "omp_basic.jsonl", options(), |request| {
        std::fs::create_dir(Path::new(&request.project).join(".git")).unwrap();
        request.memory_chat = Some(memory_chat(KEY));
    })
    .await
    .expect("the chat starts");

    let data = memory_data_dir(&Env::for_tests(run.home.path())).unwrap();
    let root = dunce::canonicalize(run.project.path()).unwrap();

    assert_eq!(
        read_chat_repo(&data, KEY).unwrap(),
        Some(root.to_string_lossy().into_owned())
    );

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_chat_outside_a_repository_names_its_folder_as_turned_off() {
    let run = start(HarnessId::Omp, "omp", "omp_basic.jsonl", Some(KEY)).await;
    let data = memory_data_dir(&Env::for_tests(run.home.path())).unwrap();

    let turn_off = MemoryGuardsPatch {
        kind: MemoryGuardKind::Notes,
        paused: None,
        add_repo: Some(run.project.path().to_string_lossy().into_owned()),
        remove_repo: None,
    };
    let off_repos = update_global(&data, &turn_off).unwrap().notes.off_repos;

    assert_eq!(read_chat(&data, KEY).unwrap(), Some(guards()));
    assert_eq!(off_repos.len(), 1);
    assert_eq!(read_chat_repo(&data, KEY).unwrap().as_ref(), Some(&off_repos[0]));

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

#[cfg(unix)]
#[tokio::test]
async fn a_data_folder_other_users_can_write_to_fails_the_start_with_the_fix() {
    use std::os::unix::fs::PermissionsExt;

    let mut data = None;
    let open_data_folder = |env: Env| {
        let folder = memory_data_dir(&env).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o770)).unwrap();
        data = Some(folder);
        env
    };

    let error = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        open_data_folder,
        |request| {
            request.memory_chat = Some(memory_chat(KEY));
        },
    )
    .await
    .err()
    .expect("refused");

    let folder = data.unwrap().display().to_string();
    let BusinessError::InvalidInput(message) = &error else {
        panic!("a message the user sees: {error:?}");
    };

    assert!(
        message.contains(&format!("can write to the Harness Memory folder {folder}")),
        "{message}"
    );
    assert!(message.contains(&format!("chmod 700 {folder}")), "{message}");
}
