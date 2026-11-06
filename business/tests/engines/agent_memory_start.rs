//! Keeping Harness Memory's guards current when an agent starts (`keep_guards_current`): nothing
//! is created while the computer has no Harness Memory (no data folder, no server), and a failure
//! fails the start only with a memory chat or while a readable version 1 file remains, as an `io`
//! error with a sentence for the user, else it only logs a warning. A helper agent never fails.

#[cfg(unix)]
use crate::agent_memory_session::with_data_folder;
use crate::agent_memory_session::{KEY, data_folder, global_file, marker_file, memory_chat, start_with, with_memory};
#[cfg(unix)]
use crate::agent_support::start_when;
use crate::agent_support::{options, start_in};

use harness_business::BusinessError;
use harness_business::ecosystem::memory_data_dir;
use harness_business::env::Env;
use harness_business::model::HarnessId;

/// A data folder where the marker cannot be written: a folder stands in its place.
fn blocked(env: Env) -> Env {
    let env = with_memory(env);
    let data = memory_data_dir(&env).unwrap();
    std::fs::create_dir_all(marker_file(&data)).unwrap();

    env
}

const VERSION_ONE: &str =
    r#"{"version":1,"notes":{"paused":true,"offRepos":["/x"]},"scripts":{"paused":false,"offRepos":[]}}"#;

/// The text a failed start shows, with the code the UI shows as it is.
fn start_failure(error: &BusinessError) -> String {
    let BusinessError::Io { .. } = error else {
        panic!("code io, which the UI shows as it is: {error:?}");
    };

    error.to_string()
}

#[tokio::test]
async fn guards_that_cannot_be_kept_current_fail_a_chat_start_with_a_readable_message() {
    let error = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        blocked,
        |request| {
            request.memory_chat = Some(memory_chat(KEY));
        },
    )
    .await
    .err()
    .expect("the start fails");

    let message = start_failure(&error);
    assert!(
        message.starts_with("Harness Memory settings could not be updated: "),
        "{message}"
    );
    assert!(message.contains("saving the Harness Memory guards"), "{message}");
}

