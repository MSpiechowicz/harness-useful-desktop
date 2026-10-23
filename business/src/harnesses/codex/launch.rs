//! `codex app-server`: the command line of a Codex chat (Codex CLI 0.154, the plan's "Protocol"
//! and decision 8).
//!
//! The chat talks JSON-RPC over stdin and stdout (`--listen stdio://`, the default), one process
//! per chat. Every launch switches Codex's own update check and update prompts off
//! ([`APP_SERVER_ARGS`]); a launch that trusts the project once trusts it for this process only
//! ([`trust_once_args`]). Nothing else of the user's `config.toml` is overridden here, and nothing
//! is written to it (a chat with a memory identity passes it on to Codex's harness-memory servers
//! per thread, see `agent/codex/memory.rs`). The chat engine names the thread after the
//! handshake (`thread/start`, or `thread/resume` for a resumed chat) with the project as its
//! working directory and the chat's approval mode, so no session, model or approval flag is ever
//! on the command line. A resumed thread id is a lowercase UUID.

use super::trust::trust_once_args;
use crate::error::{BusinessError, Result};
use crate::harness::{LaunchRequest, rpc_exec_spec};
use crate::process::ExecSpec;

/// The arguments of every Codex chat launch: the app server, without the update check at startup
/// and with the `in_app_updates` feature off (`-c features.<name>=false`, which is what
/// `--disable <name>` sets).
pub const APP_SERVER_ARGS: [&str; 5] = [
    "app-server",
    "-c",
    "check_for_update_on_startup=false",
    "-c",
    "features.in_app_updates=false",
];

/// `codex app-server <update overrides> [<trust once>]` in the project. A resumed thread is
/// checked here and named by the engine.
pub fn codex_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    if let Some(id) = request.resume
        && !is_thread_id(id)
    {
        return Err(BusinessError::InvalidInput(
            "a Codex thread id is a lowercase UUID".to_owned(),
        ));
    }

    let mut args = APP_SERVER_ARGS.map(str::to_owned).to_vec();

    if request.trust_once {
        args.extend(trust_once_args(request.project)?);
    }

    rpc_exec_spec(request.binary, request.project, &args)
}

/// True for a Codex thread id: a lowercase, hyphenated UUID.
pub fn is_thread_id(id: &str) -> bool {
    uuid::Uuid::try_parse(id).is_ok_and(|parsed| parsed.hyphenated().to_string() == id)
}
