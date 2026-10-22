//! Claude Code saved sessions: listing, titles, project isolation and deleting, over the fixtures
//! in `tests/fixtures/sessions/claude` (see `claude_session_support`). Transcripts and trees are in
//! `sessions_claude_transcript.rs`.

mod claude_session_support;
mod common;
mod session_support;

use std::fs;
use std::path::Path;

use claude_session_support::{
    COMPACTED, EDITS, NEWEST_FIRST, QUEUE, RENAMED, TOOLS, append_line, claude, claude_home, json_text, replace_in,
};
use common::CANARY;
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{HarnessId, IpcErrorCode, SessionRef, SessionSummary, SessionTitleSource};
use harness_business::pi_family::sessions::SessionCache;
use pretty_assertions::assert_eq;
use serde_json::json;
use session_support::set_mtime;

fn list(home: &claude_session_support::ClaudeHome) -> Vec<SessionSummary> {
    Claude
        .list_sessions(&home.home.env, &home.home.project, &SessionCache::default())
        .unwrap()
}

fn find<'a>(list: &'a [SessionSummary], id: &str) -> &'a SessionSummary {
    list.iter().find(|summary| summary.session.id == id).expect("listed")
}

fn error_code<T: std::fmt::Debug>(result: harness_business::Result<T>) -> IpcErrorCode {
    result.expect_err("expected an error").code()
}

#[test]
fn the_project_sessions_are_listed_newest_first_with_their_titles() {
    let home = claude_home();
    let list = list(&home);

    let ids: Vec<&str> = list.iter().map(|summary| summary.session.id.as_str()).collect();
    let expected: Vec<&str> = NEWEST_FIRST.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, expected, "the notes file and the session folder are not sessions");

    let edits = find(&list, EDITS);
    assert_eq!(edits.session.harness, HarnessId::Claude);
    assert_eq!(edits.title.as_deref(), Some("Tidy the parser"));
    assert_eq!(edits.title_source, Some(SessionTitleSource::Auto));
    assert_eq!(edits.first_prompt.as_deref(), Some("Tidy the parser and the notebook"));
    assert_eq!(edits.cwd, home.home.project.to_string_lossy());
    assert_eq!(edits.created_at, "2026-10-20T09:01:00.000Z");
    assert_eq!(edits.updated_at, "2026-10-22T09:00:00Z");
    assert_eq!(
        edits.model.as_deref(),
        Some("claude-sonnet-5"),
        "the last reply's model"
    );
    assert!(edits.size_bytes > 0);
    assert_eq!(edits.parent_session, None);
    assert!(!edits.empty_copy);

    let renamed = find(&list, RENAMED);
    assert_eq!(renamed.title.as_deref(), Some("Recorded rename"));
    assert_eq!(renamed.title_source, Some(SessionTitleSource::User));

    let compacted = find(&list, COMPACTED);
    assert_eq!(compacted.title, None);
    assert_eq!(compacted.title_source, None);
    assert_eq!(
        compacted.first_prompt.as_deref(),
        Some("Reply with exactly the word: pong")
    );

    let tools = find(&list, TOOLS);
    let prompt = tools.first_prompt.as_deref().unwrap();
    assert_eq!(prompt.chars().count(), 160, "the first prompt is cut");
    assert!(prompt.ends_with('…'));

    assert!(
        !format!("{list:?}").contains(CANARY),
        "attachment, request, subagent or hidden lines leaked into the list"
    );
}

