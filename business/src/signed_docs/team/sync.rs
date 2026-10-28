//! One refresh round of the team copies: `<plural> list` for the global documents and for each
//! known project root, collected into a [`TeamFetch`] that [`apply_round`] writes. Nothing waits
//! for it: the app uses the copies already on disk.
//!
//! Each scope is judged on its own: the global copies by the global query, the copies of a
//! repository key by the queries of the roots that have it. A failed query only keeps the copies
//! of its own scope.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::folder::{SyncCounts, TeamFolder};
use super::{ProjectKey, TeamRow};
use crate::ecosystem::memory_team_docs::list_team_docs;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::process::ProcessRunner;
use crate::signed_docs::{DocKind, DocStore};

/// What one round listed.
#[derive(Debug, Default)]
pub struct TeamFetch {
    /// Every row listed by any query, by id.
    pub rows: BTreeMap<String, TeamRow>,
    /// The identity of each project root whose query answered.
    pub projects: BTreeMap<String, ProjectKey>,
    /// True when the global query read every page: global copies it did not list are removed.
    pub global_listed: bool,
    /// The repository keys a root query read every page of: their copies it did not list are
    /// removed.
    pub listed_keys: BTreeSet<String>,
    /// Known roots that are no longer folders (an unmounted drive, a moved checkout): not asked
    /// for, and their mapping and copies are kept.
    pub skipped: Vec<String>,
    /// Roots whose query failed: their mapping and copies are kept.
    pub failed: Vec<String>,
    /// Roots that are not projects (the filesystem root, the home folder) or that the server
    /// refused: forgotten.
    pub refused: Vec<String>,
    /// True when every query succeeded and read every page.
    pub complete: bool,
    /// The first failure, when a query failed.
    pub problem: Option<BusinessError>,
}

/// True when `root` can be a project of team documents: not the filesystem root and not the home
/// folder, which Harness Memory refuses as projects.
pub fn usable_root(env: &Env, root: &str) -> bool {
    let path = Path::new(root);
    let home = dunce::canonicalize(&env.home).unwrap_or_else(|_| env.home.clone());

    path.parent().is_some() && path != home && path != env.home
}

/// Lists the team documents of kind `K` on `server`: the global ones, then those of each of
/// `roots` (project roots). A failed query is recorded and the others still run. A root that is
/// no longer a folder is not asked for ([`TeamFetch::skipped`]), nor one that cannot be a project
/// ([`usable_root`]).
pub async fn fetch_team<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    roots: &[String],
) -> TeamFetch {
    let mut fetch = TeamFetch {
        complete: true,
        ..TeamFetch::default()
    };

    let mut present = Vec::new();

    for root in roots {
        if !usable_root(env, root) {
            fetch.refused.push(root.clone());
        } else if Path::new(root).is_dir() {
            present.push(root);
        } else {
            fetch.skipped.push(root.clone());
        }
    }

    let mut queries = std::iter::once(None).chain(present.into_iter().map(Some));

    while let Some(root) = queries.next() {
        match list_team_docs::<K>(env, runner, server, root.map(Path::new)).await {
            Ok(list) => {
                if !list.complete {
                    fetch.complete = false;
                    fetch.problem.get_or_insert(BusinessError::Protocol(format!(
                        "harness-memory listed more remote {} than this app reads",
                        K::PLURAL
                    )));
                }

                match (root, &list.project) {
                    (None, _) => fetch.global_listed = list.complete,
                    (Some(root), Some(project)) => {
                        if list.complete {
                            fetch.listed_keys.insert(project.key.clone());
                        } else {
                            fetch.failed.push(root.clone());
                        }

                        fetch.projects.insert(root.clone(), project.clone());
                    }
                    (Some(root), None) => fetch.failed.push(root.clone()),
                }

                for row in list.rows {
                    fetch.rows.insert(row.id.clone(), row);
                }
            }
            // The server says this is no project it keeps documents for: forgotten, not a failure.
            Err(BusinessError::Forbidden(_) | BusinessError::InvalidInput(_)) if root.is_some() => {
                fetch.refused.extend(root.cloned());
            }
            Err(error) => {
                fetch.failed.extend(root.cloned());
                fetch.complete = false;

                // The global query's command failed (the team database cannot be reached, the
                // server is gone): the roots' queries would fail the same way, one process each,
                // so they wait for the next round. Their mapping and copies are kept like any
                // failed query's.
                if root.is_none() && matches!(error, BusinessError::Process(_)) {
                    fetch.failed.extend(queries.by_ref().flatten().cloned());
                }

                fetch.problem.get_or_insert(error);
            }
        }
    }

    fetch
}

/// Writes one refresh round ([`TeamFolder::apply`]) and drops the approvals of the copies it
/// removed (see [`DocStore::forget_team_approvals`]). App window only, under its write lock.
pub fn apply_round<K: DocKind>(store: &DocStore<K>, folder: &TeamFolder<K>, fetch: &TeamFetch) -> Result<SyncCounts> {
    let counts = folder.apply(fetch)?;
    store.forget_team_approvals(&counts.removed)?;

    Ok(counts)
}

/// Deletes every team copy (sharing is off) and their approvals (see
/// [`DocStore::forget_team_approvals`]). True when there was a team folder.
pub fn clear_team<K: DocKind>(store: &DocStore<K>, folder: &TeamFolder<K>) -> Result<bool> {
    let ids: Vec<String> = folder.ids().into_iter().collect();
    let cleared = folder.clear()?;

    store.forget_team_approvals(&ids)?;
    Ok(cleared)
}
