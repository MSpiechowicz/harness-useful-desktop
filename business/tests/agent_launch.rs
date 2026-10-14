//! Agent command lines per harness: RPC mode, approval override, trust once, resume and the
//! utility helper, and the flags no launch may ever carry.

mod agent_support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use agent_support::binary;
use harness_business::harness::{Harness, LaunchRequest};
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::ApprovalMode;
use harness_business::process::ResolvedBinary;
use pretty_assertions::assert_eq;

fn args(harness: &dyn Harness, launch: LaunchRequest<'_>) -> Vec<String> {
    let spec = harness.launch_spec(&launch).expect("launch spec");
    assert_eq!(spec.cwd.as_deref(), Some(launch.project));

    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

fn launch<'a>(binary: &'a ResolvedBinary, project: &'a Path) -> LaunchRequest<'a> {
    LaunchRequest {
        binary,
        project,
        resume: None,
        approval_override: None,
        trust_once: false,
        utility: false,
    }
}

#[test]
fn omp_runs_in_rpc_mode_with_optional_approval_override_and_resume() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let base = launch(&binary, &project);

    assert_eq!(args(&Omp, base), ["--mode", "rpc"]);

    let full = LaunchRequest {
        approval_override: Some(ApprovalMode::AlwaysAsk),
        resume: Some("0199c1f2"),
        trust_once: true,
        ..base
    };
    assert_eq!(
        args(&Omp, full),
        ["--mode", "rpc", "--approval-mode", "always-ask", "--resume", "0199c1f2"]
    );
}

#[test]
fn pi_passes_approve_only_for_trust_once_and_resumes_with_session() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let base = launch(&binary, &project);

    assert_eq!(args(&Pi, base), ["--mode", "rpc"]);

    let full = LaunchRequest {
        trust_once: true,
        resume: Some("pi-session-1"),
        approval_override: Some(ApprovalMode::Yolo),
        ..base
    };
    let expected = ["--mode", "rpc", "--approve", "--session", "pi-session-1"]; // guard-allow: approve-flag
    assert_eq!(args(&Pi, full), expected);
}

#[test]
fn a_utility_helper_runs_without_tools_session_or_thinking() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let helper = LaunchRequest {
        utility: true,
        ..launch(&binary, &project)
    };
    let expected = ["--mode", "rpc", "--no-session", "--no-tools", "--thinking", "off"];

    assert_eq!(args(&Omp, helper), expected);
    assert_eq!(args(&Pi, helper), expected);
}

#[test]
fn no_launch_ever_disables_dialogs_skips_approvals_or_drops_the_session() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let forbidden = ["no-ui", "auto", "no-session", "yolo-mode", "login"]; // guard-allow: login-rpc

    for harness in [&Omp as &dyn Harness, &Pi] {
        for approval in [None, Some(ApprovalMode::Write)] {
            let all = LaunchRequest {
                approval_override: approval,
                trust_once: true,
                ..launch(&binary, &project)
            };

            for arg in args(harness, all) {
                assert!(!forbidden.iter().any(|word| arg.contains(word)), "{arg}");
            }
        }
    }
}

#[test]
fn batch_script_launchers_accept_the_rpc_arguments() {
    let batch = ResolvedBinary {
        path: PathBuf::from("C:/npm/omp.cmd"),
        is_batch_script: true,
    };
    let project = PathBuf::from("C:/work/demo");

    let plain = Omp.launch_spec(&launch(&batch, &project)).expect("safe arguments");
    assert_eq!(plain.args, [OsString::from("--mode"), OsString::from("rpc")]);
}
