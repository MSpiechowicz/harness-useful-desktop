//! Harness commands: detection (cached), binary overrides, config and extensions views, and the
//! slash-command catalog.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::model::{BinaryOverride, CommandSpec, ConfigView, ExtensionsView, HarnessId, HarnessSummary};
use harness_business::process::{ResolvedBinary, find_override};
use harness_business::project::Projects;
use tauri::{AppHandle, State};

use super::{blocking, saved_language};
use crate::dialogs::{self, Pick};
use crate::error::CommandResult;
use crate::native_text::native_text;
use crate::state::AppState;

/// Every registered harness with a detection. Detects all of them concurrently on first use;
/// later calls return the cache, which `harness_detect` refreshes.
#[tauri::command]
pub async fn harness_list(state: State<'_, AppState>) -> CommandResult<Vec<HarnessSummary>> {
    if let Some(cached) = cached_summaries(&state).await {
        return Ok(cached);
    }

    let overrides = stored_overrides(&state).await?;
    let summaries = state.registry.summaries(&state.env, &state.runner, &overrides).await;

    let mut cache = state.detections.write().await;
    for summary in &summaries {
        cache.insert(summary.info.id, summary.clone());
    }

    Ok(summaries)
}

/// Re-runs detection for one harness.
#[tauri::command]
pub async fn harness_detect(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<HarnessSummary> {
    detect(&state, harness).await
}

/// Lets the user choose the harness executable in a native file dialog; stores it as the binary
/// override and re-detects. `None` when cancelled; `invalidInput` when the file is not named like
/// the harness binary.
#[tauri::command]
pub async fn harness_pick_binary(
    app: AppHandle,
    state: State<'_, AppState>,
    harness: HarnessId,
) -> CommandResult<Option<HarnessSummary>> {
    let binary_name = find(&state, harness)?.binary_name();
    let title = native_text(&saved_language(&state).await?).choose_executable(binary_name);

    let Some(picked) = dialogs::pick(&app, &state.dialog, Pick::File, &title, Some(&state.env.home)).await? else {
        return Ok(None);
    };

    let prefs = state.prefs.clone();
    let os = state.env.os;
    blocking(move || Ok(prefs.with(|store| store.set_binary_override(os, harness, binary_name, &picked))?)).await?;

    detect(&state, harness).await.map(Some)
}

/// Removes the binary override and re-detects.
#[tauri::command]
pub async fn harness_clear_binary(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<HarnessSummary> {
    find(&state, harness)?;

    let prefs = state.prefs.clone();
    blocking(move || Ok(prefs.with(|store| store.clear_binary_override(harness))?)).await?;

    detect(&state, harness).await
}

/// The redacted configuration of a harness, with the project layer of `project` (which must be
/// the active project or a recent one).
#[tauri::command]
pub async fn config_view(
    state: State<'_, AppState>,
    harness: HarnessId,
    project: Option<String>,
) -> CommandResult<ConfigView> {
    with_project(&state, harness, project, |harness, env, project| {
        harness.config_view(env, project)
    })
    .await
}

/// Everything installed into a harness, with the project layer of `project` (same rule as
/// `config_view`).
#[tauri::command]
pub async fn extensions_view(
    state: State<'_, AppState>,
    harness: HarnessId,
    project: Option<String>,
) -> CommandResult<ExtensionsView> {
    with_project(&state, harness, project, |harness, env, project| {
        harness.extensions_view(env, project)
    })
    .await
}

/// The static slash-command catalog (live commands are merged in once sessions exist).
#[tauri::command]
pub async fn commands_catalog(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<Vec<CommandSpec>> {
    Ok(find(&state, harness)?.catalog().merged(&[]))
}

pub(super) fn find(state: &AppState, harness: HarnessId) -> CommandResult<&dyn Harness> {
    find_in(&state.registry, harness)
}

fn find_in(registry: &HarnessRegistry, harness: HarnessId) -> CommandResult<&dyn Harness> {
    registry
        .get(harness)
        .ok_or_else(|| BusinessError::NotFound(format!("harness {harness:?}")).into())
}

/// The cached summaries in registry order, when every harness has one.
async fn cached_summaries(state: &AppState) -> Option<Vec<HarnessSummary>> {
    let cache = state.detections.read().await;

    state
        .registry
        .all()
        .map(|harness| cache.get(&harness.id()).cloned())
        .collect()
}

/// Detects `harness` with its stored override and caches the result.
pub(super) async fn detect(state: &AppState, harness: HarnessId) -> CommandResult<HarnessSummary> {
    let found = find(state, harness)?;
    let overrides = stored_overrides(state).await?;

    let summary = found
        .summary(&state.env, &state.runner, find_override(&overrides, harness))
        .await;

    state.detections.write().await.insert(harness, summary.clone());

    Ok(summary)
}

/// The detected binary of `harness` (detecting it when the cache has no entry yet); `notFound`
/// when it is not installed.
pub(super) async fn installed_binary(state: &AppState, harness: HarnessId) -> CommandResult<ResolvedBinary> {
    let cached = state.detections.read().await.get(&harness).cloned();

    let summary = match cached {
        Some(summary) => summary,
        None => detect(state, harness).await?,
    };

    let detection = summary.detection;

    match detection.binary_path {
        Some(path) if detection.installed => Ok(ResolvedBinary {
            path: PathBuf::from(path),
            is_batch_script: detection.is_batch_script,
        }),
        _ => Err(BusinessError::NotFound(format!("{harness:?} is not installed")).into()),
    }
}

async fn stored_overrides(state: &AppState) -> CommandResult<Vec<BinaryOverride>> {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| store.load()).binary_overrides)).await
}

