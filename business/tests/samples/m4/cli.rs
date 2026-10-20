//! Extension action results: an OMP install preview (`--dry-run`), the install itself, and a
//! disable that failed although OMP exited with code 0 (so `ok` is false). `command` is the
//! install command line in both the preview and the install; only the preview's `argv` ends with
//! `--dry-run`.

use harness_business::model::CliResult;
use serde_json::Value;

use super::to_json;

const SIDEBAR: &str = "harness-useful-sidebar@harness-useful-sidebar";

const INSTALL: [&str; 6] = ["omp", "plugin", "install", SIDEBAR, "--scope", "user"];

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn dry_run() -> CliResult {
    let line = format!("[dry-run] Would install {SIDEBAR}");

    CliResult {
        ok: true,
        summary: Some(line.clone()),
        exit_code: Some(0),
        stdout: format!("{line}\n"),
        stderr: String::new(),
        truncated: false,
        timed_out: false,
        argv: argv(&[INSTALL.as_slice(), &["--dry-run"]].concat()),
        command: argv(&INSTALL),
        config_edit: None,
    }
}

fn installed() -> CliResult {
    let line = "✔ Installed harness-useful-sidebar from harness-useful-sidebar (1.0.71)";

    CliResult {
        ok: true,
        summary: Some(line.to_owned()),
        exit_code: Some(0),
        stdout: format!("{line}\n"),
        stderr: String::new(),
        truncated: false,
        timed_out: false,
        argv: argv(&INSTALL),
        command: argv(&INSTALL),
        config_edit: None,
    }
}

fn failed_with_exit_zero() -> CliResult {
    let line =
        "✘ Failed to disable harness-useful-sidebar: Error: Plugin harness-useful-sidebar not found in runtime config";

    CliResult {
        ok: false,
        summary: Some(line.to_owned()),
        exit_code: Some(0),
        stdout: String::new(),
        stderr: format!("{line}\n"),
        truncated: false,
        timed_out: false,
        argv: argv(&["omp", "plugin", "disable", "harness-useful-sidebar"]),
        command: argv(&["omp", "plugin", "disable", "harness-useful-sidebar"]),
        config_edit: None,
    }
}

/// The `ecosystem/cli_result_*` fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("ecosystem/cli_result_dry_run.json", to_json(dry_run())),
        ("ecosystem/cli_result_ok.json", to_json(installed())),
        ("ecosystem/cli_result_failed.json", to_json(failed_with_exit_zero())),
    ]
}
