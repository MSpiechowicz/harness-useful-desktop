//! What [`Env::from_process`] captures from the real process environment. The app may not change
//! its own environment (`unsafe` is forbidden), so each test runs itself again as a child process
//! with the variables set and checks the capture there.

use std::process::Command;

use harness_business::env::Env;

/// Set in the child run, so the test checks the capture instead of starting another child.
const CHILD_MARKER: &str = "HARNESS_ENV_CAPTURE_CHILD";

/// Runs test `name` of this binary in a child process with `vars` set and asserts it passed.
fn run_as_child(name: &str, vars: &[(&str, &str)]) {
    let output = Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", name, "--test-threads=1"])
        .env(CHILD_MARKER, "1")
        .envs(vars.iter().copied())
        .output()
        .expect("run the test binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_opencode_2_project_config_switch_is_captured() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        run_as_child(
            "the_opencode_2_project_config_switch_is_captured",
            &[("OPENCODE_CONFIG_PROJECT_DISABLE", "1")],
        );
        return;
    }

    let env = Env::from_process().expect("environment");

    assert_eq!(env.var("OPENCODE_CONFIG_PROJECT_DISABLE"), Some("1"));
}

#[test]
fn a_captured_claude_config_dir_joins_the_deny_list() {
    let root = std::env::temp_dir().join("harness-env-capture-claude-root");

    if std::env::var_os(CHILD_MARKER).is_none() {
        run_as_child(
            "a_captured_claude_config_dir_joins_the_deny_list",
            &[
                ("CLAUDE_CONFIG_DIR", &root.to_string_lossy()),
                ("ProgramFiles", "D:\\Apps"),
            ],
        );
        return;
    }

    let env = Env::from_process().expect("environment");
    // Windows sets `ProgramFiles` itself in every new process, over the value the parent passed.
    let program_files = if cfg!(windows) {
        std::env::var("ProgramFiles").expect("set by Windows")
    } else {
        "D:\\Apps".to_owned()
    };

    assert_eq!(env.var("ProgramFiles"), Some(program_files.as_str()));
    assert!(harness_business::security::is_denied(&root.join("history.jsonl")).is_some());
    assert!(harness_business::security::is_denied(&root.join("debug").join("x.txt")).is_some());
    assert_eq!(harness_business::security::is_denied(&root.join("settings.json")), None);
}
