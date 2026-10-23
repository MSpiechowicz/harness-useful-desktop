//! Project trust of Codex chats: trust once trusts the project for that launch only (a quoted key
//! on the command line, nothing written), a path that cannot be quoted is refused, and a chat in
//! an untrusted folder with its own `.codex` layer starts with a notice that Codex ignores it.

use crate::codex_engine_support;
use crate::common;

use std::path::{Path, PathBuf};

use codex_engine_support::{CodexChat, HOME, Setup, start_with};
use common::write_file;
use harness_business::env::Env;
use harness_business::harness::{Harness, LaunchRequest};
use harness_business::harnesses::Codex;
use harness_business::harnesses::codex::{APP_SERVER_ARGS, ProjectTrust, project_trust};
use harness_business::model::{AgentEvent, IpcErrorCode, NoticeLevel};
use pretty_assertions::assert_eq;

fn trusting_once() -> Setup {
    Setup {
        trust_once: true,
        ..Setup::default()
    }
}

/// A project with its own Codex settings.
fn project_settings(project: &Path) {
    write_file(&project.join(".codex/config.toml"), "model = \"gpt-5.6-terra\"\n");
}

/// The user's `config.toml` trusting `project`.
fn user_trusts(project: &Path) {
    let entry = format!("[projects.'{}']\ntrust_level = \"trusted\"\n", project.display());

    write_file(&project.join(HOME).join("config.toml"), &entry);
}

/// The user's `config.toml` under `home` trusting `project`.
fn user_trusts_in(home: &Path, project: &Path) {
    let entry = format!("[projects.'{}']\ntrust_level = \"trusted\"\n", project.display());

    write_file(&home.join(HOME).join("config.toml"), &entry);
}

/// The notices of a chat once it opened, without the warnings about its sandbox (the recorded
/// thread may write in an extra root and reach the network).
async fn notices(chat: &CodexChat) -> Vec<(NoticeLevel, String)> {
    chat.wait("the commands", |event| {
        matches!(event, AgentEvent::CommandsUpdated { .. })
    })
    .await;

    chat.events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Notice { level, message } if level != NoticeLevel::Warning => Some((level, message)),
            _ => None,
        })
        .collect()
}

fn args(chat: &CodexChat) -> Vec<String> {
    let spec = chat.spawner.requested().remove(0);

    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[tokio::test]
async fn an_untrusted_project_with_its_own_settings_opens_with_a_notice() {
    let chat = start_with("codex_engine_open.jsonl", Setup::default(), project_settings).await;

    let notices = notices(&chat).await;
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].0, NoticeLevel::Info);
    assert!(
        notices[0].1.contains("ignores the project's .codex settings"),
        "{}",
        notices[0].1
    );
    assert_eq!(args(&chat), APP_SERVER_ARGS, "nothing is trusted unasked");

    chat.finish().await;
}

#[tokio::test]
async fn project_rules_alone_count_as_settings_codex_ignores() {
    let rules = |project: &Path| write_file(&project.join(".codex/rules/default.rules"), "");
    let chat = start_with("codex_engine_open.jsonl", Setup::default(), rules).await;

    assert_eq!(notices(&chat).await.len(), 1);
    chat.finish().await;
}

#[tokio::test]
async fn trust_once_trusts_the_project_for_this_launch_only() {
    let chat = start_with("codex_engine_open.jsonl", trusting_once(), project_settings).await;

    let key = chat.project.to_string_lossy().replace('\\', "\\\\");
    let mut expected: Vec<String> = APP_SERVER_ARGS.map(str::to_owned).to_vec();
    expected.extend([
        "-c".to_owned(),
        format!("projects={{\"{key}\"={{trust_level=\"trusted\"}}}}"),
    ]);
    assert_eq!(args(&chat), expected);
    assert_eq!(notices(&chat).await, []);
    assert!(
        !chat.project.join(HOME).join("config.toml").exists(),
        "nothing is written"
    );

    chat.finish().await;
}

#[tokio::test]
async fn a_project_the_user_trusts_or_without_settings_opens_without_a_notice() {
    let trusted = |project: &Path| {
        project_settings(project);
        user_trusts(project);
    };

    for chat in [
        start_with("codex_engine_open.jsonl", Setup::default(), trusted).await,
        start_with("codex_engine_open.jsonl", Setup::default(), |_| {}).await,
    ] {
        assert_eq!(notices(&chat).await, []);
        chat.finish().await;
    }
}

#[test]
fn the_codex_home_is_not_a_project_layer() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    project_settings(project);

    assert_eq!(
        project_trust(&project.join(".codex"), project),
        ProjectTrust::NothingToTrust
    );
    assert_eq!(project_trust(&project.join(HOME), project), ProjectTrust::Untrusted);
}

#[test]
fn trust_once_is_offered_only_for_an_untrusted_project_layer() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path()).with_var("CODEX_HOME", dir.path().join(HOME).to_string_lossy());
    let project = dir.path().join("project");
    let offered = |project: &Path| Codex.trust_once_offered(&env, project);

    assert!(!offered(&project), "no .codex folder, nothing to trust");

    write_file(&project.join(".codex/notes.md"), "not a layer file\n");
    assert!(!offered(&project), "a .codex folder without settings, hooks or rules");

    project_settings(&project);
    assert!(offered(&project));

    user_trusts_in(dir.path(), &project);
    assert!(!offered(&project), "the user's config.toml already trusts the folder");

    let rules = dir.path().join("rules-only");
    write_file(&rules.join(".codex/rules/default.rules"), "# rules\n");
    assert!(offered(&rules));
}

/// The launch of a chat in `project` that trusts it once.
fn trust_once_launch(project: &Path) -> harness_business::Result<Vec<String>> {
    let binary = crate::agent_support::binary();
    let request = LaunchRequest {
        binary: &binary,
        project,
        resume: None,
        approval_override: None,
        trust_once: true,
        utility: false,
    };

    let spec = Codex.launch_spec(&request)?;

    Ok(spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect())
}

#[cfg(unix)]
#[test]
fn quotes_and_backslashes_in_the_path_are_escaped() {
    let args = trust_once_launch(Path::new(r#"/work/my "app"\v2.0"#)).expect("a quotable path");

    assert_eq!(
        args.last().map(String::as_str),
        Some(r#"projects={"/work/my \"app\"\\v2.0"={trust_level="trusted"}}"#)
    );
}

#[test]
fn a_path_that_cannot_be_quoted_is_refused() {
    let absolute = std::env::temp_dir().join("line\nbreak");

    for project in [absolute, PathBuf::from("relative/project")] {
        let refused = trust_once_launch(&project).unwrap_err();
        assert_eq!(refused.code(), IpcErrorCode::InvalidInput, "{project:?}");
    }
}
