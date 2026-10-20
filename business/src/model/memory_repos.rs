//! Which repositories the Notes and Scripts views read: every repository, or one by its key
//! (`harness-memory --all-repos`, `--repo KEY`), and the repositories that have entries
//! (`harness-memory notes repos --json`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory_scripts::MemoryScriptsAvailability;

/// Repositories to read instead of a project's (`memory_notes_list`, `memory_note_get` and the
/// other notes and scripts commands that take one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryRepos {
    /// Every repository and the global scope.
    All,
    /// The repository with this key (`git:github.com/acme/widget`) and the global scope.
    Repo {
        /// The repository key Harness Memory gives it.
        key: String,
    },
}

/// A repository that has notes or scripts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryRepoEntry {
    /// The key to give back as [`MemoryRepos::Repo`].
    pub key: String,
    /// The repository as it is shown (`github.com/acme/widget`).
    pub label: String,
    /// How many entries it has.
    pub count: u32,
}

/// The answer of `memory_repos_list`: the repositories with entries, empty unless available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryReposPage {
    /// Whether Harness Memory could be read (see [`MemoryScriptsAvailability`]).
    pub availability: MemoryScriptsAvailability,
    /// Repositories by label.
    pub repos: Vec<MemoryRepoEntry>,
    /// How many global entries there are.
    pub global: u32,
}

impl MemoryReposPage {
    /// What a page holds when Harness Memory cannot be read.
    pub fn unavailable(availability: MemoryScriptsAvailability) -> Self {
        Self {
            availability,
            repos: Vec::new(),
            global: 0,
        }
    }
}