/// Authorizes `project` (see [`Projects::authorize`]) and runs `view` in a blocking task.
async fn with_project<T, F>(state: &AppState, harness: HarnessId, project: Option<String>, view: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&dyn Harness, &Env, Option<&Path>) -> harness_business::Result<T> + Send + 'static,
{
    let prefs = state.prefs.clone();
    let env = state.env.clone();
    let registry = state.registry.clone();

    blocking(move || {
        let project: Option<PathBuf> = prefs.with(|store| Projects::new(store, &env).authorize(project.as_deref()))?;
        let found = find_in(&registry, harness)?;

        Ok(view(found, &env, project.as_deref())?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use harness_business::model::IpcErrorCode;

    use super::*;
    use crate::test_support::TempDir;

    fn state_at(dir: &TempDir) -> AppState {
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).expect("create home");

        AppState::new(dir.path().join("config"), Env::for_tests(home))
    }

    fn project_dir(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join("home").join(name);
        std::fs::create_dir_all(&path).expect("create project");
        path
    }

    fn view(state: &AppState, project: Option<&PathBuf>) -> CommandResult<ConfigView> {
        let project = project.map(|path| path.to_string_lossy().into_owned());

        tauri::async_runtime::block_on(with_project(state, HarnessId::Omp, project, |harness, env, project| {
            harness.config_view(env, project)
        }))
    }

    #[test]
    fn config_view_accepts_no_project_and_known_projects() {
        let dir = TempDir::new("view-known");
        let state = state_at(&dir);
        let recent = project_dir(&dir, "recent");
        let active = project_dir(&dir, "active");
        state.prefs.with(|store| {
            let projects = Projects::new(store, &state.env);
            projects.open(&recent).expect("open recent");
            projects.open(&active).expect("open active");
        });

        assert!(view(&state, None).is_ok());
        assert!(view(&state, Some(&recent)).is_ok());
        assert!(view(&state, Some(&active)).is_ok());
    }

    #[test]
    fn config_view_refuses_unknown_projects() {
        let dir = TempDir::new("view-unknown");
        let state = state_at(&dir);
        let stranger = project_dir(&dir, "stranger");

        let error = view(&state, Some(&stranger)).expect_err("unknown project");

        assert_eq!(error.code(), IpcErrorCode::Forbidden);
    }
}
