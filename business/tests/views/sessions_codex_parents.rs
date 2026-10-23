//! The session tree of Codex (the fake of `sessions_codex`): Codex 0.154 lists every thread with
//! `forkedFromId: null`, so a list shows the parents known at once and reads the others with
//! `thread/read` in the background, a bounded batch per run, keeping them in the app's data folder
//! so a later list (or a restart) reads nothing again. A fork listed only after its first turn
//! gets its parent then, a corrupt file is replaced, and a deleted thread leaves the file.
#![cfg(unix)]

use crate::agent_support;
use crate::sessions_codex;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent_support::scenario;
use harness_business::app_dirs::local_data_dir;
use harness_business::harness::CliSessions;
use harness_business::harnesses::codex::sessions::{CodexSessions, PARENTS_FILE};
use harness_business::model::{IpcErrorCode, SessionSummary};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sessions_codex::{CLONE, FORK, FakeCodex, OLDER, PARENT, code, thread};

/// The threads of `codex_sessions_list.jsonl` with the parents `thread/read` names.
const ALL: [(&str, Option<&str>); 4] = [
    (CLONE, Some(PARENT)),
    (FORK, Some(PARENT)),
    (PARENT, None),
    (OLDER, None),
];

/// Scenarios written for one test; they include the fixtures by absolute path.
struct Scenarios(tempfile::TempDir);

impl Scenarios {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("scenario folder"))
    }

    /// A run answering `thread/list` with `ids`, newest first, each `forkedFromId: null` as
    /// Codex 0.154 lists them.
    fn list(&self, name: &str, ids: &[&str]) -> PathBuf {
        let threads: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| codex_thread(id, None, 1_792_689_700 - index as u64))
            .collect();
        let step = json!({"expect": "thread/list", "reply": {"data": threads, "nextCursor": null}});

        self.write(name, &[step])
    }

    /// A run answering `thread/read` of each of `threads` (id, parent) in turn, then `then`.
    fn reads(&self, name: &str, threads: &[(&str, Option<&str>)], then: &[Value]) -> PathBuf {
        let mut steps: Vec<_> = threads
            .iter()
            .map(|(id, parent)| {
                json!({
                    "expect": "thread/read",
                    "match": {"/params/threadId": id},
                    "reply": {"thread": codex_thread(id, *parent, 1_792_689_600)},
                })
            })
            .collect();
        steps.extend_from_slice(then);

        self.write(name, &steps)
    }

    fn write(&self, name: &str, steps: &[Value]) -> PathBuf {
        let handshake = json!({"include": scenario("codex_sessions_handshake.jsonl")});
        let lines: Vec<String> = std::iter::once(&handshake).chain(steps).map(Value::to_string).collect();
        let path = self.0.path().join(format!("{name}.jsonl"));

        std::fs::write(&path, lines.join("\n") + "\n").expect("write scenario");
        path
    }
}

/// A saved thread of the recorded project, as `thread/list` and `thread/read` answer it.
fn codex_thread(id: &str, parent: Option<&str>, updated: u64) -> Value {
    json!({
        "id": id, "sessionId": id, "forkedFromId": parent, "parentThreadId": null,
        "preview": format!("Prompt of {id}"), "ephemeral": false, "modelProvider": "openai",
        "model": "gpt-5.6-terra", "createdAt": updated, "updatedAt": updated,
        "status": {"type": "notLoaded"}, "cwd": "/work/project", "cliVersion": "0.154.0",
        "source": "vscode", "name": null, "turns": [],
    })
}

fn delete(id: &str, error: Option<&str>) -> Value {
    match error {
        Some(error) => json!({"expect": "thread/delete", "match": {"/params/threadId": id}, "error": error}),
        None => json!({"expect": "thread/delete", "match": {"/params/threadId": id}, "reply": {}}),
    }
}

fn parents(listed: &[SessionSummary]) -> Vec<(&str, Option<&str>)> {
    listed
        .iter()
        .map(|summary| (summary.session.id.as_str(), summary.parent_session.as_deref()))
        .collect()
}

fn has_parent(listed: &[SessionSummary], id: &str) -> bool {
    listed
        .iter()
        .any(|summary| summary.session.id == id && summary.parent_session.is_some())
}

