//! `claude` in stream-json mode: the command line of a Claude Code chat (Claude Code 2.1.304, the
//! plan's "Protocol").
//!
//! The chat talks to the CLI over stdin and stdout: user messages and control requests in,
//! frames out (`--output-format stream-json --verbose --input-format stream-json`), partial
//! messages streamed (`--include-partial-messages`) and every permission prompt sent to the app
//! as a control request (`--permission-prompt-tool stdio`), so approvals stay with the user. No
//! flag ever skips them, strips a login method or bypasses the permission mode. A resumed session
//! is `--resume <id>`, its id a Claude session UUID. No model flag is passed: the chat starts on
//! the user's own default model, and the launch request names none. Helper chats (AI naming) are
//! not available yet.

use super::sessions::is_session_id;
use crate::error::{BusinessError, Result};
use crate::harness::{LaunchRequest, flag_with, rpc_exec_spec};
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

/// `claude <stream args> [--resume <id>]` in the project.
pub fn claude_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    if request.utility {
        return Err(BusinessError::Unsupported(
            "helper chats with Claude Code are not available yet".to_owned(),
        ));
    }

    let mut args: Vec<String> = STREAM_ARGS.map(str::to_owned).to_vec();

    if let Some(id) = request.resume {
        if !is_session_id(id) {
            return Err(BusinessError::InvalidInput(
                "a Claude Code session id is a lowercase UUID".to_owned(),
            ));
        }

        args.extend(flag_with("--resume", id));
    }

    rpc_exec_spec(request.binary, request.project, &args)
}
