//! Team hooks in the app window (see `harness_business::hooks::team`): [`crate::team_sync`] for
//! hooks.
//!
//! - Whether hooks are shared comes from `harness-memory hooks config` (cached until the binary or
//!   its config file changes). While they are shared, a refresh lists the team's hooks for the
//!   global scope and each known project root (the most recent roots the Hooks view or a chat
//!   used, starting from `projects.json`), and writes the copies to `<hooks root>/team`.
//! - A refresh runs at launch, every [`TEAM_REFRESH`](crate::team_sync::TEAM_REFRESH), when the
//!   Hooks view asks (`hooks_team_refresh`), after a share or a removal, after the team database
//!   settings are saved, and soon after a chat in a new project root starts. Chats never wait for
//!   it: they run the copies already on disk.
//! - Sharing off, a server without team hooks, or no Harness Memory deletes the team folder at
//!   once, so team hooks stop. A server that cannot be asked keeps the copies but drops trust,
//!   so team hooks wait for approval until it answers again.
//!
//! Logs carry ids and counts, never a hook's text, command or body.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use harness_business::Result;
use harness_business::ecosystem::{HooksConfig, HooksSupport};
use harness_business::env::Env;
use harness_business::hooks::{HookKind, HookStore};
use harness_business::model::TeamHooks;

use super::HookRuntime;
use crate::team_sync::{self, TeamHost, TeamSync};

team_sync::team_settings!(TeamHooks);

impl TeamHost for HookRuntime {
    type Kind = HookKind;
    type Settings = TeamHooks;

    fn team_sync(&self) -> &TeamSync<HookKind, TeamHooks> {
        &self.team
    }

    fn env(&self) -> &Env {
        &self.env
    }

    fn doc_store(&self) -> Option<HookStore> {
        self.store.clone()
    }

    fn write_lock(&self) -> Arc<Mutex<()>> {
        self.writes.clone()
    }

    fn invalidate(&self) {
        HookRuntime::invalidate(self);
    }

    fn notify(&self) {
        HookRuntime::notify(self);
    }
}

impl HookRuntime {
    /// The team hooks' state for listings and the engine; `None` while hooks are not shared.
    pub fn team_settings(&self) -> Option<TeamHooks> {
        self.team.settings()
    }

    /// Whether the installed Harness Memory keeps team hooks, and what its config says.
    pub async fn team_support(&self) -> HooksSupport {
        self.team.support(&self.env).await
    }

    /// The installed server while hooks are shared (see
    /// [`MemoryHooks::sharing_server`](harness_business::ecosystem::MemoryHooks)).
    pub async fn sharing_server(&self) -> Result<(PathBuf, HooksConfig)> {
        self.team.sharing_server(&self.env).await
    }

    /// Notes that `root` (a project root) is in use: refreshes ask for its team hooks, and one not
    /// verified this session wakes a refresh (once per round). The filesystem root and the home
    /// folder are not projects and are left out.
    pub fn note_root(&self, root: &str) {
        self.team.note_root(&self.env, root);
    }

    /// True when a round of this session queried `root` and has not skipped it since, so its
    /// repository key may be used.
    pub fn root_verified(&self, root: &str) -> bool {
        self.team.root_verified(root)
    }

    /// Every root [`HookRuntime::root_verified`] holds true for now.
    pub fn verified_roots(&self) -> HashSet<String> {
        self.team.verified_roots()
    }

    /// Keeps refresh rounds from running while the guard lives (a share or a removal, so a round
    /// that listed before it cannot undo its copy).
    pub async fn hold_team_rounds(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.team.hold_rounds().await
    }

    /// Asks the refresh loop to run now.
    pub fn refresh_team_soon(&self) {
        self.team.refresh_soon();
    }

    /// Asks the server again whether hooks are shared and applies the answer (no lists): sharing
    /// off deletes the copies before this returns. True while hooks are shared.
    pub async fn team_reprobe(&self) -> bool {
        team_sync::reprobe(self).await
    }

    /// One refresh (see the module docs).
    pub async fn refresh_team(&self) {
        team_sync::refresh(self).await;
    }
}

/// Starts the refresh loop (it runs until the app exits): a refresh now, then every
/// [`TEAM_REFRESH`](crate::team_sync::TEAM_REFRESH) or when woken. Does nothing without a hooks
/// folder.
pub fn spawn(runtime: Arc<HookRuntime>) {
    team_sync::spawn(runtime);
}
