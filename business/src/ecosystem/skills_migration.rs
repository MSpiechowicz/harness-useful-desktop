//! The launch migration of the Harness Skills names (`skills_migrate_names`). Harness Skills 0.4.0
//! renamed everything to `harness-useful-skills` (plugin, marketplace, MCP server, data folders).
//! An install under the old names keeps working and is shown as installed with an update, and
//! this module moves it without a click: for each harness whose skills are found only under the
//! old names ([`legacy_skills_step`]) it runs
//!
//! - Claude Code: the marketplace plugin install of the new plugin, then the removal of the old
//!   plugin (Claude Code keeps the two as separate plugins, and both would load) and of the old
//!   marketplace when nothing else is installed from it;
//! - OpenCode and Codex: the Harness Skills installer, which migrates the old plugin, MCP entry
//!   and data itself;
//! - OMP: nothing, its plugin ID did not change and the plugin moves its own data.
//!
//! It also renames the dashboard's MCP server from `harness` to `harness-dashboard` where a
//! harness's user MCP file holds it under the old name ([`super::mcp_rename`]).
//!
//! - **No consent step:** the run is automatic, with no switch of its own. The install exists
//!   because the user set it up, and the migration only moves it to the names it now has.
//! - **No downgrade:** the skills step runs only when the latest Harness Skills release is
//!   [`SKILLS_UNIFIED_VERSION`](super::catalog::SKILLS_UNIFIED_VERSION) or newer. Without a known
//!   release no skills step runs.
//! - **Once per change:** a skills success is recorded per harness (`skills-names-migration.json`
//!   in the app's config folder), so an install the installer could not fully rename does not run
//!   again at every launch. The entry goes away when the harness has nothing old left, so old
//!   names that come back are migrated again. The MCP rename is recorded per harness too, once the
//!   file was read and had nothing left to rename or was renamed.
//! - **Failures are retried:** a failure is not recorded and is retried at the next launch from
//!   what is installed then. When the new Claude Code plugin is in but the old one could not be
//!   removed, the next launch only removes the old one.
//! - **Disabled plugins and harnesses that are not installed** are left alone.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::catalog::{DASHBOARD_MCP_SERVER, LEGACY_DASHBOARD_MCP_SERVER, SKILLS_UNIFIED_VERSION};
use super::installer::Companion;
use super::mcp_cli::{add_args, remove_args};
use super::mcp_merge::Blocked;
use super::mcp_rename::{read_rename_plan, rename_dashboard_mcp_file};
use super::releases::is_newer_release;
use super::setup::{LegacySkillsStep, McpFile, legacy_skills_step, mcp_target};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::replace_atomically;
use crate::harness::Harness;
use crate::model::{ExtensionAction, HarnessId, SkillsMigrationFailure, SkillsMigrationReport};
use crate::process::BoxFuture;
use crate::security::SafeReader;

/// File name inside the app's config folder.
pub const MIGRATION_RECORD_FILE: &str = "skills-names-migration.json";

/// Size cap for reading the record.
const RECORD_CAP: u64 = 16 * 1024;

/// Runs the steps; the app implements it with the harness CLIs and the installer. An error is the
/// short, redacted reason shown to the user.
pub trait MigrationExecutor: Send + Sync {
    /// Runs `actions` through the CLI of `harness`, in order; fails at the first one that fails.
    fn run_actions<'a>(
        &'a self,
        harness: HarnessId,
        actions: &'a [ExtensionAction],
    ) -> BoxFuture<'a, std::result::Result<(), String>>;

    /// Runs the installer of `companion`.
    fn run_installer(&self, companion: Companion) -> BoxFuture<'_, std::result::Result<(), String>>;

    /// Runs `args` (after the program name) through the CLI of `harness`, which changes the MCP
    /// servers of a file only it writes (Claude Code, Codex).
    fn run_cli(&self, harness: HarnessId, args: Vec<String>) -> BoxFuture<'_, std::result::Result<(), String>>;
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    version: u32,
    migrated: Vec<HarnessId>,
    /// The harnesses whose MCP file had nothing left to rename (see [`super::mcp_rename`]).
    #[serde(default)]
    mcp_checked: Vec<HarnessId>,
}

/// The harnesses migrated before, as loaded from the config folder.
#[derive(Debug)]
pub struct MigrationRecord {
    path: PathBuf,
    migrated: Vec<HarnessId>,
    mcp_checked: Vec<HarnessId>,
    dirty: bool,
}

