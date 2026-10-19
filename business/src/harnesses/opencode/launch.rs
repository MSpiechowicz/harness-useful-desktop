//! `opencode serve`: the command line of an OpenCode chat (the `serve` command and network
//! options of OpenCode 1.19.3).
//!
//! The server listens on a loopback address and a port the system picks, with mDNS off, so a
//! user config with `server.mdns: true` cannot make it listen on every interface. The chat engine
//! reads the port from the listening line and sets the server's username and a random password
//! itself, so the ones the app inherited are never passed on, nor a workspace id that would route
//! the server elsewhere. A resumed session, the approval mode and a helper chat's rules are set
//! over HTTP after startup, never on the command line.

use crate::error::Result;
use crate::harness::{LaunchRequest, rpc_exec_spec};
use crate::process::ExecSpec;

/// The arguments of every OpenCode chat launch.
const SERVE_ARGS: [&str; 6] = ["serve", "--hostname", "127.0.0.1", "--port", "0", "--mdns=false"];

/// Inherited variables the server never gets: the engine's own server credentials replace them,
/// and a workspace id would send the server's requests to another workspace.
const REMOVED_ENV_PREFIXES: [&str; 2] = ["OPENCODE_SERVER_", "OPENCODE_WORKSPACE_ID"];

/// `opencode serve --hostname 127.0.0.1 --port 0 --mdns=false` in the project.
pub fn opencode_launch_spec(request: &LaunchRequest<'_>) -> Result<ExecSpec> {
    let args = SERVE_ARGS.map(str::to_owned);
    let spec = rpc_exec_spec(request.binary, request.project, &args)?;

    Ok(REMOVED_ENV_PREFIXES
        .into_iter()
        .fold(spec, |spec, prefix| spec.env_remove_prefix(prefix)))
}
