//! `omp --mode rpc`: command line and configured approval mode.
//!
//! Flags mirror `cli/flag-tables.ts` (18.6.1): `--approval-mode <always-ask|write|yolo>` overrides
//! `tools.approvalMode` for one session and `--resume <id prefix|path>` resumes a session. Flags
//! that skip approvals or detach the extension UI are never passed, and sessions always persist.

use std::path::Path;

use serde_json::Value;

use super::config::{project_settings, user_settings};
use super::paths::omp_paths;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{LaunchRequest, flag_with, rpc_exec_spec, utility_args};
use crate::model::ApprovalMode;
use crate::process::ExecSpec;

/// The OMP default when `tools.approvalMode` is not configured (`tools/settings.ts`).
pub const DEFAULT_APPROVAL_MODE: ApprovalMode = ApprovalMode::Yolo;

/// The CLI spelling of `mode`; `None` for Auto, which OMP does not have.
pub fn cli_approval_mode(mode: ApprovalMode) -> Option<&'static str> {
    match mode {
        ApprovalMode::AlwaysAsk => Some("always-ask"),
        ApprovalMode::Write => Some("write"),
        ApprovalMode::Yolo => Some("yolo"),
        ApprovalMode::Auto => None,
    }
}

/// Parses the config/CLI spelling.
pub fn parse_approval_mode(label: &str) -> Option<ApprovalMode> {
    match label {
        "always-ask" => Some(ApprovalMode::AlwaysAsk),
        "write" => Some(ApprovalMode::Write),
        "yolo" => Some(ApprovalMode::Yolo),
        _ => None,
    }
}

/// `omp --mode rpc [--approval-mode <mode>] [--resume <id>]` in the project. An Auto override is
/// refused.
pub fn omp_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    let mut args = vec!["--mode".to_owned(), "rpc".to_owned()];

    if let Some(mode) = request.approval_override {
        let Some(label) = cli_approval_mode(mode) else {
            return Err(BusinessError::Unsupported(
                "Oh My Pi chats have no Auto mode: choose Always ask, Write or Yolo".to_owned(),
            ));
        };

        args.extend(flag_with("--approval-mode", label));
    }

    if let Some(id) = request.resume {
        args.extend(flag_with("--resume", id));
    }

    if request.utility {
        args.extend(utility_args());
    }

    rpc_exec_spec(request.binary, request.project, &args)
}

/// `tools.approvalMode` from the project `.omp/config.yml`, else the user (or profile)
/// `config.yml`, else [`DEFAULT_APPROVAL_MODE`]. Both the nested and the dotted key are read.
pub fn configured_approval_mode(env: &Env, project: Option<&Path>) -> ApprovalMode {
    let paths = omp_paths(env, project);
    let project_mode = project_settings(&paths).and_then(|(_, document)| approval_mode_in(document.value()));

    project_mode
        .or_else(|| approval_mode_in(user_settings(&paths).1.value()))
        .unwrap_or(DEFAULT_APPROVAL_MODE)
}

fn approval_mode_in(settings: Option<&Value>) -> Option<ApprovalMode> {
    let settings = settings?;
    let nested = settings.pointer("/tools/approvalMode");
    let dotted = settings.get("tools.approvalMode");

    nested.or(dotted).and_then(Value::as_str).and_then(parse_approval_mode)
}
