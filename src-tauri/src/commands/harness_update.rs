//! Updating the harness CLIs: the latest npm version of each and the harness's own updater.

use std::future::Future;
use std::sync::OnceLock;

use harness_business::BusinessError;
use harness_business::harness::update::run_update;
use harness_business::model::{HarnessId, HarnessInstalled, HarnessRelease, HarnessUpdateResult};
use harness_business::process::{ProcessRunner, find_override};
use tauri::{AppHandle, State};

use super::ecosystem::companion_checks_on;
use super::harness::{find, stored_overrides};
use crate::error::CommandResult;
use crate::releases::{NPM, fetch_release, release_client};
use crate::state::AppState;

/// The latest npm version of each harness in `installed`, compared with its installed version.
/// Empty, without any request, while the `checkCompanionUpdates` preference is off or the prefs file
/// cannot be used. Failed lookups are left out (never an error); see [`crate::releases`] for what
/// is requested.
#[tauri::command]
pub async fn harness_releases(
    app: AppHandle,
    state: State<'_, AppState>,
    installed: Vec<HarnessInstalled>,
) -> CommandResult<Vec<HarnessRelease>> {
    let version = app.package_info().version.to_string();

    // The client is built on the first request only, so a lookup answered from the cache sets
    // nothing up.
    let client = OnceLock::new();
    let fetch = |url| {
        let client = &client;
        let version = &version;

        async move {
            let client = client.get_or_init(|| release_client(version, NPM)).as_ref()?;
            fetch_release(client, url).await
        }
    };

    Ok(releases(&state, &installed, fetch).await)
}

/// [`harness_releases`] with the requests made by `fetch`.
pub(crate) async fn releases<F, Fut>(state: &AppState, installed: &[HarnessInstalled], fetch: F) -> Vec<HarnessRelease>
where
    F: Fn(&'static str) -> Fut,
    Fut: Future<Output = Option<Vec<u8>>>,
{
    if !companion_checks_on(&state.prefs).await {
        return Vec::new();
    }

    state.harness_releases.lookup(installed, fetch).await
}

/// Runs the harness's own updater (`claude update`, `pi update --self`) and re-detects it. The
/// webview names only the harness. `updated` is true when the updater succeeded and the version
/// changed, else `message` says why not. `notFound` when the harness is not installed,
/// `invalidInput` while an update of any harness is already running.
#[tauri::command]
pub async fn harness_update(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<HarnessUpdateResult> {
    update(&state, harness, &state.runner).await
}

/// [`harness_update`] with the process run by `runner`.
pub(crate) async fn update(
    state: &AppState,
    harness: HarnessId,
    runner: &dyn ProcessRunner,
) -> CommandResult<HarnessUpdateResult> {
    let found = find(state, harness)?;

    let Ok(_updating) = state.harness_updating.try_lock() else {
        let message = "No update can start while another one runs".to_owned();
        return Err(BusinessError::InvalidInput(message).into());
    };

    let overrides = stored_overrides(state).await?;
    let result = run_update(found, &state.env, runner, find_override(&overrides, harness)).await?;

    state.detections.write().await.insert(harness, result.summary.clone());

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use harness_business::harness::update::PACKAGES;
    use harness_business::model::IpcErrorCode;
    use harness_business::process::{BoxFuture, ExecOutput, ExecSpec};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, state_with_update_checks};

    /// A runner that must never be called.
    struct NoProcess;

    impl ProcessRunner for NoProcess {
        fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
            panic!("a process was started")
        }
    }

    fn pi() -> Vec<HarnessInstalled> {
        vec![HarnessInstalled {
            harness: HarnessId::Pi,
            version: "1.1.0".to_owned(),
        }]
    }

    #[test]
    fn with_companion_checks_off_nothing_is_requested() {
        let dir = TempDir::new("harness-releases-off");
        let state = state_with_update_checks(&dir, false);

        let found = block_on(releases(&state, &pi(), |_url| async {
            panic!("a version was requested although update checks are off")
        }));

        assert!(found.is_empty());
    }

    #[test]
    fn with_companion_checks_on_every_package_is_looked_up() {
        let dir = TempDir::new("harness-releases-on");
        let state = state_with_update_checks(&dir, true);
        let requested = Mutex::new(Vec::new());

        let found = block_on(releases(&state, &pi(), |url| {
            requested.lock().unwrap().push(url);
            async { Some(br#"{"version":"1.5.0"}"#.to_vec()) }
        }));

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].harness, HarnessId::Pi);
        assert!(found[0].update_available);
        assert_eq!(requested.into_inner().unwrap().len(), PACKAGES.len());
    }

    #[test]
    fn a_second_update_of_the_same_harness_is_refused() {
        let dir = TempDir::new("harness-update-busy");
        let state = state_with_update_checks(&dir, true);
        let _running = state.harness_updating.try_lock().expect("idle");

        let error = block_on(update(&state, HarnessId::Claude, &NoProcess)).expect_err("busy");

        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
    }

    #[test]
    fn an_update_of_another_harness_is_refused_while_one_runs() {
        let dir = TempDir::new("harness-update-serial");
        let state = state_with_update_checks(&dir, true);
        let _running = state.harness_updating.try_lock().expect("idle");

        for harness in [HarnessId::Pi, HarnessId::Codex] {
            let error = block_on(update(&state, harness, &NoProcess)).expect_err("busy");

            assert_eq!(error.code(), IpcErrorCode::InvalidInput);
        }
    }
}
