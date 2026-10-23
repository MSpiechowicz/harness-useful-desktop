//! Codex sessions through a short-lived `codex app-server`, played by `fake_agent --dialect codex`
//! behind a wrapper script that records each run (scenarios in
//! `tests/fixtures/streams/codex_sessions_*.jsonl`, built from the S0 recordings): the list with
//! forks under their parent, a transcript with its touched files, fork points and the export,
//! deleting (refused while forks reference the thread), timeouts, Codex errors and lines that are
//! no frames. The real `codex` binary never runs.
#![cfg(unix)]

use crate::agent_support;
use crate::fake_cli_support;
use crate::session_support;

use std::path::Path;
use std::time::{Duration, Instant};

use agent_support::scenario;
use fake_cli_support::FakeCli;
use harness_business::harness::{CliSessionCtx, CliSessions, Harness};
use harness_business::harnesses::Codex;
use harness_business::harnesses::codex::sessions::CodexSessions;
use harness_business::model::{ChatBlock, HarnessId, IpcErrorCode, MessageRole, SessionRef};
use harness_business::process::{ExecSpec, LineChild, ReadLine, StdRunner};
use pretty_assertions::assert_eq;
use session_support::{SessionHome, session, session_home};

pub(crate) const PARENT: &str = "01a164a0-4c41-7613-9735-20de2a8016a8";
const FORK: &str = "01a164a1-51b0-7192-935e-520ff4afa71d";
const CLONE: &str = "01a164a1-5205-7fa2-a5bc-2a7b7e0bc633";
const OLDER: &str = "01a1649c-d6df-73c3-a197-2c212fd07a54";
const ELSEWHERE: &str = "01a164a2-6c00-7000-8000-0000000e15e0";
const TURN1: &str = "01a164a0-4c86-7902-8de6-44d0f7bc8ac7";
const TURN2: &str = "01a164a0-801f-7a51-80f7-8cd218126f7d";
const TURN3: &str = "01a164a0-96a7-7243-8b57-70d95d87c471";
const PROMPT: &str = "Create a file hello.txt containing the single word hi, then run the shell command `ls -1` and tell me the output. Be brief.";

/// A Codex binary that plays one scenario on every run, recording its argv, `CODEX_HOME`, pid and
/// the number of runs beside it.
pub(crate) struct FakeCodex {
    pub(crate) cli: FakeCli,
    pub(crate) home: SessionHome,
}

impl FakeCodex {
    pub(crate) fn new(name: &str) -> Self {
        let fake = env!("CARGO_BIN_EXE_fake_agent");
        let scenario = scenario(name);
        let body = format!(
            "echo run >> \"$d/runs\"\nprintf '%s\\n' \"$@\" > \"$d/args\"\nprintf '%s' \"$CODEX_HOME\" > \"$d/codex_home\"\n\
             echo $$ > \"$d/pid\"\nexec '{fake}' --dialect codex --scenario '{}'\n",
            scenario.display()
        );

        Self {
            cli: FakeCli::script("codex", &body),
            home: session_home(),
        }
    }

    pub(crate) fn ctx(&self) -> CliSessionCtx<'_> {
        CliSessionCtx {
            env: &self.home.env,
            project: &self.home.project,
            binary: &self.cli.binary,
            runner: &StdRunner,
        }
    }

    pub(crate) fn runs(&self) -> usize {
        self.cli.recorded("runs").map_or(0, |runs| runs.lines().count())
    }

    fn pid(&self) -> i32 {
        self.cli.recorded("pid").unwrap().trim().parse().unwrap()
    }
}

pub(crate) fn thread(id: &str) -> SessionRef {
    session(HarnessId::Codex, id)
}

pub(crate) fn code<T: std::fmt::Debug>(result: harness_business::Result<T>) -> IpcErrorCode {
    result.expect_err("expected an error").code()
}

