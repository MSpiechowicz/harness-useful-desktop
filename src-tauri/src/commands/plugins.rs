//! Extension actions: install, remove, enable, disable and update extensions through the harness
//! CLI (`omp plugin …`, `pi install|remove|update`, `opencode plugin`).
//!
//! The webview sends a typed [`ExtensionAction`], never a command line. The harness must be
//! detected as installed (its resolved binary runs), a `project` must be the active or a recent
//! project (the CLI then runs in it), and the business layer validates every target. The UI
//! confirms each mutation with the user first and previews it with `dryRun` where the CLI can.
//!
//! An action the harness applies by editing its config file instead
//! ([`Harness::config_action`](harness_business::harness::Harness::config_action): OpenCode plugin
//! removal) runs on a blocking thread and never starts the CLI; its result carries `configEdit`.

use std::path::PathBuf;
use std::sync::Arc;

use harness_business::harness::{CliRun, run_cli_action};
use harness_business::model::{CliResult, ExtensionAction, HarnessId};
use tauri::State;

use super::harness::{find, find_in, installed_binary};
use super::{authorized_dir, blocking};
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

pub(crate) async fn run(
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

    if let Some(result) = config_action(state, harness, action.clone(), dry_run, project.clone()).await? {
        tracing::info!(
            ?harness,
            dry_run,
            ok = result.ok,
            "extension action edited the config file"
        );

        return Ok(result);
    }

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

/// The result of `action` when `harness` applies it by editing its config file, `None` when its
/// CLI runs it.
async fn config_action(
    state: &AppState,
    harness: HarnessId,
    action: ExtensionAction,
    dry_run: bool,
    project: Option<PathBuf>,
) -> CommandResult<Option<CliResult>> {
    let registry = Arc::clone(&state.registry);
    let env = state.env.clone();

    blocking(move || {
        let found = find_in(&registry, harness)?;

        Ok(found.config_action(&env, &action, project.as_deref(), dry_run)?)
    })
    .await
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
    fn an_opencode_plugin_removal_edits_the_global_config_without_running_the_cli() {
        let dir = TempDir::new("plugins-opencode-remove");
        let (state, _project) = setup(&dir);
        cache_detection(&state, HarnessId::OpenCode, Some(&dir.path().join("missing-opencode")));

        let config = dir.path().join("home/.config/opencode");
        std::fs::create_dir_all(&config).expect("create config dir");
        let file = config.join("opencode.json");
        std::fs::write(&file, r#"{"plugin": ["demo-plugin@1.2.0", "kept-plugin"]}"#).expect("write config");

        let remove = || ExtensionAction::PluginUninstall {
            name: "demo-plugin".to_owned(),
        };

        let preview = block_on(run(&state, HarnessId::OpenCode, remove(), true, None)).expect("a preview");
        let edit = preview.config_edit.as_ref().expect("a config edit");

        assert!(preview.ok);
        assert!(!edit.manual_edit);
        assert_eq!(edit.file, "~/.config/opencode/opencode.json");
        assert!(preview.argv.is_empty(), "nothing ran: {:?}", preview.argv);
        assert!(std::fs::read_to_string(&file).expect("read").contains("demo-plugin"));

        let removed = block_on(run(&state, HarnessId::OpenCode, remove(), false, None)).expect("removed");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");

        assert!(removed.ok);
        assert_eq!(written["plugin"], serde_json::json!(["kept-plugin"]));
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