#[test]
fn a_custom_title_line_wins_over_the_sidecar_which_wins_over_the_ai_title() {
    let home = claude_home();
    let sidecar = home.dir.join(RENAMED).join("custom-title.json");
    let file = home.file(RENAMED);

    fs::write(&sidecar, r#"{"customTitle":"  From the sidecar  "}"#).unwrap();
    set_mtime(&sidecar, "2026-10-22T08:30:00Z");
    assert_eq!(find(&list(&home), RENAMED).title.as_deref(), Some("Recorded rename"));

    // Without the rename lines the sidecar names the session.
    let kept: String = fs::read_to_string(&file)
        .unwrap()
        .lines()
        .filter(|line| !line.contains("\"custom-title\""))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(&file, kept).unwrap();
    append_line(
        &file,
        &json!({"type": "ai-title", "aiTitle": "Generated", "sessionId": RENAMED}),
    );

    let renamed = find(&list(&home), RENAMED).clone();
    assert_eq!(renamed.title.as_deref(), Some("From the sidecar"));
    assert_eq!(renamed.title_source, Some(SessionTitleSource::User));

    // Without a sidecar the last generated title is left.
    fs::remove_file(&sidecar).unwrap();
    append_line(
        &file,
        &json!({"type": "ai-title", "aiTitle": "Generated again", "sessionId": RENAMED}),
    );

    let renamed = find(&list(&home), RENAMED).clone();
    assert_eq!(renamed.title.as_deref(), Some("Generated again"));
    assert_eq!(renamed.title_source, Some(SessionTitleSource::Auto));

    // The last rename line wins over all of them.
    append_line(
        &file,
        &json!({"type": "custom-title", "customTitle": "Renamed again", "sessionId": RENAMED}),
    );
    assert_eq!(find(&list(&home), RENAMED).title.as_deref(), Some("Renamed again"));
}

#[test]
fn a_session_of_another_folder_with_the_same_slug_is_left_out() {
    let home = claude_home();
    // `<home>/Projects-demo` and `<home>/Projects/demo` share the slug.
    let twin = home.home.home.join("Projects-demo");
    replace_in(&home.file(QUEUE), &json_text(&home.home.project), &json_text(&twin));

    let list = list(&home);
    assert!(list.iter().all(|summary| summary.session.id != QUEUE));
    assert_eq!(list.len(), NEWEST_FIRST.len() - 1);
}

#[test]
fn sessions_of_other_projects_stay_apart() {
    let home = claude_home();
    let other = home.home.home.join("Projects/other");
    fs::create_dir_all(&other).unwrap();

    let other_dir = Claude.sessions_dir(&home.home.env, &other);
    fs::create_dir_all(&other_dir).unwrap();
    fs::rename(home.file(EDITS), other_dir.join(format!("{EDITS}.jsonl"))).unwrap();

    let demo = list(&home);
    assert!(demo.iter().all(|summary| summary.session.id != EDITS));

    let env = &home.home.env;
    let project = &home.home.project;
    let session = claude(EDITS);
    assert_eq!(
        error_code(Claude.resolve_session(env, project, &session)),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        error_code(Claude.session_tree(env, project, &session)),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        error_code(Claude.delete_session(env, project, &session, false)),
        IpcErrorCode::NotFound
    );
    assert!(other_dir.join(format!("{EDITS}.jsonl")).is_file());

    let theirs = Claude.list_sessions(env, &other, &SessionCache::default()).unwrap();
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0].session.id, EDITS);
}

#[test]
fn invalid_or_foreign_ids_read_and_delete_nothing() {
    let home = claude_home();
    let env = &home.home.env;
    let project = &home.home.project;
    let before = fs::read_dir(&home.dir).unwrap().count();

    let invalid = [
        "../5e1f0c2a-7b3d-4c8e-9f10-2a3b4c5d6e7f",
        "5E1F0C2A-7B3D-4C8E-9F10-2A3B4C5D6E7F",
        "5e1f0c2a7b3d4c8e9f102a3b4c5d6e7f",
        "notes",
        "",
    ];

    for id in invalid {
        let session = claude(id);
        assert_eq!(
            error_code(Claude.resolve_session(env, project, &session)),
            IpcErrorCode::InvalidInput
        );
        assert_eq!(
            error_code(Claude.delete_session(env, project, &session, false)),
            IpcErrorCode::InvalidInput
        );
    }

    let foreign = SessionRef {
        harness: HarnessId::Pi,
        id: EDITS.to_owned(),
    };
    assert_eq!(
        error_code(Claude.delete_session(env, project, &foreign, false)),
        IpcErrorCode::InvalidInput
    );

    // A folder named like a session file is not one.
    fs::create_dir(home.dir.join("00000000-0000-4000-8000-000000000000.jsonl")).unwrap();
    let folder = claude("00000000-0000-4000-8000-000000000000");
    assert_eq!(
        error_code(Claude.delete_session(env, project, &folder, false)),
        IpcErrorCode::NotFound
    );

    assert_eq!(fs::read_dir(&home.dir).unwrap().count(), before + 1);
    assert!(home.file(EDITS).is_file());
}

/// Fills the folders Claude Code keeps for session `id`.
fn session_folders(home: &claude_session_support::ClaudeHome, id: &str) -> Vec<std::path::PathBuf> {
    let folders = vec![
        home.dir.join(id).join("subagents"),
        home.config.join("file-history").join(id),
        home.config.join("session-env").join(id),
    ];

    for folder in &folders {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join("entry.json"), "{}").unwrap();
    }

    folders
}

