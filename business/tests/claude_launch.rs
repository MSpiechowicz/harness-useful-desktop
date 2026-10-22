//! The command line of a Claude Code chat: stream-json mode in the project, `--resume` only with a
//! Claude session id, no model flag, helper chats without tools, MCP servers, thinking or a saved
//! session, `--permission-mode default` for an Always ask or Write override (Yolo refused), and
//! never a flag that skips approvals, strips a login method, loads other settings or leaves the
//! default permission mode.

use std::ffi::OsString;
use std::path::PathBuf;

use harness_business::harness::{Harness, LaunchRequest};
use harness_business::harnesses::Claude;
use harness_business::model::{ApprovalMode, IpcErrorCode};
use harness_business::process::ResolvedBinary;
use pretty_assertions::assert_eq;

const STREAM: [&str; 8] = [
    "--output-format",
    "stream-json",
    "--verbose",
    "--input-format",
    "stream-json",
    "--permission-prompt-tool",
    "stdio",
    "--include-partial-messages",
];

const SESSION: &str = "c1262da3-a66b-42b5-a9ee-32bb98f2045d";

struct Setup {
    binary: ResolvedBinary,
    project: PathBuf,
}

impl Setup {
    fn new(path: &str, batch: bool) -> Self {
        Self {
            binary: ResolvedBinary {
                path: PathBuf::from(path),
                is_batch_script: batch,
            },
            project: PathBuf::from("/work/demo"),
        }
    }

    fn request(&self, resume: Option<&'static str>) -> LaunchRequest<'_> {
        LaunchRequest {
            binary: &self.binary,
            project: &self.project,
            resume,
            approval_override: None,
            trust_once: false,
            utility: false,
        }
    }
}

fn argv(request: &LaunchRequest<'_>) -> Vec<String> {
    let spec = Claude.launch_spec(request).expect("a launch spec");

    assert_eq!(spec.cwd.as_deref(), Some(request.project), "runs in the project");
    assert_eq!(spec.program, request.binary.path);
    assert!(spec.env.is_empty(), "the manager adds the spawn environment");

    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[test]
fn a_new_chat_runs_stream_json_with_permission_prompts_on_stdio() {
    let setup = Setup::new("/usr/bin/claude", false);

    assert_eq!(argv(&setup.request(None)), STREAM);
}

#[test]
fn a_resumed_chat_adds_resume_with_its_session_id() {
    let setup = Setup::new("/usr/bin/claude", false);

    let mut expected: Vec<&str> = STREAM.to_vec();
    expected.extend(["--resume", SESSION]);

    assert_eq!(argv(&setup.request(Some(SESSION))), expected);
}

#[test]
fn only_a_lowercase_session_uuid_is_resumed() {
    let setup = Setup::new("/usr/bin/claude", false);

    for id in [
        "C1262DA3-A66B-42B5-A9EE-32BB98F2045D",
        "c1262da3a66b42b5a9ee32bb98f2045d",
        "c1262da3-a66b-42b5-a9ee-32bb98f2045",
        "ses_ea2843d6fffeeHJKZwBKUXqULv",
        "../c1262da3-a66b-42b5-a9ee-32bb98f2045d",
        "--fork-session",
    ] {
        let refused = Claude.launch_spec(&setup.request(Some(id))).expect_err(id);
        assert_eq!(refused.code(), IpcErrorCode::InvalidInput, "{id}");
    }
}

#[test]
fn a_helper_chat_runs_without_tools_mcp_servers_thinking_or_a_saved_session_on_the_default_model() {
    let setup = Setup::new("/usr/bin/claude", false);
    let request = LaunchRequest {
        utility: true,
        approval_override: Some(ApprovalMode::AlwaysAsk),
        ..setup.request(None)
    };

    let mut expected: Vec<&str> = STREAM.to_vec();
    expected.extend([
        "--permission-mode",
        "default",
        "--tools",
        "",
        "--strict-mcp-config",
        "--thinking=disabled",
        "--no-session-persistence",
    ]);

    assert_eq!(argv(&request), expected);
}

/// Flags and modes no launch may carry, assembled at run time (the guard scans this file too).
fn forbidden() -> Vec<String> {
    let flag = |name: &str| ["--", name].concat();

    vec![
        "bypassPermissions".to_owned(),
        "auto".to_owned(),
        "dontAsk".to_owned(),
        "acceptEdits".to_owned(),
        flag(&["dangerously", "skip-permissions"].join("-")),
        flag(&["allow", "dangerously", "skip-permissions"].join("-")),
        flag("bare"),
        flag("permission-prompts"),
        flag("settings"),
        flag("setting-sources"),
        flag("allowedTools"),
        flag("allowed-tools"),
        flag("add-dir"),
        flag("model"),
    ]
}

#[test]
fn no_launch_skips_approvals_strips_a_login_names_a_model_or_leaves_the_default_mode() {
    let setup = Setup::new("/usr/bin/claude", false);
    let forbidden = forbidden();

    for resume in [None, Some(SESSION)] {
        for approval in [None, Some(ApprovalMode::AlwaysAsk), Some(ApprovalMode::Write)] {
            for (trust_once, utility) in [(false, false), (true, false), (false, true), (true, true)] {
                let request = LaunchRequest {
                    approval_override: approval,
                    trust_once,
                    utility,
                    ..setup.request(resume)
                };
                let args = argv(&request);

                for arg in &args {
                    assert!(!forbidden.iter().any(|word| arg.contains(word.as_str())), "{arg}");
                }

                for (at, arg) in args.iter().enumerate() {
                    if arg.starts_with("--permission-mode") {
                        assert_eq!(arg, "--permission-mode", "{args:?}");
                        assert_eq!(args.get(at + 1).map(String::as_str), Some("default"), "{args:?}");
                    }

                    if arg.starts_with("--permission-prompt-tool") {
                        assert_eq!(arg, "--permission-prompt-tool", "{args:?}");
                        assert_eq!(args.get(at + 1).map(String::as_str), Some("stdio"), "{args:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn always_ask_and_write_start_in_the_default_mode_and_no_override_keeps_the_users() {
    let setup = Setup::new("/usr/bin/claude", false);

    for approval in [ApprovalMode::AlwaysAsk, ApprovalMode::Write] {
        let request = LaunchRequest {
            approval_override: Some(approval),
            ..setup.request(Some(SESSION))
        };

        let mut expected: Vec<&str> = STREAM.to_vec();
        expected.extend(["--permission-mode", "default", "--resume", SESSION]);
        assert_eq!(argv(&request), expected, "{approval:?}");
    }

    assert!(!argv(&setup.request(None)).iter().any(|arg| arg == "--permission-mode"));
}

#[test]
fn a_yolo_override_is_refused() {
    let setup = Setup::new("/usr/bin/claude", false);
    let request = LaunchRequest {
        approval_override: Some(ApprovalMode::Yolo),
        ..setup.request(None)
    };

    let refused = Claude.launch_spec(&request).expect_err("no Yolo mode");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
}

#[test]
fn a_batch_script_launcher_accepts_the_arguments() {
    let setup = Setup::new("C:/npm/claude.cmd", true);

    let spec = Claude
        .launch_spec(&setup.request(Some(SESSION)))
        .expect("every argument is batch-safe");
    assert_eq!(spec.args.last(), Some(&OsString::from(SESSION)));
}
