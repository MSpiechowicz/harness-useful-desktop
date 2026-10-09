//! Extension actions: install, remove, enable, disable and update extensions through the harness
//! CLI (`omp plugin …`, `pi install|remove|update`).
//!
//! The webview sends a typed [`ExtensionAction`], never a command line. The harness must be
//! detected as installed (its resolved binary runs), a `project` must be the active or a recent
//! project (the CLI then runs in it), and the business layer validates every target. The UI
//! confirms each mutation with the user first and previews it with `dryRun` where the CLI can.

use std::path::PathBuf;

use harness_business::harness::{CliRun, run_cli_action};
use harness_business::model::{CliResult, ExtensionAction, HarnessId};
use tauri::State;

use super::authorized_dir;
use super::harness::{find, installed_binary};
use crate::error::CommandResult;
use crate::state::AppState;

/// Runs `action` with the CLI of `harness` (or previews it when `dry_run` is set). Success is
/// `ok` in the result, never the exit code.
#[tauri::command]
pub async fn extension_action(
    state: State<'_, AppState>,
    harness: HarnessId,
    action: ExtensionAction,
    dry_run: bool,
    project: Option<String>,
) -> CommandResult<CliResult> {
    run(&state, harness, action, dry_run, project).await
}

async fn run(
    state: &AppState,
    harness: HarnessId,
    action: ExtensionAction,
    dry_run: bool,
    project: Option<String>,
) -> CommandResult<CliResult> {
    let found = find(state, harness)?;

    let project: Option<PathBuf> = match project {
        Some(project) => Some(authorized_dir(state, project).await?),
        None => None,
    };

    let binary = installed_binary(state, harness).await?;

    let result = run_cli_action(CliRun {
        harness: found,
        env: &state.env,
        runner: &state.runner,
        binary: &binary,
        action: &action,
        dry_run,
        project: project.as_deref(),
    })
    .await?;

    tracing::info!(?harness, dry_run, ok = result.ok, exit_code = ?result.exit_code, "extension action finished");

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use harness_business::env::Env;
    use harness_business::model::{InstallScope, IpcErrorCode};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, cache_detection, state_with_project};

    fn install(scope: InstallScope) -> ExtensionAction {
        ExtensionAction::PluginInstall {
            target: "harness-useful-sidebar@harness-useful-sidebar".to_owned(),
            scope,
        }
    }

    fn path_text(path: &Path) -> Option<String> {
        Some(path.to_string_lossy().into_owned())
    }

    /// A state with `<home>/demo` opened and OMP detected at a binary that must never run.
    fn setup(dir: &TempDir) -> (AppState, PathBuf) {
        let home = dir.path().join("home");
        let project = home.join("demo");
        let state = state_with_project(dir, Env::for_tests(&home), &project);
        cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("missing-omp")));

        (state, project)
    }

    #[test]
    fn unknown_projects_are_refused_before_the_cli_runs() {
        let dir = TempDir::new("plugins-unknown");
        let (state, _project) = setup(&dir);
        let stranger = dir.path().join("home/stranger");
        std::fs::create_dir_all(&stranger).expect("create stranger");

        let error = block_on(run(
            &state,
            HarnessId::Omp,
            install(InstallScope::Project),
            true,
            path_text(&stranger),
        ))
        .expect_err("unknown project");

        assert_eq!(error.code(), IpcErrorCode::Forbidden);
    }

    #[test]
    fn a_harness_that_is_not_installed_cannot_run_actions() {
        let dir = TempDir::new("plugins-missing");
        let (state, _project) = setup(&dir);
        cache_detection(&state, HarnessId::Pi, None);

        let action = ExtensionAction::PackageInstall {
            source: "npm:pi-mcp-adapter".to_owned(),
        };
        let error = block_on(run(&state, HarnessId::Pi, action, true, None)).expect_err("not installed");

        assert_eq!(error.code(), IpcErrorCode::NotFound);
    }

    #[test]
    fn a_preview_the_cli_cannot_do_reports_the_command_without_running_it() {
        let dir = TempDir::new("plugins-preview");
        let (state, project) = setup(&dir);

        let action = ExtensionAction::PluginEnable {
            name: "demo-plugin@demo".to_owned(),
        };
        let result = block_on(run(&state, HarnessId::Omp, action, true, path_text(&project))).expect("a preview");

        assert!(result.ok);
        assert_eq!(result.exit_code, None);
        assert_eq!(result.argv, ["omp", "plugin", "enable", "demo-plugin@demo"]);
    }

    #[test]
    fn a_failure_to_start_the_cli_is_an_error() {
        let dir = TempDir::new("plugins-start");
        let (state, project) = setup(&dir);

        let error = block_on(run(
            &state,
            HarnessId::Omp,
            install(InstallScope::Project),
            true,
            path_text(&project),
        ))
        .expect_err("the binary is missing");

        assert_eq!(error.code(), IpcErrorCode::NotFound);
    }
}
