//! Process runner and spawners against a real child: this test binary re-executes itself and
//! runs `helper_process` in a mode chosen by `HARNESS_TEST_HELPER`.

use std::time::{Duration, Instant};

use harness_business::BusinessError;
use harness_business::process::{ExecSpec, ExitInfo, ProcessRunner, ProcessSpawner, TokioRunner, TokioSpawner};
use harness_business::testing::{DuplexSpawner, KILLED};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

mod process_support;

use process_support::helper;
#[cfg(unix)]
use process_support::{GROUP_KILL_TIMEOUTS, assert_process_gone, find_grandchild_pid, grandchild_was_killed};

/// Not a real test: the body of the helper child, see [`process_support::helper_process`].
#[test]
#[ignore = "helper child process, started by the other tests"]
fn helper_process() {
    process_support::helper_process();
}

#[tokio::test]
async fn runner_captures_exit_code_and_both_streams() {
    let output = TokioRunner.run(&helper("exit3")).await.unwrap();

    assert_eq!(output.status, Some(3));
    assert!(output.stderr.contains("helper failing on purpose"), "{output:?}");
    assert!(!output.timed_out && !output.truncated && !output.success());
}

#[tokio::test]
async fn runner_kills_a_child_that_exceeds_the_timeout() {
    let started = Instant::now();
    let output = TokioRunner
        .run(&helper("sleep").timeout(Duration::from_millis(300)))
        .await
        .unwrap();

    assert!(output.timed_out);
    assert_eq!(output.status, None);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn runner_caps_output_and_reports_truncation() {
    let output = TokioRunner.run(&helper("flood").max_output_bytes(1024)).await.unwrap();

    assert_eq!(output.stdout.len(), 1024);
    assert!(output.truncated);
    assert_eq!(output.status, Some(0));
}

#[tokio::test]
async fn relative_programs_are_rejected_and_missing_ones_not_found() {
    let relative = TokioRunner.run(&ExecSpec::new("sleep")).await;
    let missing = TokioRunner
        .run(&ExecSpec::new(std::env::temp_dir().join("no-such-binary-zq")))
        .await;

    assert!(matches!(relative, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(missing, Err(BusinessError::NotFound(_))));
    assert!(matches!(
        TokioSpawner.spawn(&ExecSpec::new("pi")),
        Err(BusinessError::InvalidInput(_))
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn runner_timeout_kills_the_whole_process_group() {
    for timeout in GROUP_KILL_TIMEOUTS {
        let output = TokioRunner.run(&helper("grandchild").timeout(timeout)).await.unwrap();

        if grandchild_was_killed(&output) {
            return;
        }
    }

    panic!("the helper never reported its grandchild within {GROUP_KILL_TIMEOUTS:?}");
}

/// A run whose future is dropped (a routine run stopped by the user) leaves nothing of its process
/// group running, not only its direct child.
#[cfg(unix)]
#[tokio::test]
async fn dropping_a_run_kills_its_whole_process_group() {
    let dir = tempfile::tempdir().expect("temp dir");
    let pid_file = dir.path().join("grandchild.pid");
    let spec = helper("grandchild_pidfile")
        .env(process_support::PID_FILE_ENV, &pid_file)
        .timeout(Duration::from_secs(60));

    let pid = tokio::select! {
        output = TokioRunner.run(&spec) => panic!("the run ended first: {output:?}"),
        pid = async {
            loop {
                if let Ok(text) = std::fs::read_to_string(&pid_file) {
                    break text.trim().parse::<i32>().expect("a pid");
                }

                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        } => pid,
    };

    // The run's future was dropped by `select!`.
    assert_process_gone(pid);
}

#[tokio::test]
async fn spawner_pipes_stdin_and_stdout() {
    let mut process = TokioSpawner.spawn(&helper("echo")).unwrap();

    process.stdin.write_all(b"hello\n").await.unwrap();
    process.stdin.flush().await.unwrap();

    let mut stdout = String::new();
    process.stdout.read_to_string(&mut stdout).await.unwrap();
    let exit = process.child.wait().await.unwrap();

    assert!(stdout.contains("echo:hello"), "{stdout:?}");
    assert_eq!(exit.code, Some(0));
}

#[tokio::test]
async fn spawner_kill_stops_the_child() {
    let mut process = TokioSpawner.spawn(&helper("sleep")).unwrap();
    assert!(process.child.id().is_some());

    process.child.start_kill().unwrap();
    let exit = process.child.wait().await.unwrap();

    assert_ne!(exit.code, Some(0));
    #[cfg(unix)]
    assert_eq!(exit.signal, Some(9));
    process.child.start_kill().unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn dropping_a_spawned_child_kills_its_process_group() {
    let process = TokioSpawner.spawn(&helper("grandchild")).unwrap();
    let mut lines = BufReader::new(process.stdout).lines();

    let mut pid = None;
    while let Some(line) = lines.next_line().await.unwrap() {
        if let Some(value) = line.strip_prefix("GRANDCHILD=") {
            pid = Some(value.parse().unwrap());
            break;
        }
    }

    drop(process.child);
    assert_process_gone(pid.unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn killing_the_group_after_exit_ends_what_the_child_left_running() {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let mut process = TokioSpawner.spawn(&helper("orphan")).unwrap();
    drop(process.stdin);

    let mut stdout = String::new();
    process.stdout.read_to_string(&mut stdout).await.unwrap();
    let exit = process.child.wait().await.unwrap();
    let pid = find_grandchild_pid(&stdout).expect("the helper reports its grandchild");

    assert_eq!(exit.code, Some(0));
    assert!(
        kill(Pid::from_raw(pid), None).is_ok(),
        "the grandchild outlives its parent"
    );

    process.child.kill_process_group();
    assert_process_gone(pid);
}

#[tokio::test]
async fn duplex_spawner_connects_the_app_to_a_scripted_peer() {
    let (spawner, mut peers) = DuplexSpawner::new();
    let spec = ExecSpec::new("/opt/fake/omp").args(["--mode", "rpc"]);

    let mut process = spawner.spawn(&spec).unwrap();
    let mut peer = peers.recv().await.unwrap();
    assert_eq!(peer.spec, spec);

    process.stdin.write_all(b"{\"type\":\"get_state\"}\n").await.unwrap();
    let mut request = vec![0_u8; 21];
    peer.stdin.read_exact(&mut request).await.unwrap();
    assert_eq!(request, b"{\"type\":\"get_state\"}\n");

    peer.stdout.write_all(b"{\"type\":\"ready\"}\n").await.unwrap();
    let mut lines = BufReader::new(process.stdout).lines();
    assert_eq!(lines.next_line().await.unwrap().unwrap(), "{\"type\":\"ready\"}");

    peer.exit(ExitInfo {
        code: Some(0),
        signal: None,
    });
    assert_eq!(process.child.wait().await.unwrap().code, Some(0));
}

#[tokio::test]
async fn duplex_spawner_reports_kills_and_injected_spawn_failures() {
    let (spawner, mut peers) = DuplexSpawner::new();

    let mut process = spawner.spawn(&ExecSpec::new("/opt/fake/pi")).unwrap();
    let peer = peers.recv().await.unwrap();
    process.child.start_kill().unwrap();

    assert_eq!(process.child.wait().await.unwrap(), KILLED);
    assert!(peer.is_killed());

    spawner.fail_next_spawn("boom");
    assert!(matches!(
        spawner.spawn(&ExecSpec::new("/opt/fake/pi")),
        Err(BusinessError::Process(_))
    ));
    assert!(spawner.spawn(&ExecSpec::new("/opt/fake/pi")).is_ok());
}
