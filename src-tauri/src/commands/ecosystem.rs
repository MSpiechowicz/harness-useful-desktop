//! Ecosystem commands: which companion tools are installed, their latest GitHub releases, and
//! the Harness Dashboard's data through its local, read-only MCP server (`harness-dashboard mcp`,
//! pooled in [`AppState`]).

use std::future::Future;
use std::sync::OnceLock;

use harness_business::ecosystem;
use harness_business::model::{
    DashboardAvailability, DashboardQuery, DashboardResult, EcosystemInstalled, EcosystemRelease, EcosystemStatus,
};
use harness_business::prefs::PrefsOrigin;
use tauri::async_runtime::block_on;
use tauri::{AppHandle, State};

use super::blocking;
use crate::error::CommandResult;
use crate::releases::{fetch_release, release_client};
use crate::state::{AppState, SharedPrefs};

/// The dashboard, the skills and the sidebar with their detection on this machine.
#[tauri::command]
pub async fn ecosystem_status(state: State<'_, AppState>) -> CommandResult<EcosystemStatus> {
    let env = state.env.clone();
    let runner = state.runner;

    // Detection reads the OMP plugin registries besides probing the dashboard's version.
    blocking(move || Ok(block_on(ecosystem::ecosystem_status(&env, &runner)))).await
}

/// The latest GitHub release of each companion in `installed`, compared with its installed
/// version. Empty, without any request, while the `checkCompanionUpdates` preference is off or the
/// prefs file cannot be used (see [`companion_checks_on`]).
/// Failed lookups are left out (never an error); see [`crate::releases`] for what is requested.
#[tauri::command]
pub async fn ecosystem_releases(
    app: AppHandle,
    state: State<'_, AppState>,
    installed: Vec<EcosystemInstalled>,
) -> CommandResult<Vec<EcosystemRelease>> {
    let version = app.package_info().version.to_string();

    // Built on the first request only, so a lookup answered from the cache sets nothing up.
    let client = OnceLock::new();
    let fetch = |url| {
        let client = &client;
        let version = &version;

        async move {
            let client = client.get_or_init(|| release_client(version)).as_ref()?;
            fetch_release(client, url).await
        }
    };

    Ok(releases(&state, &installed, fetch).await)
}

/// [`ecosystem_releases`] with the requests made by `fetch`.
async fn releases<F, Fut>(state: &AppState, installed: &[EcosystemInstalled], fetch: F) -> Vec<EcosystemRelease>
where
    F: Fn(&'static str) -> Fut,
    Fut: Future<Output = Option<Vec<u8>>>,
{
    if !companion_checks_on(&state.prefs).await {
        return Vec::new();
    }

    state.releases.lookup(installed, fetch).await
}

/// The `checkCompanionUpdates` preference. Fails closed: off when a prefs file exists (or was
/// moved aside as corrupt) but cannot be used, since the user may have turned the check off in
/// it; on (the default) only when nothing was ever stored.
async fn companion_checks_on(prefs: &SharedPrefs) -> bool {
    let prefs = prefs.clone();

    let loaded = blocking(move || Ok(prefs.with(|store| store.load_checked()))).await;

    match loaded {
        Ok((prefs, PrefsOrigin::Stored | PrefsOrigin::Missing)) => prefs.check_companion_updates,
        Ok((_, PrefsOrigin::Unusable)) | Err(_) => false,
    }
}

/// Whether `dashboard_query` can be answered (starts the dashboard's MCP server when needed).
#[tauri::command]
pub async fn dashboard_availability(state: State<'_, AppState>) -> CommandResult<DashboardAvailability> {
    Ok(state.dashboard.availability(&state.env).await)
}

/// Calls one dashboard tool with validated arguments; `notFound` when the dashboard is not
/// installed, `invalidInput` for arguments the tool does not accept.
#[tauri::command]
pub async fn dashboard_query(state: State<'_, AppState>, query: DashboardQuery) -> CommandResult<DashboardResult> {
    Ok(state.dashboard.query(&state.env, &query).await?)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use harness_business::env::Env;
    use harness_business::model::{AppPrefs, EcosystemId};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::TempDir;

    fn state(dir: &TempDir, check_companion_updates: bool) -> AppState {
        let state = AppState::new(dir.path().join("config"), Env::for_tests(dir.path().join("home")));
        state.prefs.with(|store| {
            store
                .save(&AppPrefs {
                    check_companion_updates,
                    ..AppPrefs::default()
                })
                .expect("save prefs")
        });

        state
    }

    fn skills() -> Vec<EcosystemInstalled> {
        vec![EcosystemInstalled {
            id: EcosystemId::Skills,
            version: "0.2.36".to_owned(),
        }]
    }

    #[test]
    fn with_companion_checks_off_nothing_is_requested() {
        let dir = TempDir::new("releases-off");
        let state = state(&dir, false);

        let found = block_on(releases(&state, &skills(), |_url| async {
            panic!("a release was requested although companion checks are off")
        }));

        assert!(found.is_empty());
    }

    #[test]
    fn with_companion_checks_on_the_installed_companions_are_looked_up() {
        let dir = TempDir::new("releases-on");
        let state = state(&dir, true);
        let requested = Mutex::new(Vec::new());

        let found = block_on(releases(&state, &skills(), |url| {
            requested.lock().unwrap().push(url);
            async { Some(br#"{"tag_name":"v0.2.40"}"#.to_vec()) }
        }));

        assert_eq!(
            found,
            vec![EcosystemRelease {
                id: EcosystemId::Skills,
                latest_version: "0.2.40".to_owned(),
                update_available: true,
            }]
        );
        // All three are requested whichever are installed; only the installed one is compared.
        let mut requested = requested.into_inner().unwrap();
        requested.sort_unstable();
        let mut all = ecosystem::ECOSYSTEM.map(ecosystem::release_endpoint).to_vec();
        all.sort_unstable();
        assert_eq!(requested, all);
    }

    #[test]
    fn with_an_unusable_prefs_file_nothing_is_requested() {
        let dir = TempDir::new("releases-corrupt");
        let state = AppState::new(dir.path().join("config"), Env::for_tests(dir.path().join("home")));
        let prefs = state.prefs.with(|store| store.path());
        std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
        std::fs::write(&prefs, r#"{"version":1,"checkCompanionUpdates":false,"theme":"bogus"}"#).unwrap();

        let refuse = |_url| async { panic!("a release was requested although the prefs could not be read") };

        assert!(block_on(releases(&state, &skills(), refuse)).is_empty());
        // The corrupt file is now moved aside; the next call must not take that for "never set".
        assert!(block_on(releases(&state, &skills(), refuse)).is_empty());
    }

    #[test]
    fn without_any_prefs_file_companion_checks_are_on() {
        let dir = TempDir::new("releases-fresh");
        let state = AppState::new(dir.path().join("config"), Env::for_tests(dir.path().join("home")));

        assert!(block_on(companion_checks_on(&state.prefs)));
    }

    #[test]
    fn companion_checks_are_on_for_prefs_saved_before_the_setting_existed() {
        let dir = TempDir::new("releases-legacy");
        let state = AppState::new(dir.path().join("config"), Env::for_tests(dir.path().join("home")));
        let prefs = state.prefs.with(|store| store.path());
        std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
        std::fs::write(&prefs, r#"{"version":1,"updatePolicy":"off"}"#).unwrap();

        assert!(block_on(companion_checks_on(&state.prefs)));
    }
}
