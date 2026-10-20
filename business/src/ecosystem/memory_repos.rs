//! The repositories that have notes or scripts, for the Repository filter of the Notes and Scripts
//! views (`memory_repos_list`): `harness-memory notes repos --json` and `scripts repos --json`
//! answer `{"repos": [{"key", "label", "count"}], "global": N}`, run as
//! [`memory_cli`](super::memory_cli) runs the server's commands. Never fails: what went wrong is
//! the page's availability, with no repositories. A server that predates the command answers a
//! usage error, which is `outdated`.

use super::memory_cli::{list_availability, run_json};
use super::memory_hidden::{call, kind_args};
use super::memory_scripts::MemoryScripts;
use crate::env::Env;
use crate::model::{MemoryCopyKind, MemoryRepoEntry, MemoryReposPage, MemoryScriptsAvailability};
use crate::process::ProcessRunner;

/// The repositories with notes (or scripts, by `kind`) and how many global ones there are.
pub async fn list_repos(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    kind: MemoryCopyKind,
) -> MemoryReposPage {
    let server = match probe.cli_server(env).await {
        Ok(server) => server,
        Err(availability) => return MemoryReposPage::unavailable(availability),
    };

    let name = match kind {
        MemoryCopyKind::Notes => "notes repos",
        MemoryCopyKind::Scripts => "scripts repos",
    };
    let args = kind_args(kind, vec!["repos".into(), "--json".into()]);

    match run_json(env, runner, &server, call(name, args)).await {
        Ok(data) => MemoryReposPage {
            availability: MemoryScriptsAvailability::Available,
            repos: data
                .get("repos")
                .and_then(|repos| serde_json::from_value::<Vec<MemoryRepoEntry>>(repos.clone()).ok())
                .unwrap_or_default(),
            global: data
                .get("global")
                .and_then(serde_json::Value::as_u64)
                .map_or(0, |count| u32::try_from(count).unwrap_or(u32::MAX)),
        },
        Err(error) => MemoryReposPage::unavailable(list_availability(error)),
    }
}
