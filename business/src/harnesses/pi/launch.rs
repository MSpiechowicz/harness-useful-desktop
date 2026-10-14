//! `pi --mode rpc`: the command line (PI docs `cli.md`).
//!
//! The approve flag trusts project-local resources for this launch only and is passed solely for
//! the explicit "trust this project once" choice. `--session <id>` resumes a session. Sessions
//! always persist.

use crate::error::Result;
use crate::harness::{LaunchRequest, flag_with, rpc_exec_spec, utility_args};
use crate::process::ExecSpec;

/// `pi --mode rpc [--approve] [--session <id>]` in the project.
pub fn pi_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    let mut args = vec!["--mode".to_owned(), "rpc".to_owned()];

    if request.trust_once {
        args.push("--approve".to_owned()); // guard-allow: approve-flag
    }

    if let Some(id) = request.resume {
        args.extend(flag_with("--session", id));
    }

    if request.utility {
        args.extend(utility_args());
    }

    rpc_exec_spec(request.binary, request.project, &args)
}
