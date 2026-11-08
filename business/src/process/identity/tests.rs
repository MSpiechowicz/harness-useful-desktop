use super::*;

#[test]
fn proc_stat_fields_count_from_the_last_parenthesis() {
    let stat = "4242 (open (code) x) S 1 4242 4242 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 12 0 987654 \
                1000 200 18446744073709551615";

    assert_eq!(parse_proc_stat(stat), Some(("S".to_owned(), 4242, 987_654)));
    assert_eq!(parse_proc_stat("garbage"), None);
}

fn running(observation: Observation) -> Observed {
    match observation {
        Observation::Running(observed) => observed,
        other => panic!("not running: {other:?}"),
    }
}

#[test]
fn ps_output_gives_group_start_and_command() {
    let line = "  501 Ss   Tue Oct  7 09:05:01 2026     /Users/u/.opencode/bin/opencode serve --port 0\n";
    let observed = running(parse_ps_line(line, 501));

    assert_eq!(observed.started, "Tue Oct 7 09:05:01 2026");
    assert_eq!(observed.command.last().map(String::as_str), Some("0"));
    assert!(observed.group_leader);

    assert!(!running(parse_ps_line(line, 777)).group_leader);
    assert_eq!(
        parse_ps_line("  501 Z    Tue Oct  7 09:05:01 2026 x\n", 501),
        Observation::Gone
    );
    assert_eq!(parse_ps_line("", 501), Observation::Unknown);
}

#[test]
fn only_a_silent_exit_1_from_ps_is_a_gone_process() {
    assert_eq!(ps_observation(Some(1), "", "", 501), Observation::Gone);
    assert_eq!(ps_observation(Some(1), "\n", "\n", 501), Observation::Gone);

    assert_eq!(
        ps_observation(Some(1), "ps: something went wrong\n", "", 501),
        Observation::Unknown
    );
    assert_eq!(
        ps_observation(Some(1), "", "ps: Operation not permitted\n", 501),
        Observation::Unknown
    );
    assert_eq!(ps_observation(Some(2), "", "", 501), Observation::Unknown);
    assert_eq!(ps_observation(None, "", "", 501), Observation::Unknown);
    assert_eq!(ps_observation(Some(0), "garbage", "", 501), Observation::Unknown);
}

#[test]
fn only_a_missing_process_is_gone_after_a_failed_read() {
    use std::io::{Error, ErrorKind};

    assert_eq!(unreadable(&Error::from(ErrorKind::NotFound)), Observation::Gone);
    assert_eq!(unreadable(&Error::from_raw_os_error(ESRCH)), Observation::Gone);
    assert_eq!(
        unreadable(&Error::from(ErrorKind::PermissionDenied)),
        Observation::Unknown
    );
    assert_eq!(unreadable(&Error::other("transient")), Observation::Unknown);
}

#[test]
fn an_owner_that_cannot_be_observed_counts_as_alive() {
    let identity = ProcessIdentity {
        pid: 42,
        started: "boot:100".to_owned(),
        args: vec!["serve".to_owned()],
    };
    let observed = |started: &str| {
        Observation::Running(Observed {
            started: started.to_owned(),
            command: vec!["/bin/app".to_owned(), "serve".to_owned()],
            group_leader: true,
        })
    };

    assert!(identity.may_be_running(&Observation::Unknown));
    assert!(identity.may_be_running(&observed("boot:100")));
    assert!(!identity.may_be_running(&observed("boot:999")), "the pid was reused");
    assert!(!identity.may_be_running(&Observation::Gone));
}

/// A `/proc` reader answering the stat file with `stat()` and the command line with
/// `cmdline()`.
#[cfg(target_os = "linux")]
fn proc_files(
    stat: impl Fn() -> std::io::Result<Vec<u8>>,
    cmdline: impl Fn() -> std::io::Result<Vec<u8>>,
) -> impl Fn(&str) -> std::io::Result<Vec<u8>> {
    move |path| if path.ends_with("/stat") { stat() } else { cmdline() }
}

#[cfg(target_os = "linux")]
#[test]
fn proc_read_errors_other_than_a_missing_process_are_unknown() {
    use std::io::{Error, ErrorKind};

    let stat = || Ok(b"42 (app) S 1 42 42 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 100 0 0".to_vec());
    let cmdline = || Ok(b"/bin/app\0serve\0".to_vec());
    let refused = || Err(Error::from(ErrorKind::PermissionDenied));

    let observed = running(observe_proc(42, "boot", proc_files(stat, cmdline)));
    assert_eq!(observed.started, "boot:100");
    assert_eq!(observed.command, ["/bin/app", "serve"]);

    assert_eq!(
        observe_proc(42, "boot", proc_files(refused, cmdline)),
        Observation::Unknown
    );
    assert_eq!(
        observe_proc(42, "boot", proc_files(stat, refused)),
        Observation::Unknown
    );

    let garbled = || Ok(b"garbage".to_vec());
    assert_eq!(
        observe_proc(42, "boot", proc_files(garbled, cmdline)),
        Observation::Unknown
    );

    let missing = || Err(Error::from(ErrorKind::NotFound));
    assert_eq!(
        observe_proc(42, "boot", proc_files(missing, cmdline)),
        Observation::Gone
    );
    assert_eq!(observe_proc(42, "boot", proc_files(stat, missing)), Observation::Gone);

    let exited = || Err(Error::from_raw_os_error(ESRCH));
    assert_eq!(observe_proc(42, "boot", proc_files(exited, cmdline)), Observation::Gone);

    let zombie = || Ok(b"42 (app) Z 1 42 42 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 100 0 0".to_vec());
    assert_eq!(observe_proc(42, "boot", proc_files(zombie, cmdline)), Observation::Gone);
}

#[test]
fn the_command_line_must_end_with_the_recorded_arguments() {
    let command: Vec<String> = ["/usr/bin/env", "bun", "/x/opencode", "serve", "--port", "0"]
        .map(str::to_owned)
        .to_vec();

    assert!(ends_with_args(&command, &["serve".to_owned(), "--port 0".to_owned()]));
    assert!(ends_with_args(&command, &[]));
    assert!(!ends_with_args(&command, &["serve".to_owned()]));
}

fn running_with(command: &[&str]) -> Observation {
    Observation::Running(Observed {
        started: "boot:100".to_owned(),
        command: command.iter().map(|word| (*word).to_owned()).collect(),
        group_leader: true,
    })
}

#[test]
fn capture_waits_for_a_program_that_is_still_loading() {
    let mut reads = 0;

    let identity = ProcessIdentity::capture_from(42, &[OsString::from("300")], || {
        reads += 1;

        if reads <= CAPTURE_ATTEMPTS * 2 {
            running_with(&[])
        } else {
            running_with(&["/usr/bin/sleep", "300"])
        }
    });

    assert_eq!(identity.expect("captured").started, "boot:100");
}

#[test]
fn capture_gives_up_soon_on_a_command_line_that_does_not_match() {
    let mut reads = 0;

    let identity = ProcessIdentity::capture_from(42, &[OsString::from("300")], || {
        reads += 1;
        running_with(&["/usr/bin/sleep", "1"])
    });

    assert_eq!(identity, None);
    assert_eq!(reads, CAPTURE_ATTEMPTS);
}

#[test]
fn capture_stops_at_once_when_the_process_is_gone() {
    let mut reads = 0;

    let identity = ProcessIdentity::capture_from(42, &[OsString::from("300")], || {
        reads += 1;
        Observation::Gone
    });

    assert_eq!(identity, None);
    assert_eq!(reads, 1);
}