impl MigrationRecord {
    /// Loads the record of `config_dir`: empty when there is none or it cannot be used (then it is
    /// rewritten by the next change).
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(MIGRATION_RECORD_FILE);

        let (migrated, mcp_checked) = match SafeReader::read_to_string(&path, RECORD_CAP) {
            Ok(text) => match serde_json::from_str::<Stored>(&text) {
                Ok(stored) if stored.version == 1 => (stored.migrated, stored.mcp_checked),
                _ => {
                    tracing::warn!("unusable Harness Skills migration record; starting a new one");
                    (Vec::new(), Vec::new())
                }
            },
            Err(BusinessError::NotFound(_)) => (Vec::new(), Vec::new()),
            Err(_) => (Vec::new(), Vec::new()),
        };

        Self {
            path,
            migrated,
            mcp_checked,
            dirty: false,
        }
    }

    /// Whether `harness` was migrated before.
    pub fn is_migrated(&self, harness: HarnessId) -> bool {
        self.migrated.contains(&harness)
    }

    /// Whether the MCP file of `harness` was renamed or found with nothing to rename before.
    pub fn is_mcp_checked(&self, harness: HarnessId) -> bool {
        self.mcp_checked.contains(&harness)
    }

    fn mark(&mut self, harness: HarnessId) {
        if !self.is_migrated(harness) {
            self.migrated.push(harness);
            self.dirty = true;
        }
    }

    fn forget(&mut self, harness: HarnessId) {
        if self.is_migrated(harness) {
            self.migrated.retain(|migrated| *migrated != harness);
            self.dirty = true;
        }
    }

    fn mark_mcp(&mut self, harness: HarnessId) {
        if !self.is_mcp_checked(harness) {
            self.mcp_checked.push(harness);
            self.dirty = true;
        }
    }

    fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let stored = Stored {
            version: 1,
            migrated: self.migrated.clone(),
            mcp_checked: self.mcp_checked.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&stored)
            .map_err(|error| BusinessError::Internal(format!("encoding the migration record: {error}")))?;

        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|error| BusinessError::from_io("creating the app's config folder", error))?;
        }

        replace_atomically(&self.path, &bytes, "saving the Harness Skills migration record")
    }
}

/// True for a plain release number at or above [`SKILLS_UNIFIED_VERSION`]. An older release, or a
/// tag with a suffix that cannot be compared, never runs an installer: it would only install the
/// old names again.
fn has_unified_names(latest: &str) -> bool {
    let comparable = is_newer_release(latest, "0");

    comparable && !is_newer_release(SKILLS_UNIFIED_VERSION, latest)
}

/// Migrates the Harness Skills of each of `harnesses` that is installed under the old names only,
/// and renames the dashboard's MCP server where it has the old name (see the module docs). `latest`
/// is the latest Harness Skills release, `None` when it is not known (no skills step runs then).
/// Never fails: what went wrong is in the report.
pub async fn migrate_skills_names(
    env: &Env,
    config_dir: &Path,
    harnesses: &[&dyn Harness],
    latest: Option<&str>,
    executor: &dyn MigrationExecutor,
) -> SkillsMigrationReport {
    let mut report = SkillsMigrationReport::default();
    let mut record = MigrationRecord::load(config_dir);
    let names_released = latest.is_some_and(has_unified_names);

    for harness in harnesses {
        if names_released {
            migrate_skills(env, *harness, executor, &mut record, &mut report).await;
        }

        rename_mcp(env, *harness, executor, &mut record, &mut report).await;
    }

    if let Err(error) = record.save() {
        tracing::warn!(%error, "cannot save the Harness Skills migration record");
    }

    report
}

/// The skills step of one harness (see the module docs).
async fn migrate_skills(
    env: &Env,
    harness: &dyn Harness,
    executor: &dyn MigrationExecutor,
    record: &mut MigrationRecord,
    report: &mut SkillsMigrationReport,
) {
    let id = harness.id();

    let Some(step) = legacy_skills_step(env, harness) else {
        record.forget(id);
        return;
    };

    if record.is_migrated(id) {
        return;
    }

    let outcome = match step {
        LegacySkillsStep::Actions { install, cleanup } => replace_plugin(executor, id, &install, &cleanup).await,
        LegacySkillsStep::Installer(companion) => executor.run_installer(companion).await,
    };

    match outcome {
        Ok(()) => {
            record.mark(id);
            push_migrated(report, id);
        }
        Err(reason) => report.failed.push(SkillsMigrationFailure { harness: id, reason }),
    }
}

