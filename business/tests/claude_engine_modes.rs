//! Approval modes of Claude Code chats: Always ask registers the ask-all hook in `initialize` and
//! answers its callbacks with `ask`, Write and Auto register none, Yolo is refused before anything spawns,
//! and the configured mode comes from `permissions.defaultMode` of the settings layers (managed
//! over local over project over user).

mod agent_support;
mod claude_engine_support;
mod common;

use std::path::Path;

use claude_engine_support::{marker, start_in, stop_cleanly};
use common::{env_at, write_file};
use harness_business::env::{Env, OsKind};
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{ApprovalMode, IpcErrorCode, NativeApprovalMode};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn always_ask_registers_the_ask_all_hook_and_answers_its_callbacks_with_ask() {
    let chat = start_in("claude_mode_ask_all.jsonl", Some(ApprovalMode::AlwaysAsk)).await;

    marker(&chat).await;
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn write_registers_no_hook() {
    let chat = start_in("claude_mode_write.jsonl", Some(ApprovalMode::Write)).await;

    marker(&chat).await;
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn auto_registers_no_hook() {
    let chat = start_in("claude_mode_write.jsonl", Some(ApprovalMode::Auto)).await;

    marker(&chat).await;
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn yolo_is_refused_before_anything_spawns() {
    let chat = start_in("claude_init.jsonl", Some(ApprovalMode::Yolo)).await;

    let refused = chat.session.as_ref().err().expect("no Yolo mode");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(chat.spawner.requested().is_empty());
}

/// A home with managed settings in reach on every OS (`%ProgramFiles%\ClaudeCode`).
fn managed_env(home: &Path) -> Env {
    env_at(home)
        .with_os(OsKind::Windows)
        .with_var("ProgramFiles", home.join("Program Files").to_string_lossy())
}

fn mode_file(path: &Path, mode: &str) {
    write_file(path, &format!(r#"{{"permissions": {{"defaultMode": "{mode}"}}}}"#));
}

fn configured(env: &Env, project: &Path) -> (Option<ApprovalMode>, NativeApprovalMode) {
    let native = Claude
        .native_approval_mode(env, Some(project))
        .expect("Claude Code names its mode");

    (Claude.configured_approval_mode(env, Some(project)), native)
}

fn native(name: &str, unattended: bool) -> NativeApprovalMode {
    NativeApprovalMode {
        name: name.to_owned(),
        unattended,
    }
}

#[test]
fn the_configured_mode_is_the_last_layer_that_sets_one_and_maps_where_the_app_has_a_mode() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let env = managed_env(home.path());

    let user = home.path().join(".claude/settings.json");
    let shared = project.path().join(".claude/settings.json");
    let local = project.path().join(".claude/settings.local.json");
    let managed = home.path().join("Program Files/ClaudeCode/managed-settings.json");

    assert_eq!(
        configured(&env, project.path()),
        (Some(ApprovalMode::Write), native("default", false)),
        "unset is Claude's default"
    );

    mode_file(&user, "manual");
    assert_eq!(
        configured(&env, project.path()),
        (Some(ApprovalMode::Write), native("default", false))
    );

    mode_file(&shared, "plan");
    assert_eq!(configured(&env, project.path()), (None, native("plan", false)));

    mode_file(&local, "bypassPermissions");
    assert_eq!(
        configured(&env, project.path()),
        (Some(ApprovalMode::Yolo), native("bypassPermissions", true))
    );

    mode_file(&managed, "auto");
    assert_eq!(
        configured(&env, project.path()),
        (Some(ApprovalMode::Auto), native("auto", true)),
        "managed settings win"
    );

    mode_file(&managed, "acceptEdits");
    assert_eq!(configured(&env, project.path()), (None, native("acceptEdits", false)));

    mode_file(&managed, "turbo");
    assert_eq!(
        configured(&env, project.path()),
        (Some(ApprovalMode::Yolo), native("bypassPermissions", true)),
        "an unknown mode is skipped"
    );

    assert_eq!(
        Claude.native_approval_mode(&env, None),
        Some(native("default", false)),
        "without a project only the user's layers count"
    );
}
