//! The blocking runner against a real child: this test binary re-executes itself and runs
//! `process_support::helper_process` in a mode chosen by `HARNESS_TEST_HELPER` (no shell is involved).

use std::time::{Duration, Instant};

use harness_business::BusinessError;
use harness_business::process::{BlockingRunner, ExecSpec, StdRunner};

use crate::process_support;

use process_support::helper;
use process_support::{GROUP_KILL_TIMEOUTS, grandchild_was_killed};
#[cfg(unix)]
use process_support::{assert_process_gone, find_grandchild_pid};

#[test]
fn a_normal_exit_captures_both_streams_and_the_spec_environment() {
    let output = StdRunner
        .run(&helper("hello").env("GREETING_NAME", "the helper"))
        .unwrap();

    assert_eq!(output.status, Some(0));
    assert!(output.stdout.contains("hello from the helper"), "{output:?}");
    assert!(output.stderr.contains("a note on stderr"), "{output:?}");
    assert!(output.success() && !output.truncated && !output.timed_out);
}

#[test]
fn a_nonzero_exit_is_reported_not_an_error() {
    let output = StdRunner.run(&helper("exit3")).unwrap();

    assert_eq!(output.status, Some(3));
    assert!(output.stderr.contains("helper failing on purpose"), "{output:?}");
    assert!(!output.success() && !output.timed_out && !output.truncated);
}

#[test]
fn a_child_that_exceeds_the_timeout_is_killed() {
    let started = Instant::now();
    let output = StdRunner
        .run(&helper("sleep").timeout(Duration::from_millis(300)))
        .unwrap();

    assert!(output.timed_out);
    assert_eq!(output.status, None);
    assert!(!output.success());
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn each_stream_is_capped_and_truncation_reported() {
    let output = StdRunner.run(&helper("flood").max_output_bytes(1024)).unwrap();

    assert_eq!(output.stdout.len(), 1024);
    assert_eq!(output.stderr.len(), 1024);
    assert!(output.truncated);
    assert_eq!(
        output.status,
        Some(0),
        "the excess is drained, so the child never blocks"
    );
}

#[test]
fn relative_programs_are_rejected_and_missing_ones_not_found() {
    let relative = StdRunner.run(&ExecSpec::new("sleep"));
    let missing = StdRunner.run(&ExecSpec::new(std::env::temp_dir().join("no-such-binary-zq")));

    assert!(matches!(relative, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(missing, Err(BusinessError::NotFound(_))));
}

/// On Windows the process tree is killed (`taskkill /T`), there is no process group.
#[test]
fn a_timeout_kills_the_whole_process_group() {
    for timeout in GROUP_KILL_TIMEOUTS {
        let output = StdRunner.run(&helper("grandchild").timeout(timeout)).unwrap();

        if grandchild_was_killed(&output) {
            return;
        }
    }

    panic!("the helper never reported its grandchild within {GROUP_KILL_TIMEOUTS:?}");
}

#[cfg(unix)]
#[test]
fn a_normal_exit_returns_promptly_while_a_grandchild_holds_the_pipes() {
    let started = Instant::now();
    let output = StdRunner
        .run(&helper("exit_leaving_grandchild").timeout(Duration::from_secs(120)))
        .unwrap();

    // Not the 120 s timeout: the drain grace starts when the helper exits.
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "took {:?}",
        started.elapsed()
    );
    assert_eq!(output.status, Some(0), "{output:?}");
    assert!(output.success() && !output.timed_out, "{output:?}");

    // The output written before the exit is kept, and the grandchild is killed with the group.
    let pid = find_grandchild_pid(&output.stdout).expect("the helper printed its grandchild");
    assert_process_gone(pid);
}