/// Installs the new plugin (when `install` is not empty), then runs `cleanup`. A failed cleanup
/// says the new plugin is in, so the user knows only the old one is left.
async fn replace_plugin(
    executor: &dyn MigrationExecutor,
    harness: HarnessId,
    install: &[ExtensionAction],
    cleanup: &[ExtensionAction],
) -> std::result::Result<(), String> {
    if !install.is_empty() {
        executor.run_actions(harness, install).await?;
    }

    executor
        .run_actions(harness, cleanup)
        .await
        .map_err(|reason| format!("The new plugin is installed, but the old one could not be removed: {reason}"))
}

/// The MCP step of one harness: renames the dashboard's server from `harness` to
/// `harness-dashboard` in its user MCP file (see [`super::mcp_rename`]), once.
async fn rename_mcp(
    env: &Env,
    harness: &dyn Harness,
    executor: &dyn MigrationExecutor,
    record: &mut MigrationRecord,
    report: &mut SkillsMigrationReport,
) {
    let id = harness.id();

    if record.is_mcp_checked(id) {
        return;
    }

    let Some(file) = mcp_target(env, harness) else {
        return;
    };

    let outcome = if file.via_cli() {
        rename_through_cli(id, &file, executor).await
    } else {
        rename_in_file(&file)
    };

    match outcome {
        McpRename::Nothing => record.mark_mcp(id),
        McpRename::Renamed => {
            record.mark_mcp(id);
            push_migrated(report, id);
        }
        McpRename::Failed(reason) => report.failed.push(SkillsMigrationFailure { harness: id, reason }),
        McpRename::Unreadable => {}
    }
}

/// How the rename of the old MCP server went in one harness.
enum McpRename {
    /// Nothing is named `harness` and runs the dashboard (or the user's own entries stand in the way).
    Nothing,
    Renamed,
    /// The CLI failed: reported, and tried again at the next launch.
    Failed(String),
    /// The file cannot be read or written safely (a symlink, comments in a JSONC file, ...): left
    /// alone and asked again at the next launch, with no notice.
    Unreadable,
}

fn rename_in_file(file: &McpFile) -> McpRename {
    match rename_dashboard_mcp_file(file) {
        Ok(true) => McpRename::Renamed,
        Ok(false) => McpRename::Nothing,
        Err(Blocked::Refused(_) | Blocked::ManualEdit) => McpRename::Unreadable,
    }
}

/// Adds the server under the new name through the harness's CLI, then removes the old one, so a
/// failure in between leaves both and never neither.
async fn rename_through_cli(harness: HarnessId, file: &McpFile, executor: &dyn MigrationExecutor) -> McpRename {
    let Ok(plan) = read_rename_plan(file) else {
        return McpRename::Unreadable;
    };

    let Some(plan) = plan else {
        return McpRename::Nothing;
    };

    if plan.add {
        let Some(args) = add_args(file.shape, DASHBOARD_MCP_SERVER, &plan.entry) else {
            return McpRename::Failed(format!(
                "The {DASHBOARD_MCP_SERVER} MCP server cannot be added through the CLI as it is."
            ));
        };

        if let Err(reason) = executor.run_cli(harness, args).await {
            return McpRename::Failed(format!("The {DASHBOARD_MCP_SERVER} MCP server was not added: {reason}"));
        }
    }

    let Some(args) = remove_args(file.shape, LEGACY_DASHBOARD_MCP_SERVER) else {
        return McpRename::Unreadable;
    };

    match executor.run_cli(harness, args).await {
        Ok(()) => McpRename::Renamed,
        Err(reason) => McpRename::Failed(format!(
            "The {DASHBOARD_MCP_SERVER} MCP server is added, but the old {LEGACY_DASHBOARD_MCP_SERVER} one was not removed: {reason}"
        )),
    }
}

/// Lists `harness` once as migrated.
fn push_migrated(report: &mut SkillsMigrationReport, harness: HarnessId) {
    if !report.migrated.contains(&harness) {
        report.migrated.push(harness);
    }
}
