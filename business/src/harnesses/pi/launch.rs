//! `pi --mode rpc`: command line and the sessions directory (PI docs `cli.md`, `sessions.md`).
//!
//! The approve flag trusts project-local resources for this launch only and is passed solely for
//! the explicit "trust this project once" choice. `--session <id>` resumes a session. Sessions
//! always persist.

use std::path::PathBuf;

use super::pi_paths;
use crate::env::Env;
use crate::error::Result;
use crate::harness::{LaunchRequest, flag_with, rpc_exec_spec};
use crate::pi_family::{read_json, text_field};
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

    rpc_exec_spec(request.binary, request.project, &args)
}

/// `PI_CODING_AGENT_SESSION_DIR`, else the `sessionDir` setting, else `<agent dir>/sessions`.
/// Relative values are ignored (they would depend on the working directory).
pub fn pi_sessions_dir(env: &Env) -> PathBuf {
    let agent_dir = pi_paths(env, None).agent_dir;

    let from_env = env.var("PI_CODING_AGENT_SESSION_DIR").map(str::to_owned);
    let from_settings = || {
        read_json(&agent_dir.join("settings.json"))
            .value()
            .and_then(|settings| text_field(settings, "sessionDir"))
    };

    from_env
        .or_else(from_settings)
        .map(|raw| env.expand_home(&raw))
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| agent_dir.join("sessions"))
}
