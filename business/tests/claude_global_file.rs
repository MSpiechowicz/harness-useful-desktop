//! Claude Code's global file (`~/.claude.json`): only its allow-listed readers open it, up to
//! 32 MiB, and a larger one is reported, never read as empty. Every other read refuses the name,
//! and a project's settings that lead to it (or anywhere outside the project) are not read.

mod common;
mod setup_support;

use std::path::Path;

use common::{CANARY, json_text, write_file};
use harness_business::BusinessError;
use harness_business::ecosystem::setup_status;
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::harnesses::Claude;
use harness_business::harnesses::claude::global_config_file;
use harness_business::model::{ConfigSection, HarnessId, McpRefusal, McpState, NoticeLevel};
use harness_business::security::SafeReader;
use pretty_assertions::assert_eq;
use serde_json::json;
use setup_support::CountingRunner;

fn stdio(command: &str) -> serde_json::Value {
    json!({ "type": "stdio", "command": command, "args": ["mcp"] })
}

async fn setup_mcp(env: &Env) -> McpState {
    let setup = setup_status(env, &CountingRunner::default(), &HarnessRegistry::with_defaults(), &[]).await;

    setup
        .harnesses
        .into_iter()
        .find(|entry| entry.harness == HarnessId::Claude)
        .expect("Claude Code")
        .mcp
}

/// The config view's section of the user's MCP servers in the global file.
fn global_section(env: &Env) -> ConfigSection {
    let view = Claude.config_view(env, None).unwrap();

    view.sections
        .into_iter()
        .find(|section| section.id == "user.globalFile")
        .expect("the global file's section")
}

/// A global file of about 3 MiB, as a long-used machine has: every project's state in it.
fn large_global_file(home: &Path) {
    let projects: serde_json::Map<String, serde_json::Value> = (0..3000)
        .map(|index| (format!("/w/project-{index}"), json!({ "history": ["x".repeat(1000)] })))
        .collect();
    let text = json!({
        "mcpServers": { "harness-dashboard": stdio("harness-dashboard") },
        "projects": projects,
    })
    .to_string();
    assert!(text.len() > 2 * 1024 * 1024);

    write_file(&home.join(".claude.json"), &text);
}

#[tokio::test]
async fn a_global_file_over_2_mib_is_read_by_every_reader() {
    let home = tempfile::tempdir().unwrap();
    large_global_file(home.path());
    let env = Env::for_tests(home.path());

    let section = global_section(&env);
    assert_eq!(section.error, None);
    assert!(section.entries.iter().any(|field| field.key == "mcpServers"));

    let extensions = Claude.extensions_view(&env, None).unwrap();
    assert!(
        extensions
            .mcp_servers
            .iter()
            .any(|server| server.name == "harness-dashboard")
    );
    assert!(extensions.notices.is_empty(), "{:?}", extensions.notices);

    assert_eq!(
        setup_mcp(&env).await,
        McpState::Configured {
            file: "~/.claude.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
}

#[tokio::test]
async fn a_global_file_over_32_mib_is_reported_as_too_large_and_never_as_empty() {
    let home = tempfile::tempdir().unwrap();
    let file = std::fs::File::create(home.path().join(".claude.json")).unwrap();
    file.set_len(33 * 1024 * 1024).unwrap();
    let env = Env::for_tests(home.path());

    let section = global_section(&env);
    assert_eq!(
        section.error.as_deref(),
        Some("The file is larger than 32 MiB, so it is not read")
    );

    let extensions = Claude.extensions_view(&env, None).unwrap();
    let notice = extensions
        .notices
        .iter()
        .find(|notice| notice.message.contains(".claude.json"));
    assert_eq!(notice.map(|notice| notice.level), Some(NoticeLevel::Warning));
    assert!(notice.unwrap().message.contains("larger than 32 MiB"));

    assert_eq!(
        setup_mcp(&env).await,
        McpState::Refused {
            file: "~/.claude.json".to_owned(),
            reason: McpRefusal::TooLarge
        }
    );
}

#[test]
fn only_the_global_file_itself_opens_for_its_readers() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let global = global_config_file(&env);
    write_file(&global, &json!({ "userID": CANARY }).to_string());

    assert!(SafeReader::read_claude_global_file(&global, 1024).is_ok());
    assert!(matches!(
        SafeReader::read_to_string(&global, 1024),
        Err(BusinessError::Forbidden(_))
    ));

    // Another file of that name, as in a dotfiles repository, is not the global file.
    let other = home.path().join("dotfiles/.claude.json");
    write_file(&other, "{}");
    assert!(matches!(
        SafeReader::read_claude_global_file(&other, 1024),
        Err(BusinessError::Forbidden(_))
    ));

    // Its backups stay refused to every reader.
    let backup = home.path().join(".claude.json.backup.1760000000000");
    write_file(&backup, "{}");
    assert!(matches!(
        SafeReader::read_claude_global_file(&backup, 1024),
        Err(BusinessError::Forbidden(_))
    ));
}

/// A project whose `.claude/settings.json` is a symlink to the user's global file, directly or to
/// the dotfiles copy the global file itself links to.
#[cfg(unix)]
#[test]
fn project_settings_linked_to_the_global_file_show_an_error_and_no_fields() {
    let home = tempfile::tempdir().unwrap();
    let dotfiles = home.path().join("dotfiles/claude.json");
    write_file(
        &dotfiles,
        &json!({ "oauthAccount": { "emailAddress": CANARY }, "userID": CANARY }).to_string(),
    );
    std::os::unix::fs::symlink(&dotfiles, home.path().join(".claude.json")).unwrap();
    let env = Env::for_tests(home.path());

    for target in [home.path().join(".claude.json"), dotfiles.clone()] {
        let project = tempfile::tempdir().unwrap();
        let settings = project.path().join(".claude/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, &settings).unwrap();
        let project = dunce::canonicalize(project.path()).unwrap();

        let view = Claude.config_view(&env, Some(&project)).unwrap();
        let section = view
            .sections
            .iter()
            .find(|section| section.id == "project.settings")
            .unwrap();

        assert!(section.error.is_some(), "{}", target.display());
        assert!(section.entries.is_empty(), "{}", target.display());
        assert!(!json_text(&view).contains("oauthAccount"));
        assert!(!json_text(&view).contains(CANARY));
    }
}

/// The project's `.mcp.json` and settings that resolve outside the project add nothing to the
/// extensions view.
#[cfg(unix)]
#[test]
fn project_files_outside_the_project_add_no_servers() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = home.path().join("elsewhere");
    write_file(
        &elsewhere.join("mcp.json"),
        &json!({ "mcpServers": { "outside": stdio("/x/outside") } }).to_string(),
    );
    write_file(
        &elsewhere.join("settings.json"),
        &json!({ "enableAllProjectMcpServers": true }).to_string(),
    );

    let project = home.path().join("project");
    std::fs::create_dir_all(project.join(".claude")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("mcp.json"), project.join(".mcp.json")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("settings.json"), project.join(".claude/settings.json")).unwrap();
    let project = dunce::canonicalize(&project).unwrap();

    let view = Claude
        .extensions_view(&Env::for_tests(home.path()), Some(&project))
        .unwrap();

    assert!(view.mcp_servers.is_empty(), "{:?}", view.mcp_servers);
}