/// Waits (as the Sessions view does, listing again) until `done` holds for a list.
fn listed_until(
    sessions: &CodexSessions,
    codex: &FakeCodex,
    done: impl Fn(&[SessionSummary]) -> bool,
) -> Vec<SessionSummary> {
    let deadline = Instant::now() + Duration::from_secs(20);

    loop {
        let listed = sessions.list(&codex.ctx()).unwrap();

        if done(&listed) {
            return listed;
        }

        assert!(
            Instant::now() < deadline,
            "the parents never showed: {:?}",
            parents(&listed)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);

    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn parents_file(codex: &FakeCodex) -> PathBuf {
    local_data_dir(&codex.home.env).unwrap().join(PARENTS_FILE)
}

fn saved(codex: &FakeCodex) -> Value {
    std::fs::read_to_string(parents_file(codex))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

/// The thread ids of the saved file, in its order.
fn saved_ids(codex: &FakeCodex) -> Vec<String> {
    saved(codex)["threads"]
        .as_object()
        .map(|threads| threads.keys().cloned().collect())
        .unwrap_or_default()
}

/// Lists, then waits until the background reads named every parent of `codex_sessions_list.jsonl`.
fn resolved(sessions: &CodexSessions, codex: &FakeCodex) {
    let first = sessions.list(&codex.ctx()).unwrap();
    assert_eq!(
        parents(&first),
        [(CLONE, None), (FORK, None), (PARENT, None), (OLDER, None)],
        "Codex lists no parent"
    );

    let later = listed_until(sessions, codex, |listed| {
        has_parent(listed, FORK) && has_parent(listed, CLONE)
    });
    assert_eq!(parents(&later), ALL);
}

#[test]
fn a_first_list_reads_the_parents_in_the_background_and_a_restart_reads_none_again() {
    let reads = Scenarios::new();
    let list = scenario("codex_sessions_list.jsonl");
    let codex = FakeCodex::playing(&[list.clone(), reads.reads("parents", &ALL, &[]), list]);

    resolved(&CodexSessions::new(), &codex);
    assert_eq!(codex.runs(), 2, "one list and one batch of reads");

    let file = saved(&codex);
    assert_eq!(
        file,
        json!({"version": 1, "threads": {CLONE: PARENT, FORK: PARENT, PARENT: null, OLDER: null}}),
        "thread ids only"
    );
    let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(parents_file(&codex)).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o600);

    // The app again: the parents come from the file, and nothing is read.
    let restarted = CodexSessions::new();
    assert_eq!(parents(&restarted.list(&codex.ctx()).unwrap()), ALL);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(parents(&restarted.list(&codex.ctx()).unwrap()), ALL);
    assert_eq!(codex.runs(), 3, "the list alone");
}

#[test]
fn a_corrupt_file_is_read_as_empty_and_replaced() {
    let reads = Scenarios::new();
    let codex = FakeCodex::playing(&[scenario("codex_sessions_list.jsonl"), reads.reads("parents", &ALL, &[])]);
    let file = parents_file(&codex);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{\"version\": 1, \"threads\": [").unwrap();

    resolved(&CodexSessions::new(), &codex);

    assert_eq!(saved_ids(&codex), [CLONE, FORK, PARENT, OLDER]);
}

#[test]
fn a_fork_listed_after_its_first_turn_gets_its_parent() {
    let reads = Scenarios::new();
    let codex = FakeCodex::playing(&[
        reads.list("before", &[PARENT, OLDER]),
        reads.reads("known", &[(PARENT, None), (OLDER, None)], &[]),
        scenario("codex_sessions_list.jsonl"),
        reads.reads("forks", &[(CLONE, Some(PARENT)), (FORK, Some(PARENT))], &[]),
    ]);

    let before = CodexSessions::new();
    assert_eq!(
        parents(&before.list(&codex.ctx()).unwrap()),
        [(PARENT, None), (OLDER, None)]
    );
    wait_until("both threads are kept", || saved_ids(&codex).len() == 2);

    // The forks had their first turns: Codex lists them now (a fresh list, past the cached one).
    let after = CodexSessions::new();
    resolved(&after, &codex);

    // Only the new threads were read.
    assert_eq!(codex.runs(), 4);
}

#[test]
fn each_background_run_reads_a_bounded_batch() {
    let reads = Scenarios::new();
    let codex = FakeCodex::playing(&[
        reads.list("three", &[CLONE, PARENT, FORK]),
        reads.reads("first", &[(CLONE, Some(PARENT)), (PARENT, None)], &[]),
        reads.reads("second", &[(FORK, Some(PARENT))], &[]),
    ]);
    let sessions = CodexSessions::new().with_parent_batch(2);

    sessions.list(&codex.ctx()).unwrap();
    let first = listed_until(&sessions, &codex, |listed| has_parent(listed, CLONE));
    assert_eq!(parents(&first), [(CLONE, Some(PARENT)), (PARENT, None), (FORK, None)]);

    let second = listed_until(&sessions, &codex, |listed| has_parent(listed, FORK));
    assert_eq!(
        parents(&second),
        [(CLONE, Some(PARENT)), (PARENT, None), (FORK, Some(PARENT))]
    );
    assert_eq!(codex.runs(), 3, "one list and two batches");
}

#[test]
fn an_opened_thread_needs_no_second_read_for_its_parent() {
    let reads = Scenarios::new();
    let codex = FakeCodex::playing(&[
        scenario("codex_sessions_read.jsonl"),
        scenario("codex_sessions_list.jsonl"),
        reads.reads(
            "rest",
            &[(CLONE, Some(PARENT)), (FORK, Some(PARENT)), (OLDER, None)],
            &[],
        ),
    ]);
    let sessions = CodexSessions::new();

    sessions.messages(&codex.ctx(), &thread(PARENT), None, 50).unwrap();
    resolved(&sessions, &codex);

    wait_until("every thread is kept", || saved_ids(&codex).len() == 4);
    assert_eq!(saved_ids(&codex), [PARENT, CLONE, FORK, OLDER]);
}

#[test]
fn a_deleted_thread_leaves_the_file_and_a_parent_with_forks_stays() {
    let reads = Scenarios::new();
    let refused = "cannot delete thread: forked history still references it";
    let codex = FakeCodex::playing(&[
        scenario("codex_sessions_list.jsonl"),
        reads.reads("parents", &ALL, &[]),
        reads.reads("refused", &[(PARENT, None)], &[delete(PARENT, Some(refused))]),
        reads.reads("deleted", &[(FORK, Some(PARENT))], &[delete(FORK, None)]),
    ]);
    let sessions = CodexSessions::new();
    resolved(&sessions, &codex);

    assert_eq!(
        code(sessions.delete(&codex.ctx(), &thread(PARENT))),
        IpcErrorCode::Conflict
    );
    assert_eq!(saved_ids(&codex), [CLONE, FORK, PARENT, OLDER]);

    sessions.delete(&codex.ctx(), &thread(FORK)).unwrap();
    assert_eq!(saved_ids(&codex), [CLONE, PARENT, OLDER]);
}
