//! The launch migration of the Harness Skills names (`skills_migrate_names`). Harness Skills 0.4.0
//! renamed everything to `harness-useful-skills` (plugin, marketplace, MCP server, data folders).
//! An install under the old names keeps working and is shown as installed with an update, and
//! this module moves it without a click: for each harness whose skills are found only under the
//! old names ([`legacy_skills_step`]) it runs the install path Recommended setup uses (the
//! marketplace plugin install for OMP and Claude Code, the Harness Skills installer for OpenCode
//! and Codex), which migrates the install and its data.
//!
//! - **Consent:** the install exists because the user set it up with consent, and the run happens
//!   only while the caller says automatic updates are on. The caller (not this module) holds that
//!   gate, and the release check below, which only [`migrate_skills_names`] sees as `latest`.
//! - **No downgrade:** only when the latest Harness Skills release is
//!   [`SKILLS_UNIFIED_VERSION`](super::catalog::SKILLS_UNIFIED_VERSION) or newer. Without a
//!   known release nothing runs.
//! - **Once per change:** a success is recorded per harness (`skills-names-migration.json` in the
//!   app's config folder), so an install the installer could not fully rename does not run again at
//!   every launch. The record entry goes away when the harness has nothing old left, so old names
//!   that come back are migrated again. A failure is not recorded and is retried at the next launch.
//! - **Disabled plugins and harnesses that are not installed** are left alone.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::catalog::SKILLS_UNIFIED_VERSION;
use super::installer::Companion;
use super::releases::is_newer_release;
use super::setup::{LegacySkillsStep, legacy_skills_step};
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
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    migrated: Vec<HarnessId>,
}

/// The harnesses migrated before, as loaded from the config folder.
#[derive(Debug)]
pub struct MigrationRecord {
    path: PathBuf,
    migrated: Vec<HarnessId>,
    dirty: bool,
}

impl MigrationRecord {
    /// Loads the record of `config_dir`: empty when there is none or it cannot be used (then it is
    /// rewritten by the next change).
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(MIGRATION_RECORD_FILE);

        let migrated = match SafeReader::read_to_string(&path, RECORD_CAP) {
            Ok(text) => match serde_json::from_str::<Stored>(&text) {
                Ok(stored) if stored.version == 1 => stored.migrated,
                _ => {
                    tracing::warn!("unusable Harness Skills migration record; starting a new one");
                    Vec::new()
                }
            },
            Err(BusinessError::NotFound(_)) => Vec::new(),
            Err(_) => Vec::new(),
        };

        Self {
            path,
            migrated,
            dirty: false,
        }
    }

    /// Whether `harness` was migrated before.
    pub fn is_migrated(&self, harness: HarnessId) -> bool {
        self.migrated.contains(&harness)
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

    fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let stored = Stored {
            version: 1,
            migrated: self.migrated.clone(),
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

/// Migrates the Harness Skills of each of `harnesses` that is installed under the old names only
/// (see the module docs). `latest` is the latest Harness Skills release, `None` when it is not
/// known. Never fails: what went wrong is in the report.
pub async fn migrate_skills_names(
    env: &Env,
    config_dir: &Path,
    harnesses: &[&dyn Harness],
    latest: Option<&str>,
    executor: &dyn MigrationExecutor,
) -> SkillsMigrationReport {
    let mut report = SkillsMigrationReport::default();

    let Some(latest) = latest else {
        return report;
    };

    if !has_unified_names(latest) {
        return report;
    }

    let mut record = MigrationRecord::load(config_dir);

    for harness in harnesses {
        let id = harness.id();

        let Some(step) = legacy_skills_step(env, *harness) else {
            record.forget(id);
            continue;
        };

        if record.is_migrated(id) {
            continue;
        }

        let outcome = match step {
            LegacySkillsStep::Actions(actions) => executor.run_actions(id, &actions).await,
            LegacySkillsStep::Installer(companion) => executor.run_installer(companion).await,
        };

        match outcome {
            Ok(()) => {
                record.mark(id);
                report.migrated.push(id);
            }
            Err(reason) => report.failed.push(SkillsMigrationFailure { harness: id, reason }),
        }
    }

    if let Err(error) = record.save() {
        tracing::warn!(%error, "cannot save the Harness Skills migration record");
    }

    report
}
