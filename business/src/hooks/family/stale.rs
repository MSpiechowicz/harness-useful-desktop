//! The automatic update of the Harness family hooks: only what is stale in the files as they are
//! read under the write lock, so an edit that landed since the dialog last looked is left alone.

use std::time::SystemTime;

use super::{HookCapabilities, HookSeed, HookStore, Result, Switch, apply_listed, family_status};

/// Brings the stale family hooks up to the current catalog version (and deletes the untouched
/// files of retired seeds), as desktop writes. Staleness is worked out from one listing of
/// `store` (`capabilities`, `installed` as in [`family_status`]) and every rewrite and delete is
/// guarded by the sha of that listing: call it under the hooks write lock. A file the user edited
/// is never touched. Every hook it rewrites is switched on, also one the user had switched off.
/// Returns the seeds changed.
pub fn update_stale_family(
    store: &HookStore,
    capabilities: HookCapabilities,
    installed: HookCapabilities,
    now: SystemTime,
) -> Result<Vec<HookSeed>> {
    let listing = store.list(capabilities);

    let stale: Vec<HookSeed> = family_status(&listing, installed)
        .into_iter()
        .filter(|entry| entry.stale)
        .map(|entry| entry.seed)
        .collect();

    if stale.is_empty() {
        return Ok(Vec::new());
    }

    apply_listed(store, &listing, &stale, Switch::On, now)
}