#[tokio::test]
async fn guards_that_cannot_be_kept_current_only_warn_without_a_memory_chat_or_for_a_helper() {
    // No memory chat and no readable version 1 file: nothing runs against stale rules.
    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        blocked,
        |request| {
            request.memory_chat = None;
        },
    )
    .await
    .expect("an agent with no memory chat starts");
    run.manager.stop(run.key()).await.unwrap();

    let run = start_in(
        HarnessId::Omp,
        "omp",
        "omp_basic.jsonl",
        options(),
        blocked,
        |request| {
            request.utility = true;
            request.memory_chat = Some(memory_chat(KEY));
        },
    )
    .await
    .expect("a helper saves nothing, so it starts");
    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_version_one_file_that_cannot_be_rewritten_fails_every_start_but_a_helpers() {
    let stale = |env: Env| {
        let env = blocked(env);
        std::fs::write(global_file(&memory_data_dir(&env).unwrap()), VERSION_ONE).unwrap();

        env
    };

    for memory_chat in [None, Some(memory_chat(KEY))] {
        let error = start_in(HarnessId::Omp, "omp", "omp_basic.jsonl", options(), stale, |request| {
            request.memory_chat = memory_chat.clone();
        })
        .await
        .err()
        .expect("the server would keep the deny list, so the start fails");

        assert!(
            start_failure(&error).starts_with("Harness Memory settings could not be updated: "),
            "{error}"
        );
    }

    let run = start_in(HarnessId::Omp, "omp", "omp_basic.jsonl", options(), stale, |request| {
        request.utility = true;
    })
    .await
    .expect("a helper saves nothing, so it starts");
    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn without_harness_memory_a_start_creates_nothing_and_cannot_fail() {
    // No data folder and no server: the user has no Harness Memory.
    let run = start_with(HarnessId::Omp, "omp", "omp_basic.jsonl", |request| {
        request.memory_chat = None
    })
    .await
    .expect("the agent starts");

    assert!(!data_folder(&run).exists(), "no folder was created");
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
    let message = start_failure(&error);

    assert!(
        message.starts_with("Harness Memory settings could not be updated: ")
            && message.contains(&format!("can write to the Harness Memory folder {folder}")),
        "{message}"
    );
    // The command quotes a folder with a space (macOS: `Application Support`) as one word.
    assert!(
        message.contains(&format!("chmod 700 {folder}")) || message.contains(&format!("chmod 700 '{folder}'")),
        "{message}"
    );
}

/// How the data folder is made impossible to use, with nothing written in it: no `global.json` and
/// no marker are left, and the server then applies no rules at all.
#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
enum Blocked {
    /// Other users can write to it (`0770`).
    Open,
    /// A symbolic link stands where it should be.
    Linked,
    /// A folder stands where the lock file belongs.
    Lock,
}

#[cfg(unix)]
fn blocked_without_files(how: Blocked) -> impl FnOnce(Env) -> Env {
    use std::os::unix::fs::PermissionsExt;

    move |env| {
        let env = with_memory(env);
        let data = memory_data_dir(&env).unwrap();

        match how {
            Blocked::Open => {
                std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o770)).unwrap();
            }
            Blocked::Linked => {
                let real = data.with_file_name("real data folder");
                std::fs::rename(&data, &real).unwrap();
                std::os::unix::fs::symlink(&real, &data).unwrap();
            }
            Blocked::Lock => {
                let guards = data.join("guards");
                std::fs::create_dir_all(guards.join("global.lock")).unwrap();
                std::fs::set_permissions(&guards, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }

        env
    }
}

#[cfg(unix)]
fn nothing_written(env_home: &std::path::Path) {
    let data = memory_data_dir(&with_data_folder(Env::for_tests(env_home))).unwrap();
    let real = data.with_file_name("real data folder");
    let data = if real.exists() { real } else { data };

    assert!(!global_file(&data).exists(), "no global.json");
    assert!(!marker_file(&data).exists(), "no marker");
}

#[cfg(unix)]
#[tokio::test]
async fn a_start_fails_when_neither_file_nor_marker_could_be_written_unless_it_is_a_helper_or_memory_is_off() {
    for how in [Blocked::Open, Blocked::Linked, Blocked::Lock] {
        // A routine run has no memory chat: the server would still apply no rules.
        let error = start_in(
            HarnessId::Omp,
            "omp",
            "omp_basic.jsonl",
            options(),
            blocked_without_files(how),
            |request| request.memory_chat = None,
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("{how:?}: the start fails"));

        let message = start_failure(&error);
        assert!(
            message.starts_with("Harness Memory settings could not be updated: "),
            "{how:?}: {message}"
        );

        // A helper saves nothing, so it starts.
        let run = start_in(
            HarnessId::Omp,
            "omp",
            "omp_basic.jsonl",
            options(),
            blocked_without_files(how),
            |request| {
                request.utility = true;
                request.memory_chat = None;
            },
        )
        .await
        .unwrap_or_else(|error| panic!("{how:?}: {error}"));
        nothing_written(run.home.path());
        run.manager.stop(run.key()).await.unwrap();

        // With Project memory off the app removed its entry, so no harness runs the server: a
        // warning, not a failure (see the test below for a server that stays configured).
        let run = start_when(
            false,
            HarnessId::Omp,
            "omp",
            "omp_basic.jsonl",
            options(),
            |env, _| blocked_without_files(how)(env),
            |request| request.memory_chat = None,
        )
        .await
        .unwrap_or_else(|error| panic!("{how:?}: {error}"));
        nothing_written(run.home.path());
        run.manager.stop(run.key()).await.unwrap();
    }
}

/// An OpenCode config with comments (which the app never rewrites) that holds the memory server
/// when `entry`, next to a data folder where nothing can be written.
#[cfg(unix)]
fn commented_opencode_config(entry: bool) -> impl FnOnce(Env, &std::path::Path) -> Env {
    move |env, _| write_commented_opencode_config(blocked_without_files(Blocked::Lock)(env), entry)
}

/// Writes the commented OpenCode config of [`commented_opencode_config`] for `env`.
#[cfg(unix)]
fn write_commented_opencode_config(env: Env, entry: bool) -> Env {
    let registry = harness_business::harness::HarnessRegistry::with_defaults();
    let file = harness_business::ecosystem::mcp_target(&env, registry.get(HarnessId::OpenCode).unwrap()).unwrap();
    let servers = if entry {
        r#""harness-memory": { "type": "local", "command": ["/opt/harness-memory", "mcp"] }"#
    } else {
        r#""other": { "type": "local", "command": ["/opt/other", "mcp"] }"#
    };

    std::fs::create_dir_all(file.path.parent().unwrap()).unwrap();
    std::fs::write(&file.path, format!("// my own notes\n{{ \"mcp\": {{ {servers} }} }}\n")).unwrap();

    env
}

#[cfg(unix)]
#[tokio::test]
async fn with_project_memory_off_a_start_fails_only_while_the_harness_still_lists_the_memory_server() {
    // The config has comments, so switching Project memory off could not remove the entry: the
    // server still runs, and with no rules it would save anywhere.
    let error = start_when(
        false,
        HarnessId::OpenCode,
        "opencode",
        "opencode_hang.jsonl",
        options(),
        commented_opencode_config(true),
        |request| request.memory_chat = None,
    )
    .await
    .err()
    .expect("the server is still configured, so the start fails");

    let message = start_failure(&error);
    assert!(
        message.starts_with("Harness Memory settings could not be updated: "),
        "{message}"
    );

    // The same config without the entry: no harness runs the server, so the start continues.
    let run = start_when(
        false,
        HarnessId::OpenCode,
        "opencode",
        "opencode_hang.jsonl",
        options(),
        commented_opencode_config(false),
        |request| request.memory_chat = None,
    )
    .await
    .expect("nothing runs the server, so the start continues");
    run.manager.stop(run.key()).await.unwrap();
}

/// A version 1 file that cannot be moved (a folder stands where the marker belongs) and a commented
/// OpenCode config that holds the memory server when `entry`.
#[cfg(unix)]
fn stale_with_opencode_config(entry: bool) -> impl FnOnce(Env, &std::path::Path) -> Env {
    move |env, _| {
        let env = blocked(env);
        std::fs::write(global_file(&memory_data_dir(&env).unwrap()), VERSION_ONE).unwrap();

        write_commented_opencode_config(env, entry)
    }
}

#[cfg(unix)]
#[tokio::test]
async fn with_project_memory_off_a_version_one_file_that_remains_fails_a_start_only_while_the_server_is_listed() {
    // The server is still configured and reads the old deny list, with saving on.
    let error = start_when(
        false,
        HarnessId::OpenCode,
        "opencode",
        "opencode_hang.jsonl",
        options(),
        stale_with_opencode_config(true),
        |request| request.memory_chat = None,
    )
    .await
    .err()
    .expect("the listed server would apply the old deny list, so the start fails");

    let message = start_failure(&error);
    assert!(
        message.starts_with("Harness Memory settings could not be updated: "),
        "{message}"
    );

    // No file lists the server: nothing reads the old file, so the start continues.
    let run = start_when(
        false,
        HarnessId::OpenCode,
        "opencode",
        "opencode_hang.jsonl",
        options(),
        stale_with_opencode_config(false),
        |request| request.memory_chat = None,
    )
    .await
    .expect("nothing runs the server, so the start continues");
    run.manager.stop(run.key()).await.unwrap();
}
