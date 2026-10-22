//! Concurrent OpenCode CLI session calls, replayed (see `opencode_session_support`): shared list
//! runs, the limit on commands running at once, and a delete that must not trust a list run
//! older than itself.

use crate::opencode_session_support;

use std::sync::Barrier;
use std::time::Duration;

use harness_business::harness::CliSessions;
use harness_business::harnesses::opencode::OpenCodeSessions;
use harness_business::model::IpcErrorCode;
use opencode_session_support::{Fixture, Held, MAIN, Slow, error_code, ok, opencode};
use pretty_assertions::assert_eq;

#[test]
fn concurrent_list_calls_share_one_run() {
    let fixture = Fixture::new();
    let runner = Slow::new(fixture.list_output(), Duration::from_millis(300));
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();
    let callers = 6;
    let start = Barrier::new(callers);

    let counts: Vec<usize> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..callers)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    sessions.list(&ctx).unwrap().len()
                })
            })
            .collect();

        handles.into_iter().map(|handle| handle.join().unwrap()).collect()
    });

    assert_eq!(counts, vec![2; callers]);
    assert_eq!(runner.runs(), 1);
}

#[test]
fn at_most_two_commands_run_at_once() {
    let fixture = Fixture::new();
    let runner = Slow::new(fixture.export_output(), Duration::from_millis(100));
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();
    let callers = 6;
    let start = Barrier::new(callers);

    // Each first page exports again, so every caller runs the CLI.
    std::thread::scope(|scope| {
        for _ in 0..callers {
            scope.spawn(|| {
                start.wait();
                sessions.messages(&ctx, &opencode(MAIN), None, 10).unwrap();
            });
        }
    });

    assert_eq!(runner.runs(), callers);
    assert!(runner.peak() <= 2, "{} ran at once", runner.peak());
}

#[test]
fn a_delete_does_not_trust_a_list_run_older_than_itself() {
    let fixture = Fixture::new();
    let without_main = fixture
        .recorded("session-list.json", fixture.project())
        .replace(MAIN, "ses_older_than_main");
    let runner = Held::answering(
        1,
        [
            ok(without_main.clone()),
            ok(without_main),
            fixture.list_output(),
            ok("Session deleted"),
        ],
    );
    let ctx = fixture.ctx(fixture.project(), &runner);
    let sessions = OpenCodeSessions::new();

    // The list is cached before MAIN existed, then an unlisted delete starts a new list that is
    // held while MAIN is created.
    sessions.list(&ctx).unwrap();

    std::thread::scope(|scope| {
        let unlisted = scope.spawn(|| sessions.delete(&ctx, &opencode("ses_listed_nowhere")));
        runner.wait_held();

        let main = scope.spawn(|| sessions.delete(&ctx, &opencode(MAIN)));

        // Gives the delete of MAIN time to reach the held list before it lands.
        std::thread::sleep(Duration::from_millis(200));
        runner.release();

        assert_eq!(error_code(unlisted.join().unwrap()), IpcErrorCode::NotFound);
        main.join().unwrap().expect("MAIN is deleted");
    });

    let commands: Vec<String> = runner.argv().iter().map(|argv| argv[..2].join(" ")).collect();
    assert_eq!(
        commands,
        ["session list", "session list", "session list", "session delete"]
    );
}
