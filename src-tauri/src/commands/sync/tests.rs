use std::sync::atomic::{AtomicUsize, Ordering};

use harness_business::model::{IpcErrorCode, SyncItemStatus, SyncNote, SyncOutcome};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec};
use serde_json::{Value, json};

use super::*;
use crate::error::CommandError;
use crate::test_support::{TempDir, cache_detection, collecting_channel, home_state, omp_mcp_file};

const DOCS: &str = "omp:mcp:docs";

/// Counts the CLI runs and runs nothing.
#[derive(Default)]
struct CountingRunner(AtomicUsize);

impl CountingRunner {
    fn runs(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

impl ProcessRunner for CountingRunner {
    fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        self.0.fetch_add(1, Ordering::Relaxed);

        Box::pin(async { Err(BusinessError::Process("running is disabled in tests".to_owned())) })
    }
}

/// A state over `<dir>/home` where only OMP is installed, at a binary that must never run. Every
/// harness has a cached detection, so nothing is probed.
fn state(dir: &TempDir) -> AppState {
    let state = home_state(dir);
    let ids: Vec<HarnessId> = state.registry.all().map(|harness| harness.id()).collect();
    for id in ids {
        cache_detection(&state, id, None);
    }
    cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("missing-omp")));

    state
}

/// A setup file sharing one HTTP MCP server with OMP.
fn shared_docs_server() -> String {
    json!({
        "format": "harness-useful-desktop/setup",
        "version": 1,
        "harnesses": {
            "omp": { "mcpServers": [{ "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" }] }
        }
    })
    .to_string()
}

fn review(state: &AppState, text: String) -> CommandResult<SyncImportPlan> {
    block_on(async { preview(state, decode_text(text).await?).await })
}

fn stored_id(state: &AppState) -> Option<String> {
    let current = state.sync_plan.lock().expect("plan lock");

    current.as_ref().map(|plan| plan.id().to_owned())
}

fn apply_with(
    state: &AppState,
    runner: &Arc<CountingRunner>,
    plan_id: &str,
    confirmed: bool,
) -> CommandResult<SyncImportResult> {
    let (channel, _events) = collecting_channel();

    block_on(apply(
        state,
        runner.clone(),
        plan_id,
        vec![DOCS.to_owned()],
        confirmed,
        channel,
    ))
}