#[test]
fn delete_removes_the_session_with_its_folders() {
    let home = claude_home();
    let env = &home.home.env;
    let project = &home.home.project;

    session_folders(&home, EDITS);
    session_folders(&home, TOOLS);
    fs::write(home.dir.join(format!("{EDITS}.precompact.json")), "{}").unwrap();

    Claude.delete_session(env, project, &claude(EDITS), false).unwrap();

    assert!(!home.file(EDITS).exists());
    assert!(!home.dir.join(EDITS).exists());
    assert!(!home.dir.join(format!("{EDITS}.precompact.json")).exists());
    assert!(!home.config.join("file-history").join(EDITS).exists());
    assert!(!home.config.join("session-env").join(EDITS).exists());

    // Everything of the other sessions stays.
    assert!(home.file(TOOLS).is_file());
    assert!(home.dir.join(TOOLS).join("subagents/entry.json").is_file());
    assert!(
        home.config
            .join("file-history")
            .join(TOOLS)
            .join("entry.json")
            .is_file()
    );
    assert!(home.config.join("session-env").join(TOOLS).join("entry.json").is_file());
    assert_eq!(list(&home).len(), NEWEST_FIRST.len() - 1);

    assert_eq!(
        error_code(Claude.delete_session(env, project, &claude(EDITS), false)),
        IpcErrorCode::NotFound
    );
}

#[cfg(unix)]
#[test]
fn delete_leaves_linked_folders_alone() {
    let home = claude_home();
    let outside = home.home.home.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep.txt"), "keep").unwrap();

    fs::create_dir_all(home.config.join("file-history")).unwrap();
    std::os::unix::fs::symlink(&outside, home.config.join("file-history").join(EDITS)).unwrap();
    std::os::unix::fs::symlink(&outside, home.dir.join(EDITS)).unwrap();

    Claude
        .delete_session(&home.home.env, &home.home.project, &claude(EDITS), false)
        .unwrap();

    assert!(!home.file(EDITS).exists());
    assert!(outside.join("keep.txt").is_file());
}

#[test]
fn a_file_without_a_line_of_the_session_is_not_deleted() {
    let home = claude_home();
    let stranger = "11111111-2222-4333-8444-555555555555";
    fs::copy(home.file(TOOLS), home.file(stranger)).unwrap();

    let result = Claude.delete_session(&home.home.env, &home.home.project, &claude(stranger), false);

    assert_eq!(error_code(result), IpcErrorCode::InvalidInput);
    assert!(home.file(stranger).is_file());
    assert!(list(&home).iter().all(|summary| summary.session.id != stranger));
}

#[test]
fn only_empty_copies_are_refused_as_none_exist() {
    let home = claude_home();
    let result = Claude.delete_session(&home.home.env, &home.home.project, &claude(EDITS), true);

    assert_eq!(error_code(result), IpcErrorCode::InvalidInput);
    assert!(home.file(EDITS).is_file());
}

/// Writes the pid file a running `claude` with `pid` keeps for session `id`.
fn pid_file(config: &Path, pid: u32, id: &str) {
    let sessions = config.join("sessions");
    fs::create_dir_all(&sessions).unwrap();

    let record = json!({"pid": pid, "sessionId": id, "cwd": "/home/demo/Projects/demo", "kind": "interactive"});
    fs::write(sessions.join(format!("{pid}.json")), record.to_string()).unwrap();
}

#[test]
fn a_running_session_is_not_deleted() {
    let home = claude_home();
    let env = &home.home.env;
    let project = &home.home.project;

    // This test process stands in for a running `claude`.
    pid_file(&home.config, std::process::id(), EDITS);

    let result = Claude.delete_session(env, project, &claude(EDITS), false);
    assert_eq!(error_code(result), IpcErrorCode::Forbidden);
    assert!(home.file(EDITS).is_file());

    // Another session of the same process is not running.
    Claude.delete_session(env, project, &claude(TOOLS), false).unwrap();
    assert!(!home.file(TOOLS).exists());
}

#[cfg(unix)]
#[test]
fn a_pid_file_left_by_an_ended_process_does_not_block_the_delete() {
    let home = claude_home();
    let mut child = std::process::Command::new("true").spawn().expect("spawn true");
    let pid = child.id();
    child.wait().expect("wait for true");

    pid_file(&home.config, pid, EDITS);

    Claude
        .delete_session(&home.home.env, &home.home.project, &claude(EDITS), false)
        .unwrap();
    assert!(!home.file(EDITS).exists());
}
