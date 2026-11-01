//! The `harness-memory` server (Harness Memory) on this machine: where it is ([`resolve_memory`]),
//! the version it reports ([`memory_version`]), and both together ([`installed_memory`]), which is
//! what the harnesses' MCP entries name.
//!
//! The lookup, its order and its refusals are [`sidecar_binary`](super::sidecar_binary)'s, run
//! for [`MEMORY_SIDECAR`]. The app does not ship the server: the setup step and the "Project
//! memory" switch run its installer (see [`super::installer`]).

use super::catalog::MEMORY_SIDECAR;
use super::sidecar_binary::{installed_sidecar, parse_sidecar_version, resolve_sidecar, sidecar_version};
use crate::env::Env;
use crate::process::{ProcessRunner, ResolvedBinary};

/// The first `harness-memory` executable that no other user can change; its version is not
/// checked (see [`installed_memory`]).
pub fn resolve_memory(env: &Env) -> Option<ResolvedBinary> {
    resolve_sidecar(&MEMORY_SIDECAR, env)
}

/// The server to register and run: [`resolve_memory`]'s binary when it reports a version within
/// the version probe's 5 s limit; `None` (logged) when it does not.
pub async fn installed_memory(env: &Env, runner: &dyn ProcessRunner) -> Option<ResolvedBinary> {
    installed_sidecar(&MEMORY_SIDECAR, env, runner).await
}

/// The version `harness-memory --version` of `binary` reports (see [`parse_memory_version`]),
/// within the version probe's 5 s limit.
pub async fn memory_version(env: &Env, runner: &dyn ProcessRunner, binary: &ResolvedBinary) -> Option<String> {
    sidecar_version(&MEMORY_SIDECAR, env, runner, binary).await
}

/// The version in `harness-memory --version` output: its first non-empty line must be
/// `harness-memory <major.minor.patch[-pre]>` and nothing else. `None` for any other output.
pub fn parse_memory_version(output: &str) -> Option<String> {
    parse_sidecar_version(&MEMORY_SIDECAR, output)
}