fn message(error: &CommandError) -> String {
    let value = serde_json::to_value(error).expect("serializable");

    value["message"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn a_review_is_kept_as_the_current_plan_and_a_newer_one_replaces_it() {
    let dir = TempDir::new("sync-review");
    let state = state(&dir);

    let first = review(&state, shared_docs_server()).expect("reviewed");

    let item = &first.sections[0].items[0];
    assert_eq!((first.sections[0].harness, item.id.as_str()), (HarnessId::Omp, DOCS));
    assert_eq!(item.status, SyncItemStatus::New);
    assert_eq!(stored_id(&state).as_deref(), Some(first.plan_id.as_str()));

    let second = review(&state, shared_docs_server()).expect("reviewed again");

    assert_ne!(second.plan_id, first.plan_id);
    assert_eq!(stored_id(&state).as_deref(), Some(second.plan_id.as_str()));
}

#[test]
fn reviewing_an_opencode_server_runs_nothing_and_notes_the_check_at_import() {
    let dir = TempDir::new("sync-opencode-review");
    let state = state(&dir);
    // A binary that cannot start: a check while reviewing would fail, and fail closed (`conflict`).
    cache_detection(&state, HarnessId::OpenCode, Some(&dir.path().join("missing-opencode")));
    let shared = json!({
        "format": "harness-useful-desktop/setup",
        "version": 1,
        "harnesses": {
            "opencode": { "mcpServers": [{ "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" }] }
        }
    });

    let plan = review(&state, shared.to_string()).expect("reviewed");

    let item = &plan.sections[0].items[0];
    assert_eq!(item.id, "opencode:mcp:docs");
    assert_eq!(item.status, SyncItemStatus::New);
    assert!(item.selectable);
    assert_eq!(item.notes, [SyncNote::OpenCodeMergedCheck]);
}

#[test]
fn rejected_inputs_are_not_echoed_and_keep_no_plan() {
    let dir = TempDir::new("sync-hostile");
    let state = state(&dir);
    let marker = "ghp_MARKER1234567890";

    for hostile in [
        format!("hud1:$(id)`{marker}`"),
        format!("{{\"format\":\"{marker}\",\"version\":1}}"),
        format!("rm -rf ~ {marker}"),
    ] {
        let error = review(&state, hostile).expect_err("rejected");

        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
        assert!(!message(&error).contains(marker), "{}", message(&error));
    }

    let newer = json!({ "format": "harness-useful-desktop/setup", "version": 2 }).to_string();
    let error = review(&state, newer).expect_err("made by a newer app");
    assert_eq!(error.code(), IpcErrorCode::Unsupported);

    assert_eq!(stored_id(&state), None);
}

#[test]
fn apply_is_refused_unconfirmed_for_another_review_or_while_one_runs_and_keeps_the_plan() {
    let dir = TempDir::new("sync-refused");
    let state = state(&dir);
    let runner = Arc::new(CountingRunner::default());
    let plan = review(&state, shared_docs_server()).expect("reviewed");

    let unconfirmed = apply_with(&state, &runner, &plan.plan_id, false).expect_err("unconfirmed");
    let stale = apply_with(&state, &runner, "another-review", true).expect_err("another review");

    let running = state.sync_apply.try_lock().expect("no import yet");
    let concurrent = apply_with(&state, &runner, &plan.plan_id, true).expect_err("already running");
    drop(running);

    for error in [unconfirmed, stale, concurrent] {
        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
    }
    assert_eq!(runner.runs(), 0);
    assert!(!omp_mcp_file(&state).exists());
    assert_eq!(stored_id(&state).as_deref(), Some(plan.plan_id.as_str()));
}

#[test]
fn applying_writes_the_mcp_server_and_uses_up_the_plan() {
    let dir = TempDir::new("sync-apply");
    let state = state(&dir);
    let runner = Arc::new(CountingRunner::default());
    let plan = review(&state, shared_docs_server()).expect("reviewed");
    let (channel, events) = collecting_channel();

    let result = block_on(apply(
        &state,
        runner.clone(),
        &plan.plan_id,
        vec![DOCS.to_owned()],
        true,
        channel,
    ))
    .expect("applied");

    assert!(result.ok, "{result:?}");
    assert_eq!(result.items[0].outcome, SyncOutcome::Done);
    assert_eq!(events.lock().expect("events").len(), 2, "started and finished");
    assert_eq!(runner.runs(), 0);

    let written = std::fs::read_to_string(omp_mcp_file(&state)).expect("mcp.json written");
    let document: Value = serde_json::from_str(&written).expect("valid JSON");
    assert_eq!(document["mcpServers"]["docs"]["url"], "https://docs.example.com/mcp");

    assert_eq!(stored_id(&state), None);
    let again = apply_with(&state, &runner, &plan.plan_id, true).expect_err("the plan is used up");
    assert_eq!(again.code(), IpcErrorCode::InvalidInput);
}

#[test]
fn a_harness_gone_since_the_review_still_gets_its_mcp_server() {
    let dir = TempDir::new("sync-gone");
    let state = state(&dir);
    let runner = Arc::new(CountingRunner::default());
    let plan = review(&state, shared_docs_server()).expect("reviewed");

    // No binary is found any more: the merge needs none.
    cache_detection(&state, HarnessId::Omp, None);
    let result = apply_with(&state, &runner, &plan.plan_id, true).expect("applied");

    assert_eq!(result.items[0].outcome, SyncOutcome::Done);
    assert_eq!(runner.runs(), 0);
    assert!(omp_mcp_file(&state).exists());
}

#[test]
fn an_exported_string_imports_again_as_already_present() {
    let dir = TempDir::new("sync-export");
    let state = state(&dir);
    let file = omp_mcp_file(&state);
    std::fs::create_dir_all(file.parent().expect("agent dir")).expect("create agent dir");
    let servers = json!({ "mcpServers": { "docs": { "type": "http", "url": "https://docs.example.com/mcp" } } });
    std::fs::write(&file, servers.to_string()).expect("write mcp.json");

    let shareable = block_on(shareable(&state)).expect("export preview");
    let ids: Vec<&str> = shareable.sections[0]
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(ids, [DOCS]);

    let exported = block_on(export(&state, vec![DOCS.to_owned()], "0.15.0".to_owned())).expect("exported");
    assert_eq!(exported.item_count, 1);

    let plan = review(&state, exported.code).expect("the string imports");
    assert_eq!(plan.app.as_deref(), Some("0.15.0"));
    assert_eq!(plan.sections[0].items[0].status, SyncItemStatus::AlreadyPresent);
}

#[test]
fn a_saved_file_replaces_the_target_whole_and_is_private() {
    let dir = TempDir::new("sync-save");
    let target = dir.path().join("harness-setup.json");
    std::fs::write(&target, "an older, longer setup file").expect("write old file");

    write_atomically(&target, b"{}").expect("saved");

    assert_eq!(std::fs::read_to_string(&target).expect("read"), "{}");
    let entries = std::fs::read_dir(dir.path()).expect("list").count();
    assert_eq!(entries, 1, "no temporary file is left behind");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(&target).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
