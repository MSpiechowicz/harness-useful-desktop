//! The launch migration of the Harness Skills names (`skills_migrate_names`, see
//! `harness_business::ecosystem::skills_migration`). The shell calls it once per launch. It runs
//! automatically: no consent step and no dependency on the update switches. The install exists
//! because the user set it up, and the migration only moves it to the names it now has. It does
//! nothing for a harness that is not installed.

use std::future::Future;

use harness_business::ecosystem::{Companion, MCP_CLI_TIMEOUT, MigrationExecutor, migrate_skills_names};
use harness_business::harness::HarnessCli;
use harness_business::model::{ExtensionAction, HarnessId, SkillsMigrationReport};
use harness_business::process::{BoxFuture, TokioSpawner};
use harness_business::security::redact_text;
use tauri::{AppHandle, State};

use super::ecosystem::latest_skills_release;
use super::harness::summaries;
use super::plugins;
use super::setup::install_streaming;
use crate::error::CommandResult;
use crate::mcp_cli::cached_binary;
use crate::state::AppState;

/// Moves the Harness Skills of the installed harnesses from the names before Harness Skills 0.4.0
/// to the new ones, and the dashboard's MCP server from `harness` to `harness-dashboard`. Never
/// fails the caller for a failed migration: that is in the report.
#[tauri::command]
pub async fn skills_migrate_names(app: AppHandle, state: State<'_, AppState>) -> CommandResult<SkillsMigrationReport> {
    migrate(&state, latest_skills_release(&app, &state)).await
}

/// [`skills_migrate_names`] with the lookup of the latest release given, which runs only when a
/// harness is installed. Without a known release only the MCP rename runs.
async fn migrate(
    state: &AppState,
    latest: impl Future<Output = Option<String>>,
) -> CommandResult<SkillsMigrationReport> {
    let installed: Vec<HarnessId> = summaries(state)
        .await?
        .into_iter()
        .filter(|summary| summary.detection.installed)
        .map(|summary| summary.info.id)
        .collect();

    if installed.is_empty() {
        return Ok(SkillsMigrationReport::default());
    }

    let latest = latest.await;

    let harnesses: Vec<&dyn harness_business::harness::Harness> =
        installed.iter().filter_map(|id| state.registry.get(*id)).collect();

    let report = migrate_skills_names(
        &state.env,
        &state.config_dir,
        &harnesses,
        latest.as_deref(),
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

    fn run_cli(&self, harness: HarnessId, args: Vec<String>) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let binary = cached_binary(self.state, harness)
                .await
                .ok_or_else(|| "the harness command was not found".to_owned())?;
            let found = self
                .state
                .registry
                .get(harness)
                .ok_or_else(|| "the harness is not known".to_owned())?;

            let cli = HarnessCli {
                harness: found,
                env: &self.state.env,
                runner: &self.state.runner,
                binary: &binary,
            };

            let result = cli
                .run_args(args, MCP_CLI_TIMEOUT)
                .await
                .map_err(|error| redact_text(&error.to_string()))?;

            if result.ok {
                return Ok(());
            }

            Err(result.summary.unwrap_or_else(|| "the command failed".to_owned()))
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;

    use harness_business::model::{AppPrefs, HarnessId};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, cache_detection, home_state};

    /// A state whose only detected harness is Claude Code at `claude`, with the dashboard's server
    /// in its global file under the old name `harness`.
    fn state_with_old_server(dir: &TempDir, claude: &Path) -> AppState {
        let state = home_state(dir);

        for harness in state.registry.all().map(|harness| harness.id()).collect::<Vec<_>>() {
            cache_detection(&state, harness, (harness == HarnessId::Claude).then_some(claude));
        }

        std::fs::write(
            dir.path().join("home/.claude.json"),
            r#"{"mcpServers":{"harness":{"type":"stdio","command":"harness-dashboard","args":["mcp"]}}}"#,
        )
        .expect("write the Claude global file");

        state
    }

    #[test]
    fn it_runs_with_the_update_switches_off_and_renames_the_dashboard_server_through_the_cli() {
        let dir = TempDir::new("skills-migration");

        // A `claude` that succeeds and prints nothing: the arguments it gets are the business crate's to test.
        let claude = dir.path().join("claude");
        let succeeds = ["/usr/bin/true", "/bin/true"]
            .into_iter()
            .find(|path| Path::new(path).exists());
        std::os::unix::fs::symlink(succeeds.expect("a `true` command"), &claude).expect("link the command");

        let state = state_with_old_server(&dir, &claude);
        let prefs = AppPrefs {
            auto_update_extensions: false,
            check_companion_updates: false,
            ..AppPrefs::default()
        };
        state.prefs.with(|store| store.save(&prefs)).expect("save the prefs");

        let report = block_on(migrate(&state, async { None })).expect("the migration answers");

        assert_eq!(report.migrated, [HarnessId::Claude]);
        assert!(report.failed.is_empty(), "{:?}", report.failed);
    }

    #[test]
    fn a_harness_whose_command_cannot_be_found_is_reported_not_skipped() {
        let dir = TempDir::new("skills-migration");
        let state = state_with_old_server(&dir, &dir.path().join("no-claude"));

        let report = block_on(migrate(&state, async { None })).expect("the migration answers");

        assert_eq!(report.migrated, []);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].harness, HarnessId::Claude);
    }
}
