//! Launching a harness in RPC mode: the request every adapter turns into a shell-free
//! [`ExecSpec`] (fixed binary, explicit argv, project as working directory, inherited environment
//! so the agent sees the user's provider configuration).

use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::model::ApprovalMode;
use crate::process::{ExecSpec, ResolvedBinary};
use crate::security::is_safe_arg;

/// What to launch; built by the agent manager after validating the start request.
#[derive(Debug, Clone, Copy)]
pub struct LaunchRequest<'a> {
    /// The detected (or user-chosen) harness binary.
    pub binary: &'a ResolvedBinary,
    /// Canonical project directory (the working directory).
    pub project: &'a Path,
    /// Session id to resume; already checked with `pi_family::session_files::is_safe_session_id`.
    pub resume: Option<&'a str>,
    /// Approval mode for this session only (harnesses with `capabilities.approvalModes`).
    pub approval_override: Option<ApprovalMode>,
    /// Trust the project's local resources for this launch only (`capabilities.projectTrust`).
    pub trust_once: bool,
}

/// `binary args…` in `project`. Batch-script wrappers (`.cmd`/`.bat`, re-parsed by `cmd.exe`)
/// only receive arguments that pass [`is_safe_arg`] once their leading dashes are removed.
pub fn rpc_exec_spec(binary: &ResolvedBinary, project: &Path, args: &[String]) -> Result<ExecSpec> {
    let batch_safe = args.iter().all(|arg| is_safe_arg(arg.trim_start_matches('-')));

    if binary.is_batch_script && !batch_safe {
        return Err(BusinessError::Forbidden(
            "unsafe argument for a batch-script launcher".to_owned(),
        ));
    }

    Ok(ExecSpec::new(&binary.path).args(args.iter().cloned()).cwd(project))
}

/// `[flag, value]` as owned arguments.
pub fn flag_with(flag: &str, value: &str) -> [String; 2] {
    [flag.to_owned(), value.to_owned()]
}
