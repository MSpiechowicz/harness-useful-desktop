//! `claude` in stream-json mode: the command line of a Claude Code chat (Claude Code 2.1.304, the
//! plan's "Protocol").
//!
//! The chat talks to the CLI over stdin and stdout: user messages and control requests in,
//! frames out (`--output-format stream-json --verbose --input-format stream-json`), partial
//! messages streamed (`--include-partial-messages`) and every permission prompt sent to the app
//! as a control request (`--permission-prompt-tool stdio`), so approvals stay with the user. No
//! flag ever skips them, strips a login method or bypasses the permission mode. An approval
//! override of Always ask or Write starts Claude Code in its `default` mode (`--permission-mode
//! default`, see [`modes`](super::modes)), Auto in its `auto` mode (`--permission-mode auto`,
//! whose classifier still sends the asks it does not approve). Yolo is refused, and without an
//! override the user's own mode applies. A resumed session is `--resume <id>`, its id a Claude session UUID; the chat
//! engine names a new session itself (`--session-id`) and starts the CLI again for a new session,
//! a switch, a fork or a clone. No model flag is passed here: the launch request names none (the
//! chat engine adds `--model=<id>` for a picked model, and Harness Skills' default model role with
//! its `--effort` for a new session without a pick). A helper chat (AI naming) runs without tools, MCP servers, thinking or a saved session
//! ([`UTILITY_ARGS`]), also on the default model, and never in the `auto` mode.

use super::modes::{WIRE_AUTO, WIRE_DEFAULT};
use super::sessions::is_session_id;
use crate::error::{BusinessError, Result};
use crate::harness::{LaunchRequest, flag_with, rpc_exec_spec};
use crate::model::ApprovalMode;
use crate::process::ExecSpec;

/// The arguments of every Claude Code chat launch.
pub const STREAM_ARGS: [&str; 8] = [
    "--output-format",
    "stream-json",
    "--verbose",
    "--input-format",
    "stream-json",
    "--permission-prompt-tool",
    "stdio",
    "--include-partial-messages",
];

/// The arguments of a helper chat: no tools, no MCP servers (`--strict-mcp-config` without an
/// `--mcp-config` loads none), no thinking, nothing saved.
pub const UTILITY_ARGS: [&str; 5] = [
    "--tools",
    "",
    "--strict-mcp-config",
    "--thinking=disabled",
    "--no-session-persistence",
];

/// `claude <stream args> [--permission-mode default|auto] [--resume <id> | <utility args>]` in
/// the project. A helper chat asked for Auto starts in `default`.
pub fn claude_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    let mut args: Vec<String> = STREAM_ARGS.map(str::to_owned).to_vec();

    match request.approval_override {
        None => {}
        Some(ApprovalMode::Auto) if !request.utility => args.extend(flag_with("--permission-mode", WIRE_AUTO)),
        Some(ApprovalMode::AlwaysAsk | ApprovalMode::Write | ApprovalMode::Auto) => {
            args.extend(flag_with("--permission-mode", WIRE_DEFAULT))
        }
        Some(ApprovalMode::Yolo) => {
            return Err(BusinessError::Unsupported(
                "Claude Code chats have no Yolo mode: choose Always ask, Write or Auto".to_owned(),
            ));
        }
    }

    if let Some(id) = request.resume {
        if !is_session_id(id) {
            return Err(BusinessError::InvalidInput(
                "a Claude Code session id is a lowercase UUID".to_owned(),
            ));
        }

        args.extend(flag_with("--resume", id));
    }

    if request.utility {
        args.extend(UTILITY_ARGS.map(str::to_owned));
    }

    rpc_exec_spec(request.binary, request.project, &args)
}
