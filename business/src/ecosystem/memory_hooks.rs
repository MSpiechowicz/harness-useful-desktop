//! Team hooks kept by Harness Memory: [`memory_team_docs`](super::memory_team_docs) for hooks.
//! [`MemoryHooks::support`] runs `hooks config --json` (`{"hooks": "remote"|"local",
//! "trustTeamHooks": bool}`), and `harness-memory hooks list|put|delete` list, store and remove
//! the team's hooks. The app never connects to the team database.

use std::path::Path;

use super::memory_team_docs::{
    MemoryTeamDocs, TeamDocsConfig, TeamDocsSupport, delete_team_doc, list_team_docs, put_team_doc,
};
use crate::env::Env;
use crate::error::Result;
use crate::hooks::HookKind;
use crate::hooks::team::{ProjectKey, TeamRow};
use crate::process::ProcessRunner;

pub use super::memory_team_docs::{PROBE_TIMEOUT, TeamList};

/// What `hooks config --json` says.
pub type HooksConfig = TeamDocsConfig;

/// Whether the installed server keeps team hooks, which decides whether the `hooks` and
/// `trust_team_hooks` keys may be saved and whether this machine runs team hooks.
pub type HooksSupport = TeamDocsSupport;

/// The probe of the installed server's team hooks.
pub type MemoryHooks = MemoryTeamDocs<HookKind>;

/// Every team hook a chat in `project` sees, all pages (see [`list_team_docs`]).
pub async fn list_team_hooks(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
) -> Result<TeamList> {
    list_team_docs::<HookKind>(env, runner, server, project).await
}

/// The identity the server gives the project root `project` (`hooks list --project`, one row).
pub async fn project_key(env: &Env, runner: &dyn ProcessRunner, server: &Path, project: &Path) -> Result<ProjectKey> {
    super::memory_team_docs::project_key::<HookKind>(env, runner, server, project).await
}

/// Stores `body` as a team hook (see [`put_team_doc`]).
pub async fn put_team_hook(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    body: &[u8],
    project: Option<&Path>,
    expect_sha: Option<&str>,
) -> Result<TeamRow> {
    put_team_doc::<HookKind>(env, runner, server, body, project, expect_sha).await
}

/// Removes team hook `id` while its stored version is `expect_sha` (see [`delete_team_doc`]).
pub async fn delete_team_hook(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    expect_sha: Option<&str>,
) -> Result<bool> {
    delete_team_doc::<HookKind>(env, runner, server, id, expect_sha).await
}
