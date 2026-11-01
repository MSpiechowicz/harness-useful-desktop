//! The `harness-knowledge` server (Harness Knowledge) on this machine: where it is
//! ([`resolve_knowledge`]), the version it reports ([`knowledge_version`]), and both together
//! ([`installed_knowledge`]), which is what the harnesses' MCP entries name.
//!
//! The lookup, its order and its refusals are [`sidecar_binary`](super::sidecar_binary)'s, run
//! for [`KNOWLEDGE_SIDECAR`]. The app does not ship the server: the setup step and the "Project
//! knowledge" switch run its installer (see [`super::installer`]).

use super::catalog::KNOWLEDGE_SIDECAR;
use super::sidecar_binary::{installed_sidecar, parse_sidecar_version, resolve_sidecar, sidecar_version};
use crate::env::Env;
use crate::process::{ProcessRunner, ResolvedBinary};

/// The variable that names the chat Harness Desktop started a harness for, so the servers the
/// harness runs know which chat they serve (`HARNESS_KNOWLEDGE_SESSION`, set to the same key as
/// Harness Memory's).
pub const KNOWLEDGE_SESSION_VAR: &str = "HARNESS_KNOWLEDGE_SESSION";

/// The first `harness-knowledge` executable that no other user can change; its version is not
/// checked (see [`installed_knowledge`]).
pub fn resolve_knowledge(env: &Env) -> Option<ResolvedBinary> {
    resolve_sidecar(&KNOWLEDGE_SIDECAR, env)
}

/// The server to register and run: [`resolve_knowledge`]'s binary when it reports a version within
/// the version probe's 5 s limit; `None` (logged) when it does not.
pub async fn installed_knowledge(env: &Env, runner: &dyn ProcessRunner) -> Option<ResolvedBinary> {
    installed_sidecar(&KNOWLEDGE_SIDECAR, env, runner).await
}

/// The version `harness-knowledge --version` of `binary` reports (see [`parse_knowledge_version`]),
/// within the version probe's 5 s limit.
pub async fn knowledge_version(env: &Env, runner: &dyn ProcessRunner, binary: &ResolvedBinary) -> Option<String> {
    sidecar_version(&KNOWLEDGE_SIDECAR, env, runner, binary).await
}

/// The version in `harness-knowledge --version` output: its first non-empty line must be
/// `harness-knowledge <major.minor.patch[-pre]>` and nothing else. `None` for any other output.
pub fn parse_knowledge_version(output: &str) -> Option<String> {
    parse_sidecar_version(&KNOWLEDGE_SIDECAR, output)
}
