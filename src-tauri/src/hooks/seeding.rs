//! Harness family hooks at launch and after an install: seeding the available members' hooks, and
//! restoring a member's missing ones.

use std::time::SystemTime;

use harness_business::ecosystem::Companion;
use harness_business::hooks::{HookCapabilities, restore_member, seed_defaults};
use harness_business::model::HookRequirement;
use tauri::{AppHandle, Manager};

use super::{HookRuntime, team, watch};
use crate::state::AppState;

/// Seeds the Harness family hooks of the available members in the background, and starts the
/// watcher and the team hooks refresh; the window does not wait for any of them.
pub fn spawn_on_launch(app: &AppHandle) {
    let state = app.state::<AppState>();
    watch::spawn(state.hooks.clone());
    team::spawn(state.hooks.clone());

    let app = app.clone();
    drop(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        seed_available(&state.hooks).await;
    }));
}

/// Launch seeding: the family hooks of every available member that were never seeded.
pub async fn seed_available(runtime: &HookRuntime) {
    let available = runtime.capabilities().await;

    if available == HookCapabilities::default() {
        return;
    }

    let seeded = runtime
        .write(move |store| seed_defaults(store, available, SystemTime::now()))
        .await;

    match seeded {
        Ok(seeds) => tracing::info!(created = seeds.len(), "harness family hooks seeded"),
        Err(error) => tracing::warn!(code = ?error.code(), "harness family hooks not seeded"),
    }
}

/// After the app installed `companion`: its missing family hooks are added again (even ones the
/// user deleted), never overwriting one that exists. The Harness Skills bring no family hooks, so
/// nothing changes for them.
pub async fn after_member_install(runtime: &HookRuntime, companion: Companion) {
    let member = match companion {
        Companion::Memory => HookRequirement::HarnessMemory,
        Companion::Dashboard => HookRequirement::HarnessDashboard,
        Companion::Knowledge => HookRequirement::HarnessKnowledge,
        Companion::Code => HookRequirement::HarnessCode,
        Companion::OpenCodeSkills | Companion::CodexSkills => return,
    };

    runtime.refresh_members();
    let available = runtime.capabilities().await;

    let restored = runtime
        .write(move |store| restore_member(store, member, available, SystemTime::now()))
        .await;

    match restored {
        Ok(seeds) => tracing::info!(?member, added = seeds.len(), "harness family hooks restored"),
        Err(error) => tracing::warn!(?member, code = ?error.code(), "harness family hooks not restored"),
    }
}
