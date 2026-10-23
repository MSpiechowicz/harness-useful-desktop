//! The launch migration of the Harness Skills names (`skills_migrate_names`, see
//! `harness_business::ecosystem::skills_migration`). The shell calls it once per launch, and it
//! runs only while the user keeps automatic updates on (`autoUpdateExtensions`) and the release
//! check on (`checkCompanionUpdates`). The user already consented to the install being moved: they
//! set the skills up through Recommended setup or an extension action, and both switches are theirs.
//! It does nothing for a harness that is not installed.

use std::future::Future;

use harness_business::ecosystem::{Companion, MigrationExecutor, migrate_skills_names};
use harness_business::model::{ExtensionAction, HarnessId, SkillsMigrationReport};
use harness_business::process::{BoxFuture, TokioSpawner};
use tauri::{AppHandle, State};

use super::ecosystem::{latest_skills_release, stored_prefs};
use super::harness::summaries;
use super::plugins;
use super::setup::install_streaming;
use crate::error::CommandResult;
use crate::state::{AppState, SharedPrefs};

/// Moves the Harness Skills of the installed harnesses from the names before Harness Skills 0.4.0
/// to the new ones. Never fails the caller for a failed migration: that is in the report.
#[tauri::command]
pub async fn skills_migrate_names(app: AppHandle, state: State<'_, AppState>) -> CommandResult<SkillsMigrationReport> {
    migrate(&state, latest_skills_release(&app, &state)).await
}

/// [`skills_migrate_names`] with the lookup of the latest release given, which runs only after the
/// switches and the installed harnesses allow the migration.
async fn migrate(
    state: &AppState,
    latest: impl Future<Output = Option<String>>,
) -> CommandResult<SkillsMigrationReport> {
    if !automatic_updates_on(&state.prefs).await {
        return Ok(SkillsMigrationReport::default());
    }

    let installed: Vec<HarnessId> = summaries(state)
        .await?
        .into_iter()
        .filter(|summary| summary.detection.installed)
        .map(|summary| summary.info.id)
        .collect();

    if installed.is_empty() {
        return Ok(SkillsMigrationReport::default());
    }

    let Some(latest) = latest.await else {
        return Ok(SkillsMigrationReport::default());
    };

    let harnesses: Vec<&dyn harness_business::harness::Harness> =
        installed.iter().filter_map(|id| state.registry.get(*id)).collect();

    let report = migrate_skills_names(
        &state.env,
        &state.config_dir,
        &harnesses,
        Some(&latest),
        &AppExecutor { state },
    )
    .await;

    tracing::info!(
        migrated = report.migrated.len(),
        failed = report.failed.len(),
        "Harness Skills names migration finished"
    );

    Ok(report)
}

/// `autoUpdateExtensions` and `checkCompanionUpdates`, both on (off when the prefs file is unusable).
async fn automatic_updates_on(prefs: &SharedPrefs) -> bool {
    stored_prefs(prefs)
        .await
        .is_some_and(|prefs| prefs.auto_update_extensions && prefs.check_companion_updates)
}

/// Runs the steps with the harness CLIs (the same path as `extension_action`) and the installer
/// (the same path as `setup_install_opencode_skills`).
struct AppExecutor<'a> {
    state: &'a AppState,
}

impl MigrationExecutor for AppExecutor<'_> {
    fn run_actions<'a>(
        &'a self,
        harness: HarnessId,
        actions: &'a [ExtensionAction],
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            for action in actions {
                let result = plugins::run(self.state, harness, action.clone(), false, None)
                    .await
                    .map_err(|error| error.message().to_owned())?;

                if !result.ok {
                    return Err(result.summary.unwrap_or_else(|| "the command failed".to_owned()));
                }
            }

            Ok(())
        })
    }

    fn run_installer(&self, companion: Companion) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let result = install_streaming(self.state, &TokioSpawner, companion, |_| {})
                .await
                .map_err(|error| error.message().to_owned())?;

            if result.ok {
                return Ok(());
            }

            Err(result.summary.unwrap_or_else(|| "the installer failed".to_owned()))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use harness_business::model::AppPrefs;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, home_state};

    /// Migrates over a home whose prefs are `prefs`; true when the release lookup ran.
    fn lookup_ran(prefs: AppPrefs) -> (SkillsMigrationReport, bool) {
        let dir = TempDir::new("skills-migration");
        let state = home_state(&dir);
        state.prefs.with(|store| store.save(&prefs)).expect("save the prefs");

        let looked_up = AtomicBool::new(false);
        let report = block_on(migrate(&state, async {
            looked_up.store(true, Ordering::SeqCst);
            Some("0.4.0".to_owned())
        }))
        .expect("the migration answers");

        (report, looked_up.load(Ordering::SeqCst))
    }

    #[test]
    fn nothing_is_asked_or_run_while_automatic_updates_are_off() {
        for (extensions, releases) in [(false, true), (true, false), (false, false)] {
            let prefs = AppPrefs {
                auto_update_extensions: extensions,
                check_companion_updates: releases,
                ..AppPrefs::default()
            };

            assert_eq!(
                lookup_ran(prefs),
                (SkillsMigrationReport::default(), false),
                "extensions {extensions}, releases {releases}"
            );
        }
    }
}
