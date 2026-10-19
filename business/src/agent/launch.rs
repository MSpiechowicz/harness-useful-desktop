//! What a start launches: only harnesses with chat, run through the harness's command line with
//! its spawn environment and the launcher search path of its binary.

use super::manager::AgentDeps;
use crate::error::{BusinessError, Result};
use crate::harness::{Harness, LaunchRequest};
use crate::process::{ExecSpec, launcher_search_dirs};

/// Refuses a harness whose chat the app cannot drive yet, before anything is resolved or spawned.
pub(super) fn ensure_chat(harness: &dyn Harness) -> Result<()> {
    if harness.capabilities().chat {
        return Ok(());
    }

    Err(BusinessError::Unsupported(format!(
        "chat with {} is coming soon",
        harness.info().name
    )))
}

/// The command line of `request` for the harness of `deps`.
pub(super) fn exec_spec(deps: &AgentDeps<'_>, request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    let spec = deps
        .harness
        .launch_spec(request)?
        .envs(deps.harness.spawn_env().iter().copied())
        .search_dirs(launcher_search_dirs(deps.env, &deps.binary.path));

    Ok(spec)
}
