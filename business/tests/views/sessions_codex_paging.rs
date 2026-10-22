//! Paging of Codex sessions through the short-lived `codex app-server` (the fake of
//! `sessions_codex`): each call has its own deadline, a run stops paging at its read budget and
//! keeps what it read, and a transcript page of a session that changed since its first page is
//! refused instead of repeating or skipping messages.
#![cfg(unix)]

use crate::agent_support;
use crate::fake_cli_support;
use crate::session_support;
use crate::sessions_codex;

use std::time::Duration;

use agent_support::scenario;
use fake_cli_support::FakeCli;
use harness_business::harness::CliSessions;
use harness_business::harnesses::codex::sessions::CodexSessions;
use harness_business::model::IpcErrorCode;
use pretty_assertions::assert_eq;
use session_support::session_home;
use sessions_codex::{FakeCodex, PARENT, code, thread};

/// A Codex binary that plays `first` on its first run and `then` on every later one.
fn changing(first: &str, then: &str) -> FakeCodex {
    let fake = env!("CARGO_BIN_EXE_fake_agent");
    let (first, then) = (scenario(first), scenario(then));
    let body = format!(
        "echo run >> \"$d/runs\"\nif [ \"$(wc -l < \"$d/runs\")\" -gt 1 ]; then s='{}'; else s='{}'; fi\n\
         exec '{fake}' --dialect codex --scenario \"$s\"\n",
        then.display(),
        first.display()
    );

    FakeCodex {
        cli: FakeCli::script("codex", &body),
        home: session_home(),
    }
}

#[test]
fn each_call_has_its_own_deadline_so_a_long_read_finishes() {
    let codex = FakeCodex::new("codex_sessions_read_slow.jsonl");
    let sessions = CodexSessions::with_timeout(Duration::from_millis(1_500));

    let page = sessions.messages(&codex.ctx(), &thread(PARENT), None, 200).unwrap();

    assert_eq!(
        page.total, 12,
        "four calls of 700 ms each, longer than one deadline together"
    );
}

#[test]
fn paging_stops_at_the_read_budget_and_keeps_what_was_read() {
    let codex = FakeCodex::new("codex_sessions_read.jsonl");
    let sessions = CodexSessions::with_limits(Duration::from_secs(30), 1);

    let page = sessions.messages(&codex.ctx(), &thread(PARENT), None, 200).unwrap();
    let whole = CodexSessions::new()
        .messages(&codex.ctx(), &thread(PARENT), None, 200)
        .unwrap();

    assert!(page.total < whole.total, "{} of {}", page.total, whole.total);
    assert!(page.total > 0);
}

#[test]
fn a_later_page_of_a_session_that_changed_meanwhile_is_refused() {
    let codex = changing("codex_sessions_read.jsonl", "codex_sessions_read_changed.jsonl");
    let sessions = CodexSessions::new();

    let first = sessions.messages(&codex.ctx(), &thread(PARENT), None, 4).unwrap();
    let cursor = first.next_cursor.unwrap();

    // The view opens again (a fresh read) while the first view still pages the old one.
    let again = sessions.messages(&codex.ctx(), &thread(PARENT), None, 4).unwrap();
    assert_eq!(codex.runs(), 2);
    assert_ne!(again.messages, first.messages);

    let stale = sessions.messages(&codex.ctx(), &thread(PARENT), Some(&cursor), 200);
    assert_eq!(code(stale), IpcErrorCode::Changed);

    let next = sessions.messages(&codex.ctx(), &thread(PARENT), again.next_cursor.as_deref(), 200);
    assert_eq!(
        next.unwrap().messages.len(),
        8,
        "the cursor of the newer read still pages"
    );
}
