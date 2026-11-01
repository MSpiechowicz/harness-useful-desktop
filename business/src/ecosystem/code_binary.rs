//! The `harness-code` server (Harness Code) on this machine: where it is ([`resolve_code`]), the
//! version it reports ([`code_version`]), and both together ([`installed_code`]), which is what the
//! harnesses' MCP entries name.
//!
//! The lookup, its order and its refusals are [`sidecar_binary`](super::sidecar_binary)'s, run for
//! [`CODE_SIDECAR`]. The app does not ship the server: the setup step and the Code settings run its
//! installer (see [`super::installer`]).

use super::catalog::CODE_SIDECAR;
use super::sidecar_binary::{installed_sidecar, parse_sidecar_version, resolve_sidecar, sidecar_version};
use crate::env::Env;
use crate::process::{ProcessRunner, ResolvedBinary};

/// The first `harness-code` executable that no other user can change; its version is not checked
/// (see [`installed_code`]).
pub fn resolve_code(env: &Env) -> Option<ResolvedBinary> {
    resolve_sidecar(&CODE_SIDECAR, env)
}

/// The server to register and run: [`resolve_code`]'s binary when it reports a version within the
/// version probe's 5 s limit; `None` (logged) when it does not.
pub async fn installed_code(env: &Env, runner: &dyn ProcessRunner) -> Option<ResolvedBinary> {
    installed_sidecar(&CODE_SIDECAR, env, runner).await
}

/// The version `harness-code --version` of `binary` reports (see [`parse_code_version`]), within
/// the version probe's 5 s limit.
pub async fn code_version(env: &Env, runner: &dyn ProcessRunner, binary: &ResolvedBinary) -> Option<String> {
    sidecar_version(&CODE_SIDECAR, env, runner, binary).await
}

/// The version in `harness-code --version` output: its first non-empty line must be
/// `harness-code <major.minor.patch[-pre]>` and nothing else. `None` for any other output.
pub fn parse_code_version(output: &str) -> Option<String> {
    parse_sidecar_version(&CODE_SIDECAR, output)
}
