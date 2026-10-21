//! `opencode serve`: the command line of an OpenCode chat (the `serve` command and network
//! options of OpenCode 1.19.3 and 2.1.2).
//!
//! The server listens on a loopback address and a port the system picks. OpenCode 1 also gets
//! mDNS off, so a user config with `server.mdns: true` cannot make it listen on every interface;
//! OpenCode 2 has no mDNS and refuses the option. The chat engine reads the port from the
//! listening line and sets the server's username and a random password itself, so the ones the
//! app inherited are never passed on (OpenCode 2 would prefer an inherited `OPENCODE_PASSWORD` to
//! the engine's `OPENCODE_SERVER_PASSWORD`), nor a workspace id that would route the server
//! elsewhere. A resumed session and a helper chat's rules are set over HTTP after startup, never
//! on the command line. An approval mode goes into the server's `OPENCODE_CONFIG_CONTENT` (the
//! chat engine's `probe.rs`).

use crate::error::Result;
use crate::harness::{LaunchRequest, OpenCodeGeneration, rpc_exec_spec};
use crate::process::ExecSpec;

/// The arguments of every OpenCode 1 chat launch.
const SERVE_ARGS_V1: [&str; 6] = ["serve", "--hostname", "127.0.0.1", "--port", "0", "--mdns=false"];

/// The arguments of every OpenCode 2 chat launch.
const SERVE_ARGS_V2: [&str; 5] = ["serve", "--hostname", "127.0.0.1", "--port", "0"];

/// Inherited variables the server never gets: the engine's own server credentials replace them,
/// and a workspace id would send the server's requests to another workspace.
const REMOVED_ENV_PREFIXES: [&str; 3] = ["OPENCODE_SERVER_", "OPENCODE_PASSWORD", "OPENCODE_WORKSPACE_ID"];

/// `opencode serve --hostname 127.0.0.1 --port 0` in the project, with `--mdns=false` on
/// OpenCode 1.
pub fn opencode_launch_spec(request: &LaunchRequest<'_>, generation: OpenCodeGeneration) -> Result<ExecSpec> {
    let args: Vec<String> = match generation {
        OpenCodeGeneration::V1 => SERVE_ARGS_V1.map(str::to_owned).to_vec(),
        OpenCodeGeneration::V2 => SERVE_ARGS_V2.map(str::to_owned).to_vec(),
    };
    let spec = rpc_exec_spec(request.binary, request.project, &args)?;

    Ok(REMOVED_ENV_PREFIXES
        .into_iter()
        .fold(spec, |spec, prefix| spec.env_remove_prefix(prefix)))
}