fn text(blocks: &[ChatBlock]) -> Vec<String> {
    blocks
        .iter()
        .map(|block| match block {
            ChatBlock::Text { text } => text.clone(),
            ChatBlock::Thinking { text, .. } => format!("thinking: {text}"),
            ChatBlock::ToolCall { name, .. } => format!("tool: {name}"),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn codex_reads_its_sessions_through_the_app_server_without_a_fork_count() {
    assert!(Codex.cli_sessions().is_some());
    assert!(!Codex.capabilities().session_tree);
}

#[test]
fn the_project_threads_are_listed_newest_first_with_forks_under_their_parent() {
    let codex = FakeCodex::new("codex_sessions_list.jsonl");
    let sessions = CodexSessions::new();

    let listed = sessions.list(&codex.ctx()).unwrap();

    let rows: Vec<_> = listed
        .iter()
        .map(|summary| {
            (
                summary.session.id.as_str(),
                summary.title.as_deref(),
                summary.parent_session.as_deref(),
                summary.updated_at.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (CLONE, Some("S0 recording"), Some(PARENT), "2026-10-22T17:20:57Z"),
            (FORK, Some("S0 recording"), Some(PARENT), "2026-10-22T17:20:56Z"),
            (PARENT, Some("S0 recording"), None, "2026-10-22T17:20:54Z"),
            (OLDER, None, None, "2026-10-22T17:16:07Z"),
        ]
    );

    let older = &listed[3];
    assert_eq!(older.first_prompt.as_deref(), Some(PROMPT));
    assert_eq!(older.created_at, "2026-10-22T17:16:02Z");
    assert_eq!(older.model.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(Path::new(&older.cwd), codex.home.project);
    assert_eq!(listed[0].first_prompt, None);

    assert_eq!(
        codex.cli.recorded("args").unwrap(),
        "app-server\n-c\ncheck_for_update_on_startup=false\n-c\nfeatures.in_app_updates=false\n"
    );
    assert_eq!(
        codex.cli.recorded("codex_home").unwrap(),
        codex.home.home.join(".codex").to_string_lossy()
    );

    // A second look within a few seconds reuses the list.
    assert_eq!(sessions.list(&codex.ctx()).unwrap(), listed);
    assert_eq!(codex.runs(), 1);
}

#[test]
fn a_thread_opens_as_a_paged_transcript_with_its_touched_files() {
    let codex = FakeCodex::new("codex_sessions_read.jsonl");
    let sessions = CodexSessions::new();
    let project = codex.home.project.to_string_lossy().into_owned();

    let first = sessions.messages(&codex.ctx(), &thread(PARENT), None, 4).unwrap();
    let rest = sessions
        .messages(&codex.ctx(), &thread(PARENT), first.next_cursor.as_deref(), 200)
        .unwrap();
    let touched = sessions.files_touched(&codex.ctx(), &thread(PARENT)).unwrap();

    // One app-server read serves every page and the touched files.
    assert_eq!(codex.runs(), 1);
    assert_eq!(first.total, 12);
    assert!(first.next_cursor.as_deref().unwrap().starts_with("4."));
    assert_eq!(rest.next_cursor, None);
    assert_eq!(rest.stats, None);
    assert_eq!(touched, ["hello.txt", "src/new.rs", "src/old.rs"]);

    let messages: Vec<_> = first.messages.iter().chain(&rest.messages).collect();
    let shape: Vec<_> = messages
        .iter()
        .map(|message| (message.role, message.id.as_deref(), text(&message.blocks)))
        .collect();
    assert_eq!(
        shape,
        [
            (MessageRole::User, Some(TURN1), vec![PROMPT.to_owned()]),
            (
                MessageRole::Assistant,
                Some(TURN1),
                vec![
                    "I’ll create the requested file, then list this directory.".to_owned(),
                    "tool: shell".to_owned(),
                    "tool: apply_patch".to_owned()
                ]
            ),
            (MessageRole::ToolResult, Some(TURN1), vec![format!("{project}\n")]),
            (
                MessageRole::ToolResult,
                Some(TURN1),
                vec![format!("add {project}/hello.txt\nhi\n")]
            ),
            (
                MessageRole::Assistant,
                Some(TURN1),
                vec!["Created `hello.txt`.\n\n`ls -1` output:\n```text\nREADME.md\nhello.txt\n```".to_owned()]
            ),
            (
                MessageRole::User,
                Some(TURN2),
                vec!["Run the shell command `cat hello.txt`. If it is not allowed, just say so.".to_owned()]
            ),
            (
                MessageRole::Assistant,
                Some(TURN2),
                vec![
                    "thinking: Reading the file as asked.".to_owned(),
                    "tool: apply_patch".to_owned(),
                    "tool: apply_patch".to_owned(),
                    "tool: apply_patch".to_owned()
                ]
            ),
            (
                MessageRole::ToolResult,
                Some(TURN2),
                vec!["The user declined this change.".to_owned()]
            ),
            (
                MessageRole::ToolResult,
                Some(TURN2),
                vec!["update /elsewhere/project/notes.txt\n@@\n".to_owned()]
            ),
            (
                MessageRole::ToolResult,
                Some(TURN2),
                vec![format!(
                    "update {project}/src/old.rs (moved to {project}/src/new.rs)\n@@\n"
                )]
            ),
            (MessageRole::CompactionSummary, Some(TURN3), vec![]),
            (
                MessageRole::User,
                Some(TURN3),
                vec!["Run `sleep 6 && echo done` and report the result.".to_owned()]
            ),
        ]
    );

    let probe = messages[2];
    assert_eq!(probe.tool_name.as_deref(), Some("shell"));
    assert_eq!(
        probe.tool_call_id.as_deref(),
        Some("exec-f61152e4-887b-4723-add5-193b11a1ad79")
    );
    assert_eq!(probe.is_error, Some(true));

    let failed = messages[6];
    assert_eq!(failed.stop_reason.as_deref(), Some("error"));
    assert_eq!(
        failed.error_message.as_deref(),
        Some("The model gpt-6-astra is not supported when using Codex with a ChatGPT account.")
    );
    assert_eq!(messages[1].stop_reason.as_deref(), Some("commentary"));
    assert_eq!(messages[4].stop_reason.as_deref(), Some("final_answer"));
    assert_eq!(messages[4].model.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(messages[0].timestamp, Some(1_792_689_589_000));

    let stats = first.stats.unwrap();
    assert_eq!((stats.messages, stats.tool_calls), (6, 5));
    assert_eq!(
        (stats.started_at, stats.ended_at),
        (Some(1_792_689_589_000), Some(1_792_689_608_000))
    );
    assert_eq!(stats.models.len(), 1);
    assert_eq!(stats.models[0].turns, 3);
}

#[test]
fn a_fork_keeps_the_turn_of_its_message_and_the_export_holds_the_transcript() {
    let codex = FakeCodex::new("codex_sessions_read.jsonl");
    let sessions = CodexSessions::new();

    assert_eq!(
        sessions.fork_point(&codex.ctx(), &thread(PARENT), TURN2).unwrap(),
        TURN2
    );
    assert_eq!(
        code(sessions.fork_point(&codex.ctx(), &thread(PARENT), "not-a-turn")),
        IpcErrorCode::NotFound
    );

    let html = sessions
        .export_html(&codex.ctx(), &thread(PARENT), "2026-10-22T18:00:00Z")
        .unwrap();
    assert!(html.contains("S0 recording"));
    assert!(html.contains("Create a file hello.txt"));

    let saved = sessions.saved_messages(&codex.ctx(), &thread(PARENT)).unwrap();
    assert_eq!(saved.len(), 12);
}

#[test]
fn a_thread_of_the_project_is_deleted() {
    let codex = FakeCodex::new("codex_sessions_delete.jsonl");

    CodexSessions::new().delete(&codex.ctx(), &thread(OLDER)).unwrap();

    assert_eq!(codex.runs(), 1);
}

#[test]
fn a_thread_its_forks_reference_is_kept_with_a_conflict() {
    let codex = FakeCodex::new("codex_sessions_delete_forked.jsonl");

    let refused = CodexSessions::new().delete(&codex.ctx(), &thread(PARENT));

    let error = refused.expect_err("Codex refuses");
    assert_eq!(error.code(), IpcErrorCode::Conflict);
    assert!(error.to_string().contains("delete the forks of this session first"));
}

#[test]
fn a_thread_of_another_folder_is_neither_read_nor_deleted() {
    let codex = FakeCodex::new("codex_sessions_delete_elsewhere.jsonl");
    let sessions = CodexSessions::new();

    assert_eq!(
        code(sessions.delete(&codex.ctx(), &thread(ELSEWHERE))),
        IpcErrorCode::NotFound
    );
}

#[test]
fn a_thread_codex_does_not_have_is_not_found() {
    let codex = FakeCodex::new("codex_sessions_unknown.jsonl");

    let read = CodexSessions::new().messages(&codex.ctx(), &thread(PARENT), None, 50);

    assert_eq!(code(read), IpcErrorCode::NotFound);
}

#[test]
fn ids_of_other_harnesses_or_unsafe_ids_never_start_codex() {
    let codex = FakeCodex::new("codex_sessions_read.jsonl");
    let sessions = CodexSessions::new();

    let foreign = session(HarnessId::OpenCode, PARENT);
    assert_eq!(
        code(sessions.messages(&codex.ctx(), &foreign, None, 50)),
        IpcErrorCode::InvalidInput
    );
    assert_eq!(
        code(sessions.delete(&codex.ctx(), &thread("../x"))),
        IpcErrorCode::InvalidInput
    );
    assert_eq!(codex.runs(), 0);
}

#[test]
fn a_codex_that_does_not_answer_is_killed_at_the_deadline() {
    let codex = FakeCodex::new("codex_sessions_hang.jsonl");
    let sessions = CodexSessions::with_timeout(Duration::from_secs(3));
    let started = Instant::now();

    let listed = sessions.list(&codex.ctx());

    assert_eq!(code(listed), IpcErrorCode::Timeout);
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(!process_exists(codex.pid()), "the app-server survived");
}

#[test]
fn lines_that_are_no_frames_are_skipped_and_requests_of_codex_refused() {
    let codex = FakeCodex::new("codex_sessions_malformed.jsonl");

    let listed = CodexSessions::new().list(&codex.ctx()).unwrap();

    let ids: Vec<_> = listed.iter().map(|summary| summary.session.id.as_str()).collect();
    assert_eq!(ids, [OLDER]);
}

#[test]
fn a_codex_error_names_what_failed() {
    let codex = FakeCodex::new("codex_sessions_list_error.jsonl");

    let error = CodexSessions::new().list(&codex.ctx()).expect_err("Codex refuses");

    assert_eq!(error.code(), IpcErrorCode::Process);
    assert!(
        error
            .to_string()
            .contains("codex thread/list failed: the state database is unavailable")
    );
}

#[test]
fn a_codex_that_stops_says_why() {
    let codex = FakeCodex::new("codex_sessions_exit.jsonl");

    let error = CodexSessions::new().list(&codex.ctx()).expect_err("Codex stops");

    assert_eq!(error.code(), IpcErrorCode::Process);
    assert!(
        error
            .to_string()
            .contains("codex app-server stopped: Error: failed to load config.toml: expected a table"),
        "{error}"
    );
}

#[test]
fn a_missing_binary_is_not_found() {
    let codex = FakeCodex::new("codex_sessions_list.jsonl");
    let missing = harness_business::process::ResolvedBinary {
        path: codex.home.home.join("bin/codex"),
        is_batch_script: false,
    };
    let ctx = CliSessionCtx {
        binary: &missing,
        ..codex.ctx()
    };

    assert_eq!(code(CodexSessions::new().list(&ctx)), IpcErrorCode::NotFound);
}

#[test]
fn a_line_child_drops_lines_over_its_cap_and_keeps_reading() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = dir.path().join("long.jsonl");
    let long = "x".repeat(300);
    std::fs::write(
        &scenario,
        format!("{{\"emit\": {{\"method\": \"warning\", \"params\": {{\"message\": \"{long}\"}}}}}}\n{{\"emit\": {{\"id\": 1, \"result\": {{}}}}}}\n"),
    )
    .unwrap();

    let spec = ExecSpec::new(env!("CARGO_BIN_EXE_fake_agent"))
        .args(["--dialect", "codex", "--scenario"])
        .args([scenario.as_os_str()])
        .cwd(dir.path());
    let mut child = LineChild::spawn(&spec, 100).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);

    assert_eq!(child.read_line(deadline), ReadLine::TooLong);
    assert_eq!(
        child.read_line(deadline),
        ReadLine::Line(r#"{"id":1,"result":{}}"#.to_owned())
    );
    assert!(child.finish(deadline), "the fake exits once its input closes");
}

/// True while process `pid` exists (signal 0 reaches it).
fn process_exists(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    kill(Pid::from_raw(pid), None).is_ok()
}
