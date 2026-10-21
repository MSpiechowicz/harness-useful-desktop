//! The MCP server names OpenCode merges from all its config layers, read from
//! `opencode debug config`, which prints the resolved config as JSON. OpenCode masks some secret
//! values in it, not all: the output is never logged, stored or returned.
//!
//! The command is not read-only: OpenCode may fetch its organization config over the network,
//! start a dependency install (`bun add @opencode-ai/plugin`) in its config directories, create
//! `opencode.jsonc`, add `$schema`, or migrate a legacy TOML config. It starts OpenCode fully, so
//! the configured plugins load and run (including any the same import just installed), and a
//! plugin's config hook can change what it reports. "Share setup" runs it only while applying an
//! import the user confirmed, never while reviewing one.
//!
//! OpenCode also merges layers the app cannot read: a remote organization config (`opencode auth
//! login`), managed settings, `OPENCODE_CONFIG_CONTENT` and the project layers of the directory it
//! runs in. A server imported under the name of one of those would be deep-merged with it, keeping
//! its headers or env. "Share setup" checks the names here before writing an OpenCode server.
//!
//! The command runs in the home directory (as the app's other OpenCode commands), with OpenCode's
//! spawn environment (no self-update), no colors, the launcher search directories, a
//! [`PROBE_TIMEOUT`] and a [`PROBE_MAX_BYTES`] cap. Its stdout goes through a file: OpenCode keeps
//! only the first 64 KiB of it in a pipe. The output is config: only the names under `mcp` are
//! kept, and errors never repeat any of it.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value;

use super::SPAWN_ENV;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::process::{ExecSpec, ProcessRunner, ResolvedBinary, launcher_search_dirs};

/// The arguments of the probe.
pub const PROBE_ARGS: [&str; 2] = ["debug", "config"];

/// Time limit of the probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Output cap of the probe; a larger config fails it.
pub const PROBE_MAX_BYTES: usize = 4 * 1024 * 1024;

/// The names of the MCP servers in OpenCode's resolved config. An error when the command could not
/// run, failed, timed out, printed more than [`PROBE_MAX_BYTES`] or something that is not a JSON
/// object whose `mcp`, if any, is an object.
pub async fn merged_mcp_names(
    env: &Env,
    runner: &dyn ProcessRunner,
    binary: &ResolvedBinary,
) -> Result<BTreeSet<String>> {
    let spec = ExecSpec::new(&binary.path)
        .args(PROBE_ARGS)
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .envs(SPAWN_ENV.iter().copied())
        .search_dirs(launcher_search_dirs(env, &binary.path))
        .timeout(PROBE_TIMEOUT)
        .max_output_bytes(PROBE_MAX_BYTES)
        .stdout_through_file();

    let output = runner.run(&spec).await?;

    if output.timed_out {
        return Err(BusinessError::Timeout("opencode debug config".to_owned()));
    }

    if !output.success() {
        return Err(BusinessError::Process("opencode debug config failed".to_owned()));
    }

    if output.truncated {
        return Err(BusinessError::InvalidInput(
            "the output of opencode debug config is too large".to_owned(),
        ));
    }

    mcp_names(&output.stdout)
}

/// The keys of `mcp` in the printed config.
fn mcp_names(stdout: &str) -> Result<BTreeSet<String>> {
    let unreadable = || BusinessError::Protocol("the output of opencode debug config is not a config".to_owned());

    let Ok(Value::Object(config)) = serde_json::from_str::<Value>(stdout.trim()) else {
        return Err(unreadable());
    };

    match config.get("mcp") {
        None => Ok(BTreeSet::new()),
        Some(Value::Object(servers)) => Ok(servers.keys().cloned().collect()),
        Some(_) => Err(unreadable()),
    }
}
